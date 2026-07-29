#!/usr/bin/env python3
"""Post-alignment for per-cell linked-atlas normals (marigold-linked-normals P2/P3).

Pure numpy — no model, no venv requirement. Two passes, each A/B-able against
`atlas_check.py` numbers:

  align_atlas       P2 — put every cell in the ONE canonical frame:
                    (a) rotate each cell so its dominant (flat-top) cluster's mean
                        lands exactly on +Z (estimated over the client-sampled
                        window, applied to the whole cell);
                    (b) gentle bevel-gain match: scale each cell's tangential (x,y)
                        slope magnitude toward the atlas median over non-flat
                        pixels (clamped ±25 % — content stays content).

  symmetrize_atlas  P3 — make equivalent pieces IDENTICAL and seams exact:
                    (a) arm averaging: an arm (N/S/E/W run entering the hub) is the
                        same piece wherever it appears — average it across every
                        cell that has it (D1 bits) and write the average back.
                        E and W are mirror-equivalent (side runs) and unify under
                        the mirror transform (flip columns AND negate normal-x);
                        N and S stay separate (the oblique art shows different
                        faces). Hubs are NOT averaged — each connectivity class is
                        genuinely different art.
                    (b) seam enforcement: all E|W adjacencies share one canonical
                        edge cross-section (as do S|N); each member's edge is set
                        to it and feathered ~half a unit inward.

Geometry matches atlas_check.py: cells sliced by the client's sampled window
(cell − 2·internal_padding units); the D1 bits name which arms a cell has.
"""
from __future__ import annotations

import numpy as np

from atlas_check import ang_deg, cell_bits, decode, flat_cluster_mean

PAD_UNITS = 1.0  # the DSL internal_padding for linked kinds today (walls author 1)


def _encode(n: np.ndarray) -> np.ndarray:
    return np.clip(np.round((n + 1.0) * 127.5), 0, 255).astype(np.uint8)


def _rot_to_z(mean: np.ndarray) -> np.ndarray:
    """The rotation matrix taking unit vector `mean` onto +Z (Rodrigues)."""
    z = np.array([0.0, 0.0, 1.0])
    v = np.cross(mean, z)
    s = np.linalg.norm(v)
    c = float(np.dot(mean, z))
    if s < 1e-9:
        return np.eye(3) if c > 0 else np.diag([1.0, -1.0, -1.0])
    vx = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + vx + vx @ vx * ((1 - c) / (s * s))


def _geom(shape: int, cols: int) -> tuple[int, int, int]:
    cell = shape // cols
    pad = int(round(PAD_UNITS * (shape / (16.0 * cols))))
    return cell, pad, cell - 2 * pad


def align_atlas(atlas_u8: np.ndarray, rgba: np.ndarray, cols: int, rows: int) -> np.ndarray:
    from PIL import Image

    n = decode(Image.fromarray(atlas_u8, "RGB"))
    alpha = rgba[:, :, 3] > 16
    cell, pad, win = _geom(atlas_u8.shape[0], cols)

    # (a) per-cell flat-frame rotation to +Z
    for c in range(cols * rows):
        cx, cy = (c % cols) * cell, (c // cols) * cell
        wn = n[cy + pad : cy + pad + win, cx + pad : cx + pad + win]
        wm = alpha[cy + pad : cy + pad + win, cx + pad : cx + pad + win]
        mean, _ = flat_cluster_mean(wn, wm)
        R = _rot_to_z(mean)
        blk = n[cy : cy + cell, cx : cx + cell]
        m = alpha[cy : cy + cell, cx : cx + cell]
        blk[m] = blk[m] @ R.T
        n[cy : cy + cell, cx : cx + cell] = blk

    # (b) bevel-gain match toward the atlas median (non-flat pixels only, clamped)
    up = np.array([0.0, 0.0, 1.0])
    mags = []
    sel_nonflat = []
    for c in range(cols * rows):
        cx, cy = (c % cols) * cell, (c // cols) * cell
        blk = n[cy : cy + cell, cx : cx + cell]
        m = alpha[cy : cy + cell, cx : cx + cell]
        nonflat = m & (ang_deg(blk, up[None, None, :]) > 15.0)
        sel_nonflat.append(nonflat)
        t = np.linalg.norm(blk[..., :2], axis=-1)
        mags.append(float(t[nonflat].mean()) if nonflat.any() else 0.0)
    ref = float(np.median([v for v in mags if v > 0]) or 0)
    if ref > 0:
        for c in range(cols * rows):
            if mags[c] <= 0:
                continue
            # BOOST-ONLY (user, 2026-07-29): shrinking a strong cell toward the median
            # discards exactly the relief the walls need — gain only lifts starved cells.
            g = float(np.clip(ref / mags[c], 1.0, 1.25))
            if abs(g - 1.0) < 1e-3:
                continue
            cx, cy = (c % cols) * cell, (c // cols) * cell
            blk = n[cy : cy + cell, cx : cx + cell]
            nf = sel_nonflat[c]
            blk[nf, 0] *= g
            blk[nf, 1] *= g
            ln = np.linalg.norm(blk[nf], axis=-1, keepdims=True)
            blk[nf] = blk[nf] / np.maximum(ln, 1e-9)
            n[cy : cy + cell, cx : cx + cell] = blk
    return _encode(n)


def symmetrize_atlas(atlas_u8: np.ndarray, rgba: np.ndarray, cols: int, rows: int) -> np.ndarray:
    from PIL import Image

    n = decode(Image.fromarray(atlas_u8, "RGB"))
    alpha = rgba[:, :, 3] > 16
    cell, pad, win = _geom(atlas_u8.shape[0], cols)
    t0, t1 = win // 3, win - win // 3

    def wslice(c: int):
        cx, cy = (c % cols) * cell + pad, (c // cols) * cell + pad
        return (slice(cy, cy + win), slice(cx, cx + win))

    regions = {
        "N": (slice(0, t0), slice(t0, t1), 0),
        "S": (slice(t1, win), slice(t0, t1), 2),
        "E": (slice(t0, t1), slice(t1, win), 1),
        "W": (slice(t0, t1), slice(0, t0), 3),
    }

    def members(bit: int) -> list[int]:
        return [c for c in range(cols * rows) if cell_bits(c, cols)[bit] == 1]

    def region_of(c: int, rs, cs):
        wy, wx = wslice(c)
        return (slice(wy.start + rs.start, wy.start + rs.stop),
                slice(wx.start + cs.start, wx.start + cs.stop))

    def unit(v: np.ndarray) -> np.ndarray:
        ln = np.linalg.norm(v, axis=-1, keepdims=True)
        return np.divide(v, ln, out=np.zeros_like(v), where=ln > 1e-9)

    # (a) arm STAMPING (user, 2026-07-29 — averaging across cells blurred the relief the
    # walls need: real per-pixel content variance is detail, not noise). Each arm is
    # stamped from a DONOR — the pure-run cell, whose arm is the cleanest instance of the
    # piece (N|S run for vertical arms, E|W run for horizontal) — so every member carries
    # the donor's FULL detail and the spread is 0 by construction. E and W share one donor
    # under the mirror transform (flip columns, negate normal-x).
    def donor(mem: list[int], want_bits: tuple[int, int, int, int]) -> int:
        for c in mem:
            if cell_bits(c, cols) == want_bits:
                return c
        return mem[0]

    for name, run_bits in (("N", (1, 0, 1, 0)), ("S", (1, 0, 1, 0))):
        rs, cs, bit = regions[name]
        mem = members(bit)
        if len(mem) < 2:
            continue
        d = donor(mem, run_bits)
        stamp = n[region_of(d, rs, cs)].copy()
        ok = alpha[region_of(d, rs, cs)] & np.stack([alpha[region_of(c, rs, cs)] for c in mem]).all(axis=0)
        for c in mem:
            if c == d:
                continue
            ry, rx = region_of(c, rs, cs)
            blk = n[ry, rx]
            blk[ok] = stamp[ok]
            n[ry, rx] = blk
    rsE, csE, bitE = regions["E"]
    rsW, csW, bitW = regions["W"]
    memE, memW = members(bitE), members(bitW)
    if memE and memW:
        def mirror(v: np.ndarray) -> np.ndarray:
            out = v[:, ::-1].copy()
            out[..., 0] = -out[..., 0]
            return out
        d = donor(memE, (0, 1, 0, 1))
        stamp = n[region_of(d, rsE, csE)].copy()
        ok = (alpha[region_of(d, rsE, csE)]
              & np.stack([alpha[region_of(c, rsE, csE)] for c in memE]).all(axis=0)
              & np.stack([alpha[region_of(c, rsW, csW)][:, ::-1] for c in memW]).all(axis=0))
        for c in memE:
            if c == d:
                continue
            ry, rx = region_of(c, rsE, csE)
            blk = n[ry, rx]
            blk[ok] = stamp[ok]
            n[ry, rx] = blk
        stampW, okW = mirror(stamp), ok[:, ::-1]
        for c in memW:
            ry, rx = region_of(c, rsW, csW)
            blk = n[ry, rx]
            blk[okW] = stampW[okW]
            n[ry, rx] = blk

    # (b) seam enforcement: one canonical cross-section per axis, feathered inward.
    feather = max(2, int(round((cell / 16.0) / 2)))  # ~half a unit
    for axis, (bit_a, bit_b) in {"EW": (1, 3), "SN": (2, 0)}.items():
        mem_a, mem_b = members(bit_a), members(bit_b)
        if not mem_a or not mem_b:
            continue
        strips = []
        for c in mem_a:  # A's far edge (east or south)
            wy, wx = wslice(c)
            strips.append(n[wy.start + t0 : wy.start + t1, wx.stop - 1] if axis == "EW"
                          else n[wy.stop - 1, wx.start + t0 : wx.start + t1])
        for c in mem_b:  # B's near edge (west or north)
            wy, wx = wslice(c)
            strips.append(n[wy.start + t0 : wy.start + t1, wx.start] if axis == "EW"
                          else n[wy.start, wx.start + t0 : wx.start + t1])
        canon = unit(np.stack(strips).mean(axis=0))
        for c, near in [(c, False) for c in mem_a] + [(c, True) for c in mem_b]:
            wy, wx = wslice(c)
            for f in range(feather):
                w = 1.0 - f / feather  # 1 at the edge → toward own content inward
                if axis == "EW":
                    col = wx.start + f if near else wx.stop - 1 - f
                    seg = n[wy.start + t0 : wy.start + t1, col]
                    n[wy.start + t0 : wy.start + t1, col] = unit(canon * w + seg * (1 - w))
                else:
                    row = wy.start + f if near else wy.stop - 1 - f
                    seg = n[row, wx.start + t0 : wx.start + t1]
                    n[row, wx.start + t0 : wx.start + t1] = unit(canon * w + seg * (1 - w))
    return _encode(n)
