#!/usr/bin/env python3
"""Analytic normals for REGULAR linked walls (analytic-wall-normals P1).

The smooth wall is a beveled prism — its true normal map is computable, so no model
guesses at it. Per cell: every UNCONNECTED side (D1 bits) carries an edge treatment
measured from the drawn art (`geometry.json`, fitted in P0):

    north edge : outline | (ramp) | top face          — the back edge, seen from above
    south edge : outline | front face | (ramp) | top  — the visible south face
    east/west  : outline | side face  | (ramp) | top  — the run's flanks

A pixel takes the treatment of the NEAREST unconnected side (ties feathered), connected
sides contribute nothing (the wall continues through the window edge), and everything
else is the flat top. Band normals are view-space (+X right, +Y up, +Z at the viewer —
the corpus convention normals.py documents), pitched by the authored angles; encoding
0.5·n+0.5 over RGB. Consistency across cells is EXACT by construction: the same profile
sweeps every cell, so seams, piece identity, and the flat frame need no post pass.

Usage:
  atlas_geometry.py <kind-dir>            # write normal.l.0.png
  atlas_geometry.py <kind-dir> --check    # register check vs the art (no write)
  atlas_geometry.py <kind-dir> --out P    # write elsewhere (A/B)
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from atlas_check import cell_bits

DIFFUSE = "diffuse.l.0.png"
NORMAL = "normal.l.0.png"
PAD_UNITS = 1


def band_normals(gm: dict) -> dict[str, np.ndarray]:
    fr = np.radians(gm["front_pitch_deg"])
    sd = np.radians(gm["side_pitch_deg"])
    no = np.radians(gm["north_pitch_deg"])
    return {
        "top": np.array([0.0, 0.0, 1.0]),
        "front": np.array([0.0, -np.sin(fr), np.cos(fr)]),
        "east": np.array([np.sin(sd), 0.0, np.cos(sd)]),
        "west": np.array([-np.sin(sd), 0.0, np.cos(sd)]),
        "north": np.array([0.0, np.sin(no), np.cos(no)]),
    }


def _dilate(mask: np.ndarray, px: int) -> np.ndarray:
    out = mask.copy()
    for _ in range(px):
        grown = out.copy()
        grown[1:, :] |= out[:-1, :]
        grown[:-1, :] |= out[1:, :]
        grown[:, 1:] |= out[:, :-1]
        grown[:, :-1] |= out[:, 1:]
        out = grown
    return out


def _dist_along(fp: np.ndarray, axis: int, reverse: bool) -> np.ndarray:
    """Per pixel: distance to the nearest footprint pixel scanning backward along `axis`
    (i.e. how far this pixel sits past the footprint's boundary in that direction).
    BIG where no footprint precedes it."""
    BIG = 10_000
    a = fp if not reverse else np.flip(fp, axis=axis)
    d = np.full(a.shape, BIG, dtype=np.int32)
    n = a.shape[axis]
    run = np.full(a.shape[1 - axis], BIG, dtype=np.int32)
    for i in range(n):
        sl = np.take(a, i, axis=axis)
        run = np.where(sl, 0, np.minimum(run + 1, BIG))
        if axis == 0:
            d[i, :] = run
        else:
            d[:, i] = run
    return d if not reverse else np.flip(d, axis=axis)


def build_cell(bits: tuple[int, int, int, int], win: int, gm: dict, tops: dict) -> np.ndarray:
    """The cell's normal field from its wall FOOTPRINT (the model the art actually uses —
    measured 2026-07-29): the body is a BAND, not the full square. E/W arms occupy the
    northern rows (the south FACE draws BELOW the body in screen space, oblique); N/S arms
    occupy the middle columns (side faces flank them); the hub is their intersection
    block. Faces attach to the footprint's exposed boundaries; outline rings everything."""
    n, e, s, w = bits
    o, sd, fr, t = gm["outline_px"], gm["side_px"], gm["front_px"], gm["transition_px"]
    body_s = win - (fr + t + o)     # E/W body rows o..body_s (art: 13..64)
    col_w, col_e = o + sd, win - o - sd  # N/S body cols (art: 29..83)

    fp = np.zeros((win, win), bool)
    fp[o:body_s, col_w:col_e] = True                     # hub block
    if n:
        fp[0:body_s, col_w:col_e] = True
    if s:
        fp[o:win, col_w:col_e] = True
    if w:
        fp[o:body_s, 0:col_e] = True
    if e:
        fp[o:body_s, col_w:win] = True

    d_below = _dist_along(fp, axis=0, reverse=False)     # px below the body's south boundary
    d_right = _dist_along(fp, axis=1, reverse=False)     # px right of a body's east boundary
    d_left = _dist_along(fp, axis=1, reverse=True)       # px left of a body's west boundary

    # ── the ANGLED corner model (the art's own geometry, measured 2026-07-29) ─────────
    # The front-face APRON hangs below the body and SPLAYS: its free ends widen at 45°
    # over the flanking side bands (the prism's south face drawn in oblique — the side
    # bands' visible width shrinks with depth until the face meets the outline).
    #  · CONCAVE (a T corner): a perpendicular arm stands at apron rows; the arm's side
    #    face claims the 45° wedge NEARER the arm than the body above (it widens downward
    #    alongside the arm), the front face keeps the rest.
    #  · CONVEX (a lone/end corner): the front face GAINS band pixels within 45° of the
    #    flanked column's body corner (the splay), the side band keeps what's beyond.
    # `A` = fp OR anything above it in the column (the silhouette column set).
    A = np.logical_or.accumulate(fp, axis=0)
    dA_right = _dist_along(A, axis=1, reverse=False)     # px right of the silhouette
    dA_left = _dist_along(A, axis=1, reverse=True)       # px left of the silhouette

    apron = ~fp & (d_below > 0) & (d_below <= fr + t)
    concave_e = apron & (d_left > 0) & (d_left <= sd + t) & (d_left < d_below)    # arm to the EAST
    concave_w = apron & (d_right > 0) & (d_right <= sd + t) & (d_right < d_below)  # arm to the WEST
    front = apron & ~(concave_e | concave_w)

    bandE = (~A) & (dA_right > 0) & (dA_right <= sd + t)
    bandW = (~A) & (dA_left > 0) & (dA_left <= sd + t)
    # The splay: depth below the FLANKED silhouette column's body end, at 45°.
    fp_end = np.where(fp.any(axis=0), win - 1 - np.argmax(fp[::-1, :], axis=0), -10_000)
    xs = np.arange(win)[None, :]
    ys = np.arange(win)[:, None]
    srcE = np.clip(xs - dA_right, 0, win - 1)
    depthE = ys - fp_end[srcE]
    gainE = bandE & (depthE > 0) & (dA_right <= depthE) & (depthE <= fr + t)
    srcW = np.clip(xs + dA_left, 0, win - 1)
    depthW = ys - fp_end[srcW]
    gainW = bandW & (depthW > 0) & (dA_left <= depthW) & (depthW <= fr + t)
    front = front | gainE | gainW
    east = (bandE & ~gainE) | concave_w
    west = (bandW & ~gainW) | concave_e

    out = np.tile(tops["top"], (win, win, 1))
    wsum = np.ones((win, win))
    out[~fp] = 0.0
    wsum[~fp] = 0.0

    def face_depth(key: str) -> np.ndarray:
        """The ramp driver per face: its own directional distance, capped by its reach."""
        if key == "front":
            d = np.where(gainE, depthE, np.where(gainW, depthW, d_below))
            return d, fr
        if key == "east":
            return np.where(concave_w, d_right, dA_right), sd
        return np.where(concave_e, d_left, dA_left), sd

    for mask, key in ((front, "front"), (east, "east"), (west, "west")):
        d, reach = face_depth(key)
        ramp = np.where(mask, np.clip((reach + t - d) / max(t, 1), 0.0, 1.0), 0.0)
        # face·ramp + top·(1−ramp) INSIDE the mask — the ramp genuinely fades to the top
        # (weights must sum to 1 there, or normalisation cancels the fade).
        out += tops[key][None, None, :] * ramp[..., None] \
             + tops["top"][None, None, :] * np.where(mask, 1.0 - ramp, 0.0)[..., None]
        wsum += np.where(mask, 1.0, 0.0)

    # Outline: rings the silhouette (footprint + faces). Inherits the nearest face's tilt
    # cheaply by ONE more dilation pass carrying the current field outward.
    covered = fp | front | east | west
    ring = _dilate(covered, o) & ~covered
    field_src = out.copy()
    w_src = wsum.copy()
    for _ in range(o):
        for shift in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            sy, sx = shift
            src_f = np.roll(field_src, (sy, sx), axis=(0, 1))
            src_w = np.roll(w_src, (sy, sx), axis=(0, 1))
            take = ring & (w_src == 0) & (src_w > 0)
            field_src[take] = src_f[take]
            w_src[take] = src_w[take]
    out[ring & (wsum == 0)] = field_src[ring & (wsum == 0)]
    wsum[ring & (w_src > 0) & (wsum == 0)] = 1.0

    flat = wsum == 0
    out[flat] = tops["top"]
    wsum[flat] = 1.0
    out = out / wsum[..., None]
    ln = np.linalg.norm(out, axis=-1, keepdims=True)
    return out / np.maximum(ln, 1e-9)


def generate(kind_dir: Path) -> np.ndarray:
    gm = json.loads((kind_dir / "geometry.json").read_text())
    src = Image.open(kind_dir / DIFFUSE).convert("RGBA")
    side = src.size[0]
    scale = side / gm["atlas_side"]
    g = {k: (v * scale if k.endswith("_px") else v) for k, v in gm.items() if isinstance(v, (int, float))}
    cols = rows = 4
    cell = side // cols
    pad = int(round(PAD_UNITS * side / (16.0 * cols)))
    win = cell - 2 * pad
    tops = band_normals(gm)
    atlas = np.zeros((side, side, 3), dtype=np.uint8)
    atlas[:, :] = (128, 128, 255)
    for c in range(cols * rows):
        f = build_cell(cell_bits(c, cols), win, {k: int(round(v)) if isinstance(v, float) and k.endswith("_px") else v for k, v in g.items()}, tops)
        enc = np.clip(np.round((f + 1.0) * 127.5), 0, 255).astype(np.uint8)
        cy, cx = (c // cols) * cell + pad, (c % cols) * cell + pad
        atlas[cy : cy + win, cx : cx + win] = enc
        # pad ring: replicate window edges (the bleed guard)
        atlas[cy - pad : cy, cx : cx + win] = enc[:1]
        atlas[cy + win : cy + win + pad, cx : cx + win] = enc[-1:]
        atlas[cy - pad : cy + win + pad, cx - pad : cx] = atlas[cy - pad : cy + win + pad, cx : cx + 1]
        atlas[cy - pad : cy + win + pad, cx + win : cx + win + pad] = atlas[cy - pad : cy + win + pad, cx + win - 1 : cx + win]
    return atlas


def register_check(kind_dir: Path) -> int:
    """The profile's band boundaries vs the ART's luminance steps on the two run cells."""
    gm = json.loads((kind_dir / "geometry.json").read_text())
    d = np.asarray(Image.open(kind_dir / DIFFUSE).convert("RGB"), dtype=np.float64)
    side = d.shape[0]
    scale = side / gm["atlas_side"]
    cell = side // 4
    pad = int(round(PAD_UNITS * side / 64.0))
    win = cell - 2 * pad

    def window(c):
        cx, cy = (c % 4) * cell + pad, (c // 4) * cell + pad
        return d[cy : cy + win, cx : cx + win]

    def steps(profile):
        g = np.abs(np.diff(profile))
        return [i for i in range(1, len(g)) if g[i] > 12 and g[i] >= g[i - 1]]

    o = gm["outline_px"] * scale
    fr = gm["front_px"] * scale
    t = gm["transition_px"] * scale
    sd = gm["side_px"] * scale
    ok = True
    # cell 6 (E|W run): expect steps near row o (outline→top) and rows win−o−fr−t / win−o (front band)
    prof = window(6).mean(axis=(1, 2))
    got = steps(prof)
    for want, label in ((o, "north outline→top"), (win - o - fr - t, "top→front ramp"), (win - o, "front→south outline")):
        best = min(got, key=lambda s: abs(s - want)) if got else -99
        off = best - want
        print(f"  cell 6 {label:22s} want {want:6.1f} art {best:4d}  off {off:+.1f}px")
        ok &= abs(off) <= 1.0
    # cell 9 (N|S run): side faces at cols o..o+sd both flanks
    prof = window(9).mean(axis=(0, 2))
    got = steps(prof)
    for want, label in ((o, "west outline→face"), (o + sd, "west face→top"), (win - o - sd, "top→east face"), (win - o, "east face→outline")):
        best = min(got, key=lambda s: abs(s - want)) if got else -99
        off = best - want
        print(f"  cell 9 {label:22s} want {want:6.1f} art {best:4d}  off {off:+.1f}px")
        ok &= abs(off) <= 1.0
    print(f"register: {'OK (all ≤1px)' if ok else 'MISALIGNED'}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("kind_dir", type=Path)
    ap.add_argument("--check", action="store_true", help="register check only (no write)")
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()
    if args.check:
        return register_check(args.kind_dir)
    atlas = generate(args.kind_dir)
    op = args.out or (args.kind_dir / NORMAL)
    Image.fromarray(atlas, "RGB").save(op)
    print(f"atlas_geometry: wrote {op}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
