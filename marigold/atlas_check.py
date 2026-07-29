#!/usr/bin/env python3
"""Consistency oracle for LINKED-atlas normal maps (marigold-linked-normals P0).

Measures the three ways a per-atlas Marigold inference drifts across the 16 autotile
cells, so alignment work is judged by numbers instead of eyeballs:

  flat   per-cell FLAT-frame error — the dominant (flat-top) normal cluster's mean,
         reported as angular deviation from +Z. Every cell's flat top should encode
         the SAME up vector.
  piece  same-piece deviation — the D1 cell semantics (x = N+2E, y = 3-(S+2W)) name
         which run arms a cell contains; equivalent arm regions across cells should
         carry equivalent normals. Reported as mean angular spread about the
         cross-cell mean, per piece, worst pair named.
  seam   in-world seam continuity — under autotiling, ANY E-connected cell can sit
         west of ANY W-connected cell (same N/S): the abutting edge strips of the
         sampled window should agree. Reported mean/max per pair class.

Geometry: the cell is `atlas_side/cols` px; the client samples the WINDOW inset by the
DSL internal_padding (units of a 16-unit cell -> px = side/(16*cols) per unit). The
checker slices by the SAME window the client samples (see TextureResolver.cellFrame).
Normals decode as OpenGL +Y-up view-space, [-1,1] over RGB (normals.py convention).

Usage: atlas_check.py <kind-dir|normal-atlas.png> [--pad-units 1] [--json]
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

MAPS = ("normal.l.0.png", "diffuse.l.0.png")


def cell_bits(cell: int, cols: int = 4) -> tuple[int, int, int, int]:
    """(N, E, S, W) connection bits for a row-major cell index — the D1 formula inverted."""
    x, y = cell % cols, cell // cols
    n, e = x & 1, (x >> 1) & 1
    sw = 3 - y
    s, w = sw & 1, (sw >> 1) & 1
    return n, e, s, w


def decode(img: Image.Image) -> np.ndarray:
    """H×W×3 unit normals from an RGB normal map (zero vectors stay zero)."""
    a = np.asarray(img.convert("RGB"), dtype=np.float64) / 127.5 - 1.0
    ln = np.linalg.norm(a, axis=2, keepdims=True)
    return np.divide(a, ln, out=np.zeros_like(a), where=ln > 1e-6)


def ang_deg(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """Per-pixel angle (degrees) between two normal arrays of the same shape."""
    d = np.clip(np.sum(a * b, axis=-1), -1.0, 1.0)
    return np.degrees(np.arccos(d))


def flat_cluster_mean(n: np.ndarray, mask: np.ndarray, radius_deg: float = 12.0) -> tuple[np.ndarray, float]:
    """The dominant normal cluster's mean (unit vector) + its pixel share.

    Iterated trim: start from the masked mean, keep pixels within `radius_deg` of the
    current mean, re-mean; 8 rounds converges on the dominant (flat) mode for wall art
    whose majority surface is the top face."""
    sel = mask.copy()
    if not sel.any():
        return np.array([0.0, 0.0, 1.0]), 0.0
    mean = n[sel].mean(axis=0)
    mean /= max(np.linalg.norm(mean), 1e-9)
    for _ in range(8):
        within = ang_deg(n, mean[None, None, :]) <= radius_deg
        keep = mask & within
        if not keep.any():
            break
        m = n[keep].mean(axis=0)
        ln = np.linalg.norm(m)
        if ln < 1e-9:
            break
        mean = m / ln
        sel = keep
    return mean, float(sel.sum()) / float(mask.sum())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", help="kind dir (with normal.l.0.png + diffuse.l.0.png) or a normal atlas png")
    ap.add_argument("--pad-units", type=float, default=1.0, help="DSL internal_padding (units of a 16-unit cell)")
    ap.add_argument("--cols", type=int, default=4)
    ap.add_argument("--rows", type=int, default=4)
    ap.add_argument("--json", action="store_true", help="machine-readable summary on stdout")
    args = ap.parse_args()

    p = Path(args.path)
    if p.is_dir():
        normal_path = p / MAPS[0]
        diffuse_path = p / MAPS[1]
    else:
        normal_path = p
        diffuse_path = p.parent / MAPS[1]
    if not normal_path.exists():
        print(f"atlas_check: no {normal_path}", file=sys.stderr)
        return 2

    normals = decode(Image.open(normal_path))
    side = normals.shape[0]
    cols, rows = args.cols, args.rows
    cell_px = side // cols
    unit_px = side / (16.0 * cols)
    pad_px = int(round(args.pad_units * unit_px))
    win = cell_px - 2 * pad_px  # the sampled window (what the client stretches over a tile)

    alpha = None
    if diffuse_path.exists():
        alpha = np.asarray(Image.open(diffuse_path).convert("RGBA"), dtype=np.uint8)[:, :, 3] > 16

    def window(cell: int, arr: np.ndarray) -> np.ndarray:
        cx, cy = (cell % cols) * cell_px, (cell // cols) * cell_px
        return arr[cy + pad_px : cy + pad_px + win, cx + pad_px : cx + pad_px + win]

    cells = [window(c, normals) for c in range(cols * rows)]
    masks = [window(c, alpha) if alpha is not None else np.ones((win, win), bool) for c in range(cols * rows)]

    # ── flat: per-cell dominant-cluster deviation from +Z ─────────────────────────────
    up = np.array([0.0, 0.0, 1.0])
    flat_rows, flat_means = [], []
    for c in range(cols * rows):
        mean, share = flat_cluster_mean(cells[c], masks[c])
        dev = float(ang_deg(mean, up))
        flat_rows.append((c, dev, share, mean))
        flat_means.append(mean)
    worst_flat = max(flat_rows, key=lambda r: r[1])
    # cross-cell frame spread: the flat means' deviation from THEIR OWN mean — the drift
    # that survives even if the whole atlas shares one (wrong) frame.
    fm = np.stack(flat_means)
    fmean = fm.mean(axis=0)
    fmean /= max(np.linalg.norm(fmean), 1e-9)
    flat_spread = [float(ang_deg(m, fmean)) for m in fm]

    # ── piece: same-arm angular spread across the cells that have the arm ─────────────
    # Regions in WINDOW fractions: the hub is the central third; each arm spans hub-width
    # across its half, outside the hub.
    t0, t1 = win // 3, win - win // 3
    regions = {
        "hub": (slice(t0, t1), slice(t0, t1)),
        "N": (slice(0, t0), slice(t0, t1)),
        "S": (slice(t1, win), slice(t0, t1)),
        "W": (slice(t0, t1), slice(0, t0)),
        "E": (slice(t0, t1), slice(t1, win)),
    }
    piece_rows = []
    worst_piece = ("", 0.0, -1, -1)
    for name, (rs, cs) in regions.items():
        has = [c for c in range(cols * rows)
               if name == "hub" or cell_bits(c, cols)[{"N": 0, "E": 1, "S": 2, "W": 3}[name]] == 1]
        if len(has) < 2:
            continue
        stack = np.stack([cells[c][rs, cs] for c in has])           # k×h×w×3
        mstack = np.stack([masks[c][rs, cs] for c in has])
        valid = mstack.all(axis=0)                                   # pixels present in EVERY member
        if not valid.any():
            continue
        mean = stack.mean(axis=0)
        ln = np.linalg.norm(mean, axis=-1, keepdims=True)
        mean = np.divide(mean, ln, out=np.zeros_like(mean), where=ln > 1e-9)
        devs = np.stack([ang_deg(stack[i], mean)[valid].mean() for i in range(len(has))])
        spread = float(devs.mean())
        piece_rows.append((name, spread, len(has)))
        for i in range(len(has)):
            for j in range(i + 1, len(has)):
                d = float(ang_deg(stack[i], stack[j])[valid].mean())
                if d > worst_piece[1]:
                    worst_piece = (name, d, has[i], has[j])

    # ── seam: every in-world-valid abutting strip pair ────────────────────────────────
    strip = max(2, int(round(unit_px / 2)))
    band = (slice(t0, t1),)  # compare along the run's cross-section band only

    def edge(cell: int, side_name: str) -> np.ndarray:
        w = cells[cell]
        if side_name == "E":
            return w[t0:t1, win - strip : win].mean(axis=1)
        if side_name == "W":
            return w[t0:t1, 0:strip].mean(axis=1)
        if side_name == "S":
            return w[win - strip : win, t0:t1].mean(axis=0)
        return w[0:strip, t0:t1].mean(axis=0)  # N

    def norm_rows(v: np.ndarray) -> np.ndarray:
        ln = np.linalg.norm(v, axis=-1, keepdims=True)
        return np.divide(v, ln, out=np.zeros_like(v), where=ln > 1e-9)

    seam_stats = {}
    worst_seam = ("", 0.0, -1, -1)
    for axis, (bit_a, side_a, bit_b, side_b) in {
        "E|W": (1, "E", 3, "W"),   # A's east edge meets B's west edge
        "S|N": (2, "S", 0, "N"),   # A's south edge meets B's north edge
    }.items():
        av = [c for c in range(cols * rows) if cell_bits(c, cols)[bit_a] == 1]
        bv = [c for c in range(cols * rows) if cell_bits(c, cols)[bit_b] == 1]
        diffs = []
        for a in av:
            ea = norm_rows(edge(a, side_a))
            for b in bv:
                eb = norm_rows(edge(b, side_b))
                d = ang_deg(ea, eb)
                dm = float(d.mean())
                diffs.append(dm)
                if dm > worst_seam[1]:
                    worst_seam = (axis, dm, a, b)
        if diffs:
            seam_stats[axis] = (float(np.mean(diffs)), float(np.max(diffs)), len(diffs))

    # ── relief: detail, measured AGAINST EACH CELL'S OWN FLAT FRAME ───────────────────
    # (vs +Z would count global tilt as detail; the per-cell-inference regression that
    # flattened the walls read 23%→2% on THIS metric while frame metrics looked great.)
    relief_shares = []
    for c in range(cols * rows):
        mean, _ = flat_cluster_mean(cells[c], masks[c])
        rel = ang_deg(cells[c], mean[None, None, :])[masks[c]]
        relief_shares.append(float((rel > 15.0).mean()) if rel.size else 0.0)
    relief = float(np.mean(relief_shares))

    # ── report ────────────────────────────────────────────────────────────────────────
    summary = {
        "relief_share_gt15": relief,
        "flat_worst_dev_deg": worst_flat[1],
        "flat_mean_dev_deg": float(np.mean([r[1] for r in flat_rows])),
        "flat_frame_spread_deg": float(np.mean(flat_spread)),
        "piece": {n: s for n, s, _ in piece_rows},
        "piece_worst": {"piece": worst_piece[0], "deg": worst_piece[1],
                        "cells": [worst_piece[2], worst_piece[3]]},
        "seam": {k: {"mean": v[0], "max": v[1], "pairs": v[2]} for k, v in seam_stats.items()},
    }
    if args.json:
        print(json.dumps(summary, indent=2))
        return 0

    print(f"atlas_check: {normal_path}  ({side}x{side}, {cols}x{rows} cells, window {win}px, pad {pad_px}px)")
    print(f"\nRELIEF (share of pixels >15° from the cell's OWN flat frame — detail, tilt-immune)")
    print(f"  mean {relief:.1%}   (healthy wall art ≈ 20-35%; a starved/flattened run reads <5%)")
    print("\nFLAT (per-cell dominant cluster vs +Z; spread = vs the atlas's own mean frame)")
    print("  cell  bits(NESW)  dev(+Z)°  spread°  share")
    for c, dev, share, _ in flat_rows:
        b = cell_bits(c, cols)
        print(f"   {c:2d}   {b[0]}{b[1]}{b[2]}{b[3]}        {dev:7.2f}  {flat_spread[c]:6.2f}  {share:5.0%}")
    print(f"  worst cell {worst_flat[0]} at {worst_flat[1]:.2f}°; mean {summary['flat_mean_dev_deg']:.2f}°; frame spread {summary['flat_frame_spread_deg']:.2f}°")
    print("\nPIECE (same-arm spread about the cross-cell mean)")
    for n, s, k in piece_rows:
        print(f"  {n:>3}: {s:6.2f}°   ({k} cells)")
    print(f"  worst pair: {worst_piece[0]} cells {worst_piece[2]} vs {worst_piece[3]} at {worst_piece[1]:.2f}°")
    print("\nSEAM (abutting window edges under every valid in-world adjacency)")
    for k, (m, mx, n_pairs) in seam_stats.items():
        print(f"  {k}: mean {m:6.2f}°  max {mx:6.2f}°   ({n_pairs} pairs)")
    print(f"  worst pair: {worst_seam[0]} cells {worst_seam[2]}|{worst_seam[3]} at {worst_seam[1]:.2f}°")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
