#!/usr/bin/env python3
"""PROTOTYPE — silhouette-DOME depth (+ AO), symmetric by construction.

The integrate-from-normal depth (depth_ao.py) inherits the source sprite's one-sided
lighting: the shadowed half gives Laigter a faint normal, so that half's relief — and
its AO — is weak. This estimates height from the SILHOUETTE instead: a distance
transform inside the outline (far-from-edge = tall) makes a rounded dome that depends
only on the SHAPE, not the lighting. So it's left/right symmetric automatically (no
hsym needed) AND it carries the MACRO volume the integrated relief never had (fixing
the "crinkled paper / no depth" look).

Final height = dome_weight · dome(silhouette)  +  detail_weight · relief(normal)
             — macro rounded volume from the shape, fine frond detail from the normal.

AO is the same HBAO horizon march as depth_ao.py, run over that combined height. With
--visualize, writes occlusion-compare.png: diffuse | dome | integrated | combined | AO
| albedo×AO. No scipy/cv2 — the distance transform is iterative 8-connected erosion.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# Reuse the AO march + helpers from the sibling prototype (same bin/lib dir on sys.path).
from depth_ao import height_to_ao, normal_to_height, find_diffuse, _panel


def dist_inside(mask: np.ndarray) -> np.ndarray:
    """Distance of each interior pixel from the nearest edge (8-connected chessboard),
    by iterative erosion — pure numpy, no scipy. Far-from-edge accumulates the most."""
    m = mask.copy()
    dist = np.zeros(mask.shape, np.float64)
    for _ in range(max(mask.shape)):
        e = (m & np.roll(m, 1, 0) & np.roll(m, -1, 0) & np.roll(m, 1, 1) & np.roll(m, -1, 1)
             & np.roll(np.roll(m, 1, 0), 1, 1) & np.roll(np.roll(m, 1, 0), -1, 1)
             & np.roll(np.roll(m, -1, 0), 1, 1) & np.roll(np.roll(m, -1, 0), -1, 1))
        e[0, :] = e[-1, :] = e[:, 0] = e[:, -1] = False   # image border reads as background
        m = e
        if not m.any():
            break
        dist += m
    return dist


def norm01(a: np.ndarray, mask: np.ndarray) -> np.ndarray:
    """Normalise to [0,1] over the silhouette; 0 outside."""
    out = np.zeros_like(a)
    if mask.any():
        inside = a[mask]
        lo, hi = float(inside.min()), float(inside.max())
        rng = hi - lo if hi > lo else 1.0
        out[mask] = (inside - lo) / rng
    return out


def dome_height(mask: np.ndarray, shape: float) -> np.ndarray:
    """Rounded dome from the silhouette: normalised distance-from-edge, gamma-shaped
    (shape<1 rounds the top toward a cap; 1 = linear cone)."""
    d = norm01(dist_inside(mask), mask)
    return np.where(mask, np.clip(d, 0, 1) ** shape, 0.0)


def main() -> int:
    ap = argparse.ArgumentParser(description="PROTOTYPE: silhouette-dome depth + AO.")
    ap.add_argument("paths", nargs="+", type=Path, help="diffuse.png files or dirs")
    ap.add_argument("--dome-shape", type=float, default=0.6, help="dome gamma; <1 rounds the top (default 0.6)")
    ap.add_argument("--dome-weight", type=float, default=1.0, help="macro dome contribution (default 1.0)")
    ap.add_argument("--detail-weight", type=float, default=0.4, help="fine relief-from-normal contribution (default 0.4)")
    ap.add_argument("--height-scale", type=float, default=48.0, help="px relief for the AO slope (default 48)")
    ap.add_argument("--radius", type=float, default=8.0, help="AO horizon march radius px (default 8)")
    ap.add_argument("--strength", type=float, default=1.5, help="AO darkness (default 1.5)")
    ap.add_argument("--visualize", action="store_true", help="write occlusion-compare.png")
    args = ap.parse_args()

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("depth_dome: no diffuse.png found", file=sys.stderr)
        return 1

    print(f"depth_dome: silhouette dome + AO for {len(diffuse)} sprite(s) "
          f"[shape {args.dome_shape}, dome {args.dome_weight} + detail {args.detail_weight}]", flush=True)
    ok = 0
    for i, d in enumerate(diffuse, 1):
        alpha = np.asarray(Image.open(d).convert("RGBA").getchannel("A"))
        mask = alpha > 16

        dome = dome_height(mask, args.dome_shape)
        # Optional fine detail from the co-located normal (integrated relief).
        nrm_path = d.with_name("normal.png")
        if nrm_path.exists() and args.detail_weight > 0:
            nrm = np.asarray(Image.open(nrm_path).convert("RGB"))
            if nrm.shape[:2] != mask.shape:
                nrm = np.asarray(Image.fromarray(nrm).resize(mask.shape[::-1], Image.NEAREST))
            relief = norm01(normal_to_height(nrm, mask), mask)
        else:
            relief = np.zeros_like(dome)

        combined = norm01(args.dome_weight * dome + args.detail_weight * relief, mask)
        ao = height_to_ao(combined, mask, height_scale=args.height_scale, radius=args.radius,
                          dirs=8, steps=4, strength=args.strength, bias=0.3, power=1.0)

        ao8 = (np.clip(ao, 0, 1) * 255).round().astype(np.uint8)
        Image.fromarray(ao8, "L").save(d.with_name("occlusion.png"))

        tail = ""
        if args.visualize:
            sc = max(1, 256 // max(mask.shape))
            g = lambda a: (np.clip(a, 0, 1) * 255).round().astype(np.uint8)
            panels = [
                _panel(Image.open(d), "diffuse", sc),
                _panel(g(dome), "dome (silhouette)", sc),
                _panel(g(relief), "integrated relief", sc),
                _panel(g(combined), "combined height", sc),
                _panel(ao8, "AO (dome-based)", sc),
            ]
            alb = d.with_name("albedo.png")
            if alb.exists():
                a = np.asarray(Image.open(alb).convert("RGB")).astype(np.float64)
                panels.append(_panel((a * ao[..., None]).clip(0, 255).astype(np.uint8), "albedo x AO", sc))
            gap = 8
            w = sum(p.width for p in panels) + gap * (len(panels) + 1)
            hh = max(p.height for p in panels) + 2 * gap
            sheet = Image.new("RGB", (w, hh), (0, 0, 0))
            x = gap
            for p in panels:
                sheet.paste(p, (x, gap)); x += p.width + gap
            cp = d.with_name("occlusion-compare.png"); sheet.save(cp)
            tail = f" (+{cp.name})"
        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.parent.name} -> occlusion.png{tail}", flush=True)

    print(f"depth_dome: wrote {ok} AO map(s)", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
