#!/usr/bin/env python3
"""PROTOTYPE — ambient occlusion from the normal-integrated HEIGHT FIELD.

Bench reference for a CLIENT-SIDE AO pass. The client already integrates a height
field from the normal at render time (see marigold/normal_depth.py — Frankot-Chellappa,
pure FFT, no model), so AO can be computed from that SAME height in-shader and NOTHING
needs baking. This script:

  1. integrates the co-located normal -> height (identical math to normal_depth.py, so
     the AO is derived from exactly the height the client will have), then
  2. runs an HBAO-style horizon occlusion over that height,

and writes `occlusion.png` co-located (+ a `occlusion-compare.png` montage under
--visualize) so we can eyeball the method on foliage BEFORE porting the loop to
`pixijs/.../lightingShader.ts`.

Why this beats Laigter here: Laigter's AO is a smoothed copy of the diffuse LUMINANCE
(bright=raised), which on a conifer just re-emits the albedo (near-white foliage).
This reads occlusion off the GEOMETRY (the integrated relief), so frond-to-frond
cavities darken even where the albedo is uniform.

The height->AO step is the SAME work a fragment shader does sampling the depth
composite (the lighting shader's shadow march already marches uDepth): for each of D
screen directions, march S steps and keep the MAX horizon slope; AO = 1 - mean over
directions of sin(horizon_angle). Direct port: replace np.roll taps with texture()
samples of the depth composite at the fragment's uv + dir*step*uUvPerWorld.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw


# ---- normal -> height (copied verbatim from marigold/normal_depth.py so the proto is
# ---- self-contained under system python3, and the height MATCHES what the client
# ---- integrates; see that file's docstring for the convention derivation). ----------

def integrate_frankot_chellappa(p: np.ndarray, q: np.ndarray) -> np.ndarray:
    """Least-squares height whose gradients best match p = h_col, q = h_row."""
    m, n = p.shape
    wr = 2.0 * np.pi * np.fft.fftfreq(m)
    wc = 2.0 * np.pi * np.fft.fftfreq(n)
    wc_grid, wr_grid = np.meshgrid(wc, wr)
    denom = wc_grid ** 2 + wr_grid ** 2
    denom[0, 0] = 1.0
    p_hat = np.fft.fft2(p)
    q_hat = np.fft.fft2(q)
    h_hat = (-1j * wc_grid * p_hat - 1j * wr_grid * q_hat) / denom
    h_hat[0, 0] = 0.0
    return np.real(np.fft.ifft2(h_hat))


def normal_to_height(normal_rgb: np.ndarray, mask: np.ndarray,
                     nz_floor: float = 0.05, grad_clip: float = 16.0) -> np.ndarray:
    """Integrate an RGB normal map -> per-sprite-normalised height in [0,1]."""
    n = normal_rgb.astype(np.float64) / 255.0 * 2.0 - 1.0
    nx, ny, nz = n[..., 0], n[..., 1], n[..., 2]
    nz = np.clip(nz, nz_floor, None)
    p = np.clip(-nx / nz, -grad_clip, grad_clip)   # h_col
    q = np.clip(ny / nz, -grad_clip, grad_clip)    # h_row
    p = np.where(mask, p, 0.0)
    q = np.where(mask, q, 0.0)
    h = integrate_frankot_chellappa(p, q)
    if not mask.any():
        return np.zeros_like(h)
    inside = h[mask]
    lo, hi = inside.min(), inside.max()
    rng = hi - lo if hi > lo else 1.0
    out = np.zeros_like(h)
    out[mask] = (inside - lo) / rng
    return out


# ---- height -> AO (the shader-portable part) ----------------------------------------

def height_to_ao(h: np.ndarray, mask: np.ndarray, *, height_scale: float,
                 radius: float, dirs: int, steps: int, strength: float,
                 bias: float, power: float) -> np.ndarray:
    """HBAO-style horizon occlusion over the height field.

    h is per-sprite-normalised [0,1]; height_scale lifts it into pixel-equivalent
    relief so a slope (dh/dist) is meaningful against the pixel radius. For each of
    `dirs` screen directions we march `steps` taps out to `radius` px, keep the max
    horizon slope, convert to sin(horizon), and average — occluded pixels (in a
    cavity, neighbours rising above them) darken. Outside the silhouette: 1 (open).

    Direct shader analogue: the np.roll tap == texture(uDepth, uv + dir*dist*uUvPerWorld).
    """
    H = h * height_scale
    occ = np.zeros_like(H)
    for d in range(dirs):
        ang = 2.0 * np.pi * d / dirs
        ca, sa = np.cos(ang), np.sin(ang)
        max_slope = np.zeros_like(H)
        for s in range(1, steps + 1):
            dist = radius * s / steps
            dy = int(round(sa * dist))
            dx = int(round(ca * dist))
            if dy == 0 and dx == 0:
                continue
            Hs = np.roll(np.roll(H, dy, axis=0), dx, axis=1)
            slope = (Hs - H - bias) / dist
            max_slope = np.maximum(max_slope, slope)
        occ += max_slope / np.sqrt(1.0 + max_slope * max_slope)   # sin(horizon)
    occ /= dirs
    ao = 1.0 - strength * np.clip(occ, 0.0, 1.0)
    ao = np.clip(ao, 0.0, 1.0) ** power
    return np.where(mask, ao, 1.0)


# ---- discovery (mirrors _find_diffuse: the variant-leaf diffuse.png) -----------------

def find_diffuse(paths: list[Path]) -> list[Path]:
    out: list[Path] = []
    for p in paths:
        if p.is_dir():
            out += sorted(p.rglob("diffuse.png"))
        elif p.is_file() and p.name == "diffuse.png":
            out.append(p)
        else:
            print(f"depth_ao: not a directory or a diffuse.png file: {p}", file=sys.stderr)
            return []
    return out


def _panel(arr_or_img, label: str, scale: int) -> Image.Image:
    """A labelled, nearest-upscaled panel for the compare montage."""
    img = arr_or_img if isinstance(arr_or_img, Image.Image) else Image.fromarray(arr_or_img)
    img = img.convert("RGB").resize((img.width * scale, img.height * scale), Image.NEAREST)
    strip = Image.new("RGB", (img.width, 16), (0, 0, 0))
    ImageDraw.Draw(strip).text((3, 3), label, fill=(255, 255, 255))
    out = Image.new("RGB", (img.width, img.height + 16), (0, 0, 0))
    out.paste(img, (0, 0))
    out.paste(strip, (0, img.height))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="PROTOTYPE: AO from the normal-integrated height field.")
    ap.add_argument("paths", nargs="+", type=Path, help="diffuse.png files or dirs to scan")
    ap.add_argument("--height-scale", type=float, default=32.0,
                    help="lift normalised [0,1] height into px-relief before slope (default 32)")
    ap.add_argument("--radius", type=float, default=6.0, help="max horizon march radius px (default 6)")
    ap.add_argument("--dirs", type=int, default=8, help="horizon directions (default 8)")
    ap.add_argument("--steps", type=int, default=4, help="taps per direction (default 4)")
    ap.add_argument("--strength", type=float, default=1.0, help="AO darkness multiplier (default 1.0)")
    ap.add_argument("--bias", type=float, default=0.3, help="height bias px, kills self-occlusion noise (default 0.3)")
    ap.add_argument("--power", type=float, default=1.0, help="contrast gamma on AO (default 1.0)")
    ap.add_argument("--no-pack-normal", dest="pack_normal", action="store_false",
                    help="only write occlusion.png; do NOT pack AO into normal.png's alpha")
    ap.add_argument("--surface", action="store_true",
                    help="write the co-located surface.png (R=integrated height, G=AO, B=open) "
                         "and do NOT pack AO into normal.a — the successor to alpha-packing. "
                         "Alpha carries the silhouette (opaque interior), so it loads premultiply-safe.")
    ap.add_argument("--pack-only", action="store_true",
                    help="skip AO computation; pack an EXISTING occlusion.png into normal.a. "
                         "The maps pipeline computes AO off the UNTILTED normal, then packs "
                         "after the tilt has rewritten normal.rgb (which resets its alpha).")
    ap.add_argument("--ao-floor", type=float, default=0.03,
                    help="floor packed in-silhouette AO to this (>0) so the premultiplied "
                         "normal composite stays un-premultiply-able in the shader (default 0.03)")
    ap.add_argument("--visualize", action="store_true",
                    help="also write occlusion-compare.png (diffuse|height|AO|albedo*AO)")
    ap.set_defaults(pack_normal=True)
    args = ap.parse_args()
    if args.pack_only and not args.pack_normal:
        print("depth_ao: --pack-only with --no-pack-normal does nothing", file=sys.stderr)
        return 1

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("depth_ao: no diffuse.png found under the given paths", file=sys.stderr)
        return 1

    if args.pack_only:
        print(f"depth_ao: packing existing occlusion.png -> normal.a for {len(diffuse)} "
              f"sprite(s) [floor {args.ao_floor}]", flush=True)
    else:
        print(f"depth_ao: normal -> height -> AO for {len(diffuse)} sprite(s) "
              f"[scale {args.height_scale}, r{args.radius}, {args.dirs}x{args.steps}, "
              f"strength {args.strength}]", flush=True)
    t0 = time.time()
    ok = missing = 0
    for i, d in enumerate(diffuse, 1):
        nrm_path = d.with_name("normal.png")
        if not nrm_path.exists():
            print(f"  [{i}/{len(diffuse)}] {d.parent.name}: no co-located normal.png — run 'art normal' first",
                  file=sys.stderr)
            missing += 1
            continue

        normal = Image.open(nrm_path).convert("RGB")
        alpha = Image.open(d).convert("RGBA").getchannel("A")
        if alpha.size != normal.size:
            alpha = alpha.resize(normal.size, Image.NEAREST)
        mask = np.asarray(alpha) > 16

        op = d.with_name("occlusion.png")
        height = None
        if args.pack_only:
            # Reuse the AO already measured off the untilted normal (occlusion.png).
            if not op.exists():
                print(f"  [{i}/{len(diffuse)}] {d.parent.name}: no occlusion.png to pack — "
                      f"run 'art ao ... --no-pack-normal' first", file=sys.stderr)
                missing += 1
                continue
            if Image.open(op).size != normal.size:
                ao = np.asarray(Image.open(op).convert("L").resize(normal.size, Image.NEAREST))
            else:
                ao = np.asarray(Image.open(op).convert("L"))
            ao = ao.astype(np.float64) / 255.0
        else:
            height = normal_to_height(np.asarray(normal), mask)
            ao = height_to_ao(height, mask, height_scale=args.height_scale, radius=args.radius,
                              dirs=args.dirs, steps=args.steps, strength=args.strength,
                              bias=args.bias, power=args.power)
            ao8 = (np.clip(ao, 0.0, 1.0) * 255.0).round().astype(np.uint8)
            Image.fromarray(ao8, mode="L").save(op)   # standalone AO map (unfloored, opaque)

        # The successor to alpha-packing: one RGB(A) map holding R=height, G=AO, B=open.
        # Alpha is the SILHOUETTE (opaque interior, 0 outside), so a premultiplied upload
        # leaves the interior data intact — no straight-alpha handling needed. `height`
        # is None under --pack-only (no integrated relief), so surface needs a real compute.
        if args.surface and height is not None:
            h8 = (np.clip(height, 0.0, 1.0) * 255.0).round().astype(np.uint8)
            zero = np.zeros_like(h8)
            sil = np.where(mask, np.uint8(255), np.uint8(0))
            surf = np.dstack([h8, ao8, zero, sil])     # R=height, G=AO, B=open, A=silhouette
            Image.fromarray(surf, "RGBA").save(d.with_name("surface.png"))

        what = "surface.png" if args.surface else ("normal.a" if args.pack_only else "occlusion.png")
        tail = ""
        # Pack AO into the co-located normal's ALPHA (normal.rgb untouched). Alpha carries
        # AO * silhouette: outside the sprite -> 0 (uncovered, transparent); inside -> AO,
        # FLOORED to --ao-floor so it never hits 0. The floor matters because the client's
        # normal composite is PREMULTIPLIED — the shader recovers the normal as rgb/alpha,
        # which needs alpha > 0 (a fully-occluded pixel at alpha 0 is indistinguishable from
        # an uncovered one and would lose its normal). The standalone occlusion.png keeps
        # the true unfloored AO. See docs/lighting.md.
        if args.pack_normal and not args.surface:
            floored = np.where(mask, np.maximum(np.clip(ao, 0.0, 1.0), args.ao_floor), 0.0)
            packed_a = (floored * 255.0).round().astype(np.uint8)
            rgba = np.dstack([np.asarray(normal), packed_a])          # normal.rgb + AO alpha
            Image.fromarray(rgba, "RGBA").save(nrm_path)
            tail += " (+AO->normal.a)"
        if args.visualize and height is not None:   # height is None under --pack-only
            scale = max(1, 256 // max(normal.width, normal.height))
            h8 = (np.clip(height, 0.0, 1.0) * 255.0).round().astype(np.uint8)
            panels = [
                _panel(Image.open(d), "diffuse", scale),
                _panel(h8, "height (integrated)", scale),
                _panel(ao8, "AO (depth-derived)", scale),
            ]
            alb_path = d.with_name("albedo.png")
            if alb_path.exists():
                alb = np.asarray(Image.open(alb_path).convert("RGB")).astype(np.float64)
                lit = (alb * ao[..., None]).clip(0, 255).astype(np.uint8)
                panels.append(_panel(alb.astype(np.uint8), "albedo", scale))
                panels.append(_panel(lit, "albedo x AO", scale))
            gap = 8
            w = sum(p.width for p in panels) + gap * (len(panels) + 1)
            hgt = max(p.height for p in panels) + 2 * gap
            sheet = Image.new("RGB", (w, hgt), (0, 0, 0))
            x = gap
            for p in panels:
                sheet.paste(p, (x, gap))
                x += p.width + gap
            cp = d.with_name("occlusion-compare.png")
            sheet.save(cp)
            tail += f" (+{cp.name})"

        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.parent.name} -> {what}{tail}", flush=True)

    skip = f" ({missing} skipped: no normal)" if missing else ""
    print(f"depth_ao: wrote {ok} AO map(s) in {time.time() - t0:.1f}s{skip}", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
