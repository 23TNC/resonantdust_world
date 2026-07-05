#!/usr/bin/env python3
"""Depth (height field) by INTEGRATING a co-located normal map — no learned model.

Marigold's monocular depth net (depth.py) reads the *diffuse* and needs shading /
perspective cues; on flat-shaded art it flattens. Feeding it the normal map AS the
diffuse doesn't help either — a normal map is a low-contrast, mostly-blue image, far
outside the natural-photo distribution the net was trained on, so it reads as one
flat surface (measured: ~26x less relief than the diffuse on a conifer). See
docs/de-lighting.md.

But for a 2.5D sprite the normal map already IS the gradient of a single height
field, so we can recover height by INTEGRATING it directly — Frankot-Chellappa least
squares in the frequency domain. Pure numpy (FFT), no torch, no GPU. This is the
canonical depth source for flat art (the Laigter normal being the point); it holds
up where the diffuse gives the depth net nothing to latch onto (e.g. the wolf).

Convention (canonical, shared by both normal engines — verified empirically, see
docs/de-lighting.md): OpenGL / view-space, X right (+red), Y up (+green), Z toward
viewer (+blue), [-1,1] mapped to [0,255]. For a height field h(row,col) in image
coords (row down, col right) the outward normal is N ~ (-h_col, h_row, 1), so the
per-texel gradients are

    h_col = -nx/nz        h_row = ny/nz

which Frankot-Chellappa integrates back to h. (The +green -> +h_row sign is the
row-down / Y-up flip already folded in; verified against synthetic luminance ramps.)

Output matches depth.py's file contract so the two engines are drop-in: `N.depth.png`
co-located, 16-bit grayscale, per-sprite affine-invariant (normalised within the
silhouette), NO alpha (coverage comes from the shared albedo/normal alpha). By
default it stores HEIGHT (taller relief = brighter = 1, flat ground = 0); pass
--invert for depth.py's legacy near=0 / far=1 sense. Outside the silhouette the
gradients are forced flat (the clean flat-up plateau), so Laigter's noisy edge guess
in the transparent region can't bleed spurious relief into the rim.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image

# Shared discovery / path helpers live in the sibling de-lighter.
from delight import find_diffuse, out_path


def integrate_frankot_chellappa(p: np.ndarray, q: np.ndarray) -> np.ndarray:
    """Least-squares height whose gradients best match p = h_col, q = h_row.

    Solves the Poisson equation in the frequency domain (Frankot & Chellappa 1988):
    the height that minimises the integrability error is a single division per
    frequency. Boundaries are periodic — fine for a sprite floated on a flat plateau.
    """
    m, n = p.shape
    wr = 2.0 * np.pi * np.fft.fftfreq(m)   # angular freq along rows (axis 0)
    wc = 2.0 * np.pi * np.fft.fftfreq(n)   # angular freq along cols (axis 1)
    wc_grid, wr_grid = np.meshgrid(wc, wr)
    denom = wc_grid ** 2 + wr_grid ** 2
    denom[0, 0] = 1.0                       # avoid /0; the DC term is set to 0 below
    p_hat = np.fft.fft2(p)
    q_hat = np.fft.fft2(q)
    h_hat = (-1j * wc_grid * p_hat - 1j * wr_grid * q_hat) / denom
    h_hat[0, 0] = 0.0                       # arbitrary height datum
    return np.real(np.fft.ifft2(h_hat))


def normal_to_height(normal_rgb: np.ndarray, mask: np.ndarray,
                     nz_floor: float, grad_clip: float) -> np.ndarray:
    """Integrate an RGB normal map -> per-sprite-normalised height in [0,1].

    Taller relief -> 1, flat ground (and everything outside the silhouette) -> 0.
    """
    n = normal_rgb.astype(np.float64) / 255.0 * 2.0 - 1.0
    nx, ny, nz = n[..., 0], n[..., 1], n[..., 2]
    # nz -> 0 is a wall (infinite slope); clamp so -nx/nz doesn't explode.
    nz = np.clip(nz, nz_floor, None)
    p = np.clip(-nx / nz, -grad_clip, grad_clip)   # h_col
    q = np.clip(ny / nz, -grad_clip, grad_clip)    # h_row
    # Retain a CLEAN flat-up background: outside the silhouette the surface is the
    # flat ground plane, so its gradient is exactly 0. This also discards Laigter's
    # noisy edge guess in the transparent region (measured std ~30 near the rim).
    p = np.where(mask, p, 0.0)
    q = np.where(mask, q, 0.0)

    h = integrate_frankot_chellappa(p, q)

    if not mask.any():
        return np.zeros_like(h)
    inside = h[mask]
    lo = inside.min()
    hi = inside.max()
    rng = hi - lo if hi > lo else 1.0
    out = np.zeros_like(h)                          # background = flat ground (0)
    out[mask] = (inside - lo) / rng                 # taller relief -> 1
    return out


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Depth (height field) by integrating a co-located normal map.")
    ap.add_argument("paths", nargs="+", type=Path,
                    help="diffuse PNGs or directories to scan for *.diffuse.png")
    ap.add_argument("--out-dir", type=Path, default=None,
                    help="write depth here (flat) instead of co-located next to the diffuse")
    ap.add_argument("--nz-floor", type=float, default=0.05,
                    help="clamp normal.z to at least this before dividing (default 0.05)")
    ap.add_argument("--grad-clip", type=float, default=16.0,
                    help="clamp per-texel slope magnitude (default 16)")
    ap.add_argument("--invert", action="store_true",
                    help="store depth.py's legacy sense (near=0 / far=1) instead of height "
                         "(taller=1)")
    ap.add_argument("--skip-existing", action="store_true",
                    help="skip a sprite if its depth already exists")
    ap.add_argument("--visualize", action="store_true",
                    help="also write an 8-bit N.depth-viz.png for inspection")
    args = ap.parse_args()

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("marigold: no *.diffuse.png found under the given paths", file=sys.stderr)
        return 1

    if args.out_dir is not None:
        args.out_dir.mkdir(parents=True, exist_ok=True)

    if args.skip_existing:
        pending = [d for d in diffuse if not out_path(d, "depth", args.out_dir).exists()]
        skipped = len(diffuse) - len(pending)
        if skipped:
            print(f"marigold: skipping {skipped} sprite(s) with existing depth")
        diffuse = pending
        if not diffuse:
            print("marigold: nothing to do")
            return 0

    print(f"marigold: integrating normals -> depth for {len(diffuse)} sprite(s) "
          f"(Frankot-Chellappa, no model)", flush=True)
    t0 = time.time()
    ok = 0
    missing = 0
    for i, d in enumerate(diffuse, 1):
        # The normal is always co-located next to the diffuse (never in --out-dir).
        nrm_path = out_path(d, "normal", None)
        if not nrm_path.exists():
            print(f"  [{i}/{len(diffuse)}] {d.name}: no co-located {nrm_path.name} — "
                  f"run 'art normal' first", file=sys.stderr)
            missing += 1
            continue

        normal = Image.open(nrm_path).convert("RGB")
        # Silhouette from the diffuse's alpha — the true coverage. (Laigter normals
        # are opaque; the pipeline stamps alpha later, so trust the diffuse.)
        alpha = Image.open(d).convert("RGBA").getchannel("A")
        if alpha.size != normal.size:
            alpha = alpha.resize(normal.size, Image.NEAREST)
        mask = np.asarray(alpha) > 16

        height = normal_to_height(np.asarray(normal), mask,
                                  nz_floor=args.nz_floor, grad_clip=args.grad_clip)
        if args.invert:
            # depth.py's legacy sense: nearest (tallest) -> 0, farthest -> 1.
            height = np.where(mask, 1.0 - height, 1.0)

        img16 = (np.clip(height, 0.0, 1.0) * 65535.0).round().astype(np.uint16)
        op = out_path(d, "depth", args.out_dir)
        op.parent.mkdir(parents=True, exist_ok=True)
        # uint16 array -> PIL infers mode "I;16" (16-bit grayscale PNG); passing an
        # explicit mode= is deprecated in Pillow >=11.
        Image.fromarray(img16).save(op)

        if args.visualize:
            viz = (np.clip(height, 0.0, 1.0) * 255.0).round().astype(np.uint8)
            vp = op.with_name(op.name.replace(".depth.png", ".depth-viz.png"))
            Image.fromarray(viz).save(vp)
        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.name} -> {op.name}"
              f"{' (+viz)' if args.visualize else ''}", flush=True)

    tail = f" ({missing} skipped: no normal)" if missing else ""
    print(f"marigold: wrote {ok} depth map(s) in {time.time() - t0:.1f}s{tail}", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
