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

    return live_pass(g, args)


# ── P2: the ComfyUI seam-inpaint pass ─────────────────────────────────────────────────

UPSCALE = 4  # SDXL wants a real canvas; 112px windows upscale 4× for inpaint, then back
POS_PROMPT = ("stone wall, top-down 2D game tile texture, flat cel shading, hand-painted "
              "game art, one continuous seamless wall surface, consistent lighting")


def _inpaint_layout(g: Geo, name: str, shape: list[str], dn: float, band_units: float, seed: int) -> np.ndarray:
    """Masked low-denoise img2img of a layout's seam bands on the ComfyUI box. Returns the
    layout RGBA with ONLY masked pixels replaced (outside-mask identity by composite)."""
    import generate as G  # the proven client plumbing (COMFYUI_URL, upload/run/fetch)
    import io

    img, _cells = assemble(g, shape)
    mask, _classes = seam_masks(g, shape, band_units)
    h, w = img.shape[:2]
    # Transparent empties composite over a neutral dark backdrop — the seams sit BETWEEN
    # wall windows, so the backdrop is context only.
    rgb = img[:, :, :3].astype(np.float64)
    a = (img[:, :, 3:4].astype(np.float64)) / 255.0
    back = np.array([48.0, 48.0, 44.0])
    flat = (rgb * a + back * (1 - a)).astype(np.uint8)

    big = Image.fromarray(flat, "RGB").resize((w * UPSCALE, h * UPSCALE), Image.LANCZOS)
    bigmask = Image.fromarray(mask, "L").resize((w * UPSCALE, h * UPSCALE), Image.NEAREST)

    up_img = G._upload(big, f"retile-{name}.png")
    up_mask = G._upload(bigmask.convert("RGB"), f"retile-{name}-mask.png")

    graph = {
        "1": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": G.MODEL}},
        "2": {"class_type": "CLIPTextEncode", "inputs": {"text": POS_PROMPT, "clip": ["1", 1]}},
        "3": {"class_type": "CLIPTextEncode", "inputs": {"text": G.GENERIC_NEG, "clip": ["1", 1]}},
        "4": {"class_type": "LoadImage", "inputs": {"image": up_img}},
        "5": {"class_type": "LoadImage", "inputs": {"image": up_mask}},
        "6": {"class_type": "ImageToMask", "inputs": {"image": ["5", 0], "channel": "red"}},
        "7": {"class_type": "VAEEncode", "inputs": {"pixels": ["4", 0], "vae": ["1", 2]}},
        "8": {"class_type": "SetLatentNoiseMask", "inputs": {"samples": ["7", 0], "mask": ["6", 0]}},
        "10": {"class_type": "KSampler", "inputs": {"seed": seed, "steps": G.STEPS, "cfg": G.CFG,
                "sampler_name": G.SAMPLER, "scheduler": G.SCHED, "denoise": dn,
                "model": ["1", 0], "positive": ["2", 0], "negative": ["3", 0], "latent_image": ["8", 0]}},
        "11": {"class_type": "VAEDecode", "inputs": {"samples": ["10", 0], "vae": ["1", 2]}},
        "9": {"class_type": "SaveImage", "inputs": {"images": ["11", 0], "filename_prefix": f"retile-{name}-out"}},
    }
    data = G._run(graph)
    out_big = Image.open(io.BytesIO(data)).convert("RGB").resize((w, h), Image.LANCZOS)
    out = np.asarray(out_big, dtype=np.uint8)

    # Composite back THROUGH the mask — outside-mask bit-identity is enforced here.
    m = (mask > 0)[:, :, None]
    result = img.copy()
    result[:, :, :3] = np.where(m, out, img[:, :, :3])
    return result


def _stamp_bands(g: Geo, canon: dict[str, np.ndarray], band: int) -> None:
    """Stamp the frozen canonical bands into every same-class window edge, feathered over
    the inner half so the band meets each cell's interior smoothly. RGB only."""
    fe = max(1, band // 2)
    for c in range(g.cols * g.rows):
        x, y = c % g.cols, c // g.cols
        n_, e_ = x & 1, (x >> 1) & 1
        sw = 3 - y
        s_, w_ = sw & 1, (sw >> 1) & 1
        wy, wx = g.window(c)
        win = g.atlas[wy : wy + g.win, wx : wx + g.win]

        def blend(region: np.ndarray, stamp: np.ndarray, axis_from_edge: np.ndarray) -> np.ndarray:
            wgt = np.clip((band - axis_from_edge) / fe, 0.0, 1.0)[..., None]
            return (stamp * wgt + region * (1 - wgt)).astype(np.uint8)

        if e_ and "E" in canon:
            d = np.arange(band)[::-1]  # px from the east edge
            win[:, g.win - band :, :3] = blend(win[:, g.win - band :, :3].astype(np.float64),
                                               canon["E"].astype(np.float64), d[None, :])
        if w_ and "W" in canon:
            d = np.arange(band)
            win[:, :band, :3] = blend(win[:, :band, :3].astype(np.float64),
                                      canon["W"].astype(np.float64), d[None, :])
        if s_ and "S" in canon:
            d = np.arange(band)[::-1]
            win[g.win - band :, :, :3] = blend(win[g.win - band :, :, :3].astype(np.float64),
                                              canon["S"].astype(np.float64), d[:, None])
        if n_ and "N" in canon:
            d = np.arange(band)
            win[:band, :, :3] = blend(win[:band, :, :3].astype(np.float64),
                                      canon["N"].astype(np.float64), d[:, None])


def live_pass(g: Geo, args) -> int:
    band = max(2, int(round(args.band_units * g.win / 16.0)))
    before = g.atlas.copy()

    # ONE inpaint per canonical seam class (F1): hrun freezes E|W, vrun freezes S|N.
    print(f"retile-linked: inpainting hrun (E|W) + vrun (S|N) at dn {args.dn} ...", flush=True)
    hr = _inpaint_layout(g, "hrun", LAYOUTS["hrun"], args.dn, args.band_units, seed=2024)
    vr = _inpaint_layout(g, "vrun", LAYOUTS["vrun"], args.dn, args.band_units, seed=2024)

    # The canonical bands come from the MIDDLE seam (mid-run cell on both sides).
    canon: dict[str, np.ndarray] = {}
    x = 2 * g.win  # hrun seam between positions 1|2 (cells 6|6)
    canon["E"] = hr[:g.win, x - band : x, :3].copy()
    canon["W"] = hr[:g.win, x : x + band, :3].copy()
    y = 2 * g.win  # vrun seam between positions 1|2 (cells 9|9)
    canon["S"] = vr[y - band : y, :g.win, :3].copy()
    canon["N"] = vr[y : y + band, :g.win, :3].copy()

    _stamp_bands(g, canon, band)
    rebleed(g)

    # Outside-band identity check: the only pixels allowed to differ are the window edge
    # bands (the stamp) and the pad rings (the bleed guard, refreshed by design).
    changed = (g.atlas[:, :, :3] != before[:, :, :3]).any(axis=-1)
    inband = np.zeros_like(changed)
    for c in range(g.cols * g.rows):
        wy, wx = g.window(c)
        inband[wy : wy + g.win, wx : wx + band] = True
        inband[wy : wy + g.win, wx + g.win - band : wx + g.win] = True
        inband[wy : wy + band, wx : wx + g.win] = True
        inband[wy + g.win - band : wy + g.win, wx : wx + g.win] = True
        inband[wy - g.pad : wy, wx - g.pad : wx + g.win + g.pad] = True  # pad ring (rebleed)
        inband[wy + g.win : wy + g.win + g.pad, wx - g.pad : wx + g.win + g.pad] = True
        inband[wy : wy + g.win, wx - g.pad : wx] = True
        inband[wy : wy + g.win, wx + g.win : wx + g.win + g.pad] = True
    stray = int((changed & ~inband).sum())
    print(f"  outside-band pixels changed: {stray} (must be 0)")

    op = g.kind_dir / DIFFUSE
    Image.fromarray(g.atlas, "RGBA").save(op)
    print(f"retile-linked: wrote {op}")
    return 0 if stray == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
