#!/usr/bin/env python3
"""bin/art retile-linked — seam-inpaint a LINKED atlas's diffuse so cells truly tile.

(comfy-linked-tiling, 2026-07-29.) The autotile cells are authored/generated per cell, so
in-world-abutting edges don't perfectly continue (measured: the smooth wall's E|W seams
mean 2.07 / max 7.29 RGB, every worst pair sharing cell 1's west band). This tool fixes
the SOURCE:

  layouts   cells assembled as the D1 table actually places them in-world (runs, corners,
            Ts, cross — atlas order is NOT world adjacency), in client-sampled WINDOW
            units (the 8px pad ring is a bleed guard, refreshed after write-back)
  masks     the seam BANDS of the two canonical seam classes — E|W (horizontal run) and
            S|N (vertical run). Only TWO inpaints are ever needed: the D1 contract says
            every E-band must equal every other E-band (any E-cell can abut any W-cell),
            so one blended seam per class is made canonical and then STAMPED into every
            same-class band deterministically (F1 — frozen edges, convergent in one round)
  inpaint   masked low-denoise img2img on the ComfyUI box (P2; reuses generate.py's
            client). Outside-mask pixels are composited back through the mask, so
            bit-identity outside the bands is enforced, not assumed
  verify    marigold/atlas_check.py's ALBEDO SEAM section is the gate

Usage:
  retile_linked.py <kind-dir> --selftest          # P1: assemble→slice round-trip
  retile_linked.py <kind-dir> --dry               # emit layouts+masks, list planned work
  retile_linked.py <kind-dir> [--dn 0.35 ...]     # P2: the live pass (writes diffuse)
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
DIFFUSE = "diffuse.l.0.png"
PAD_UNITS = 1  # the DSL internal_padding for linked kinds (walls author 1)

# ── D1 (linkedCell.ts, verbatim semantics) ────────────────────────────────────────────

N, E, S, W = 1, 2, 4, 8


def linked_cell(mask: int) -> int:
    x = (1 if mask & N else 0) + (2 if mask & E else 0)
    y = 3 - ((1 if mask & S else 0) + (2 if mask & W else 0))
    return y * 4 + x


# In-world LAYOUTS: 2D wall shapes; each position's cell derives from its neighbours
# within the shape — exactly what the tile expansion does. Every one of the 16 cells
# appears in at least one layout, in real context.
LAYOUTS: dict[str, list[str]] = {
    "hrun":     ["####"],
    "vrun":     ["#", "#", "#", "#"],
    "corner-nw": ["##", "#."],
    "corner-ne": ["##", ".#"],
    "corner-sw": ["#.", "##"],
    "corner-se": [".#", "##"],
    "t-n":      ["###", ".#."],
    "t-s":      [".#.", "###"],
    "t-e":      ["#.", "##", "#."],
    "t-w":      [".#", "##", ".#"],
    "cross":    [".#.", "###", ".#."],
    "lone":     ["#"],
}


def layout_cells(shape: list[str]) -> list[tuple[int, int, int]]:
    """(gx, gy, cell) for every wall position in a shape."""
    rows, cols = len(shape), len(shape[0])
    has = lambda x, y: 0 <= x < cols and 0 <= y < rows and shape[y][x] == "#"
    out = []
    for gy in range(rows):
        for gx in range(cols):
            if not has(gx, gy):
                continue
            m = (N if has(gx, gy - 1) else 0) | (E if has(gx + 1, gy) else 0) \
              | (S if has(gx, gy + 1) else 0) | (W if has(gx - 1, gy) else 0)
            out.append((gx, gy, linked_cell(m)))
    return out


# ── geometry ──────────────────────────────────────────────────────────────────────────

class Geo:
    def __init__(self, kind_dir: Path):
        self.kind_dir = kind_dir
        aj = json.loads((kind_dir / "atlas.json").read_text()) if (kind_dir / "atlas.json").exists() else {}
        self.cols = int(aj.get("cols", 4))
        self.rows = int(aj.get("rows", 4))
        img = Image.open(kind_dir / DIFFUSE).convert("RGBA")
        self.atlas = np.asarray(img, dtype=np.uint8).copy()
        self.side = self.atlas.shape[0]
        self.cell = self.side // self.cols
        self.pad = int(round(PAD_UNITS * self.side / (16.0 * self.cols)))
        self.win = self.cell - 2 * self.pad

    def window(self, c: int) -> tuple[int, int]:
        """Atlas (y, x) of cell c's window origin."""
        return (c // self.cols) * self.cell + self.pad, (c % self.cols) * self.cell + self.pad


def assemble(g: Geo, shape: list[str]) -> tuple[np.ndarray, list[tuple[int, int, int]]]:
    """The layout image (window units, RGBA; empty = transparent) + its cell placements."""
    cells = layout_cells(shape)
    rows, cols = len(shape), len(shape[0])
    img = np.zeros((rows * g.win, cols * g.win, 4), dtype=np.uint8)
    for gx, gy, c in cells:
        wy, wx = g.window(c)
        img[gy * g.win : (gy + 1) * g.win, gx * g.win : (gx + 1) * g.win] = \
            g.atlas[wy : wy + g.win, wx : wx + g.win]
    return img, cells


def slice_back(g: Geo, shape: list[str], img: np.ndarray, into: np.ndarray,
               only_cells: set[int] | None = None) -> None:
    """Write a layout's windows back into an atlas array (first occurrence wins)."""
    seen: set[int] = set()
    for gx, gy, c in layout_cells(shape):
        if c in seen or (only_cells is not None and c not in only_cells):
            continue
        seen.add(c)
        wy, wx = g.window(c)
        into[wy : wy + g.win, wx : wx + g.win] = \
            img[gy * g.win : (gy + 1) * g.win, gx * g.win : (gx + 1) * g.win]


def seam_masks(g: Geo, shape: list[str], band_units: float = 2.0) -> tuple[np.ndarray, list[str]]:
    """The seam-band mask for a layout (255 = inpaintable) + the seam classes it contains.

    A seam = two abutting wall positions; its band spans `band_units` (of 16 per window)
    on EACH side. Classes: 'EW' (horizontal neighbour), 'SN' (vertical)."""
    cells = {(gx, gy) for gx, gy, _ in layout_cells(shape)}
    rows, cols = len(shape), len(shape[0])
    mask = np.zeros((rows * g.win, cols * g.win), dtype=np.uint8)
    band = max(2, int(round(band_units * g.win / 16.0)))
    classes: list[str] = []
    for gx, gy in sorted(cells):
        if (gx + 1, gy) in cells:  # E|W seam at the boundary x = (gx+1)·win
            x = (gx + 1) * g.win
            mask[gy * g.win : (gy + 1) * g.win, x - band : x + band] = 255
            if "EW" not in classes:
                classes.append("EW")
        if (gx, gy + 1) in cells:  # S|N seam at the boundary y = (gy+1)·win
            y = (gy + 1) * g.win
            mask[y - band : y + band, gx * g.win : (gx + 1) * g.win] = 255
            if "SN" not in classes:
                classes.append("SN")
    return mask, classes


def rebleed(g: Geo) -> None:
    """Refresh each cell's pad ring by replicating its window edges (the bleed guard —
    a server downscale must never pull a neighbour cell across)."""
    a = g.atlas
    for c in range(g.cols * g.rows):
        wy, wx = g.window(c)
        y0, x0 = wy - g.pad, wx - g.pad
        cellblk = a[y0 : y0 + g.cell, x0 : x0 + g.cell]
        win = cellblk[g.pad : g.pad + g.win, g.pad : g.pad + g.win]
        cellblk[: g.pad, g.pad : g.pad + g.win] = win[:1]
        cellblk[g.pad + g.win :, g.pad : g.pad + g.win] = win[-1:]
        cellblk[:, : g.pad] = cellblk[:, g.pad : g.pad + 1]
        cellblk[:, g.pad + g.win :] = cellblk[:, g.pad + g.win - 1 : g.pad + g.win]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("kind_dir", type=Path)
    ap.add_argument("--selftest", action="store_true", help="assemble→slice round-trip (must be bit-identical)")
    ap.add_argument("--dry", action="store_true", help="emit layouts+masks to --out-dir, change nothing")
    ap.add_argument("--out-dir", type=Path, default=None, help="where --dry emits (default: CWD ./retile-dry)")
    ap.add_argument("--dn", type=float, default=0.35, help="inpaint denoise (P2)")
    ap.add_argument("--band-units", type=float, default=2.0, help="seam band half-width per side (units of 16)")
    args = ap.parse_args()

    g = Geo(args.kind_dir)
    print(f"retile-linked: {args.kind_dir}  ({g.side}px, {g.cols}x{g.rows}, window {g.win}px, pad {g.pad}px)")

    if args.selftest:
        out = g.atlas.copy()
        covered: set[int] = set()
        for name, shape in LAYOUTS.items():
            img, cells = assemble(g, shape)
            slice_back(g, shape, img, out)
            covered.update(c for _, _, c in cells)
        ident = bool((out == g.atlas).all())
        print(f"  round-trip bit-identical: {ident}; cells covered by layouts: {sorted(covered)} ({len(covered)}/16)")
        return 0 if ident and len(covered) == 16 else 1

    if args.dry:
        od = args.out_dir or Path("retile-dry")
        od.mkdir(parents=True, exist_ok=True)
        frozen: set[str] = set()
        plan: list[str] = []
        for name, shape in LAYOUTS.items():
            img, cells = assemble(g, shape)
            mask, classes = seam_masks(g, shape, args.band_units)
            Image.fromarray(img, "RGBA").save(od / f"layout-{name}.png")
            if mask.any():
                Image.fromarray(mask, "L").save(od / f"mask-{name}.png")
            todo = [cl for cl in classes if cl not in frozen]
            if todo:
                frozen.update(todo)
                plan.append(f"INPAINT {name}: seam class(es) {todo} → canonical, then stamp atlas-wide")
        print(f"  layouts+masks → {od}/")
        for p in plan:
            print(f"  {p}")
        print(f"  (remaining layouts verify only — every seam class is frozen after the above)")
        return 0

    print("retile-linked: live pass lands with P2 (--dry / --selftest only for now)", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
