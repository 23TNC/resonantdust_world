#!/usr/bin/env python3
"""Horizontal-symmetry relief balancing for a tangent-space normal map (`--hsym`).

A sprite lit from one side in its source art gives Laigter a low-contrast SHADOWED
half, so the derived normal is faint there (e.g. a conifer lit upper-left → weak
right-side relief → weak AO on the right). This rebalances the two halves WITHOUT
touching the source: measure each half's average relief strength, pick the STRONGER
half as the target, and scale the weaker half's tangent (XY) up so its average matches
— then renormalize (the canonical normal-strength operation, per-half).

"Relief strength" = the tangent tilt magnitude sqrt(nx²+ny²) (0 = flat-up, 1 = fully
sideways), averaged over the silhouette pixels in each half.

The per-column scale is FEATHERED across the midline (a linear band) so the stronger
and weaker halves blend instead of leaving a relief seam down the centre. The scale is
clamped so a near-flat half can't blow up. The normal's ALPHA (packed AO, if present)
is preserved untouched; the flat-up background (tilt 0) is unaffected by any scale.

Runs on the same leaves as the rest of the pipeline: give it diffuse.png files or dirs
(the co-located normal.png is balanced, the diffuse's alpha is the silhouette mask).
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def find_diffuse(paths: list[Path]) -> list[Path]:
    out: list[Path] = []
    for p in paths:
        if p.is_dir():
            out += sorted(p.rglob("diffuse.png"))
        elif p.is_file() and p.name == "diffuse.png":
            out.append(p)
        else:
            print(f"normal_hsym: not a directory or diffuse.png: {p}", file=sys.stderr)
            return []
    return out


def balance(nrm_rgb: np.ndarray, mask: np.ndarray, *, feather: float,
            max_scale: float) -> tuple[np.ndarray, dict]:
    """Return the rebalanced RGB (uint8) plus before/after stats."""
    n = nrm_rgb.astype(np.float64) / 255.0 * 2.0 - 1.0
    nx, ny, nz = n[..., 0].copy(), n[..., 1].copy(), n[..., 2].copy()
    tilt = np.hypot(nx, ny)                                    # relief strength per texel
    H, W = tilt.shape
    mid = W // 2
    lmask = mask.copy(); lmask[:, mid:] = False
    rmask = mask.copy(); rmask[:, :mid] = False
    # A unit normal's "strength" is its ROTATION off flat-up; what reads as relief is how
    # much that rotation VARIES across the half (ridges = neighbours tilting opposite ways),
    # NOT its average. So equalize the per-half CONTRAST (std of tilt), not the mean — a
    # low-contrast (faint) half and a high-contrast half can share the same average tilt.
    avgL = float(tilt[lmask].std()) if lmask.any() else 0.0
    avgR = float(tilt[rmask].std()) if rmask.any() else 0.0
    target = max(avgL, avgR)
    # Scale factor per HALF: the stronger half stays 1.0, the weaker's tangent is scaled so
    # its tilt CONTRAST matches (scaling XY by k multiplies the tilt spread by ~k).
    kL = 1.0 if avgL >= avgR else min(max_scale, target / avgL if avgL > 1e-4 else max_scale)
    kR = 1.0 if avgR >= avgL else min(max_scale, target / avgR if avgR > 1e-4 else max_scale)

    # Per-column scale, feathered across the midline so there is no seam.
    fw = max(1.0, feather * W)                                 # feather half-width px
    cols = np.arange(W, dtype=np.float64)
    t = np.clip((cols - (mid - fw)) / (2.0 * fw), 0.0, 1.0)    # 0 on left, 1 on right
    scale_col = kL * (1.0 - t) + kR * t                        # (W,)
    scale = np.broadcast_to(scale_col, (H, W))

    nx2 = np.where(mask, nx * scale, nx)
    ny2 = np.where(mask, ny * scale, ny)
    length = np.sqrt(nx2 * nx2 + ny2 * ny2 + nz * nz)
    length = np.where(length < 1e-6, 1.0, length)
    out = np.stack([nx2 / length, ny2 / length, nz / length], -1)
    out8 = ((out + 1.0) * 0.5 * 255.0).round().clip(0, 255).astype(np.uint8)

    # after-stats
    to = np.hypot(out[..., 0], out[..., 1])
    stats = dict(avgL=avgL, avgR=avgR, kL=kL, kR=kR,
                 newL=float(to[lmask].std()) if lmask.any() else 0.0,
                 newR=float(to[rmask].std()) if rmask.any() else 0.0)
    return out8, stats


def main() -> int:
    ap = argparse.ArgumentParser(description="Horizontal-symmetry relief balancing for normal maps.")
    ap.add_argument("paths", nargs="+", type=Path, help="diffuse.png files or dirs to scan")
    ap.add_argument("--feather", type=float, default=0.12,
                    help="feather band half-width as a fraction of sprite width (default 0.12)")
    ap.add_argument("--max-scale", type=float, default=4.0,
                    help="clamp the weaker-half boost so a near-flat half can't blow up (default 4)")
    args = ap.parse_args()

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("normal_hsym: no diffuse.png found", file=sys.stderr)
        return 1

    ok = 0
    for i, d in enumerate(diffuse, 1):
        np_path = d.with_name("normal.png")
        if not np_path.exists():
            print(f"  [{i}/{len(diffuse)}] {d.parent.name}: no normal.png", file=sys.stderr)
            continue
        img = Image.open(np_path)
        alpha_ch = img.getchannel("A") if "A" in img.getbands() else None   # preserve packed AO
        rgb = np.asarray(img.convert("RGB"))
        amask = Image.open(d).convert("RGBA").getchannel("A")
        if amask.size != img.size:
            amask = amask.resize(img.size, Image.NEAREST)
        mask = np.asarray(amask) > 16

        out8, s = balance(rgb, mask, feather=args.feather, max_scale=args.max_scale)
        if alpha_ch is not None:
            out = Image.fromarray(np.dstack([out8, np.asarray(alpha_ch)]), "RGBA")
        else:
            out = Image.fromarray(out8, "RGB")
        out.save(np_path)
        weak = "R" if s["avgR"] < s["avgL"] else "L"
        print(f"  [{i}/{len(diffuse)}] {d.parent.name}: L={s['avgL']:.3f} R={s['avgR']:.3f} "
              f"-> boost {weak} x{max(s['kL'], s['kR']):.2f} -> L={s['newL']:.3f} R={s['newR']:.3f}",
              flush=True)
        ok += 1

    print(f"normal_hsym: balanced {ok} normal(s)", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
