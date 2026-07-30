#!/usr/bin/env python3
"""Marigold normals for LINKED (grid) atlases — per-cell, deterministic, aligned.

The whole-atlas path (normals.py) feeds the 4×4 autotile sheet to Marigold as ONE
scene, so each cell lands in its own normal frame (measured by atlas_check.py:
shared ~10.6° tilt off +Z, E/W piece spread 5–6.4°, E|W seams max ~18°). This
runner treats each CELL as its own scene instead:

  slice    the diffuse atlas into cells (atlas.json grid; cell = side/cols px)
  pad      each cell crop with REPLICATED edges — context without ever showing the
           model its ATLAS neighbour (atlas order is NOT world adjacency; a run arm
           genuinely continues past the edge in-world, which is exactly what
           edge-replication depicts)
  infer    the normals pipeline per cell, ONE model load, a fresh generator with the
           SAME pinned seed per cell (identical noise → maximal cross-cell agreement)
  crop     back to the cell and composite the output atlas; outside the silhouette
           the flat facing-viewer fill (#8080FF), same as normals.py

Post passes land in later phases of the stream (marigold-linked-normals P2/P3) and
hang off flags so each is A/B-able against atlas_check numbers:
  --align       per-cell flat-frame rotation to +Z + bevel gain match   (P2)
  --symmetrize  equivalent-piece averaging + in-world seam blending     (P3)

Encoding matches normals.py: view-space, X right / Y up / Z toward viewer, [-1,1]
over RGB. Output is `normal.l.0.png` beside the diffuse (or --out for A/B runs).
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image

from normals import DEFAULT_MODEL, FLAT_NORMAL_RGB, load_pipeline

DIFFUSE = "diffuse.l.0.png"
NORMAL = "normal.l.0.png"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("kind_dir", type=Path, help=f"kind dir holding {DIFFUSE} (+ atlas.json)")
    ap.add_argument("--out", type=Path, default=None, help=f"write here instead of <kind>/{NORMAL}")
    ap.add_argument("--engine", choices=("whole", "cell"), default="whole",
                    help="whole = ONE inference over the full atlas (keeps relief — per-cell crops "
                         "starve Marigold on near-flat art: measured 33%%→2%% of pixels >15°); "
                         "cell = the per-cell experiment (default whole)")
    ap.add_argument("--context-frac", type=float, default=0.5,
                    help="cell engine: replicated-edge padding per side, as a fraction of the cell (default 0.5)")
    ap.add_argument("--steps", type=int, default=4)
    ap.add_argument("--ensemble", type=int, default=5)
    ap.add_argument("--resolution", type=int, default=768)
    ap.add_argument("--seed", type=int, default=2024)
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--device", default="cuda")
    ap.add_argument("--no-half", action="store_true")
    ap.add_argument("--align", action="store_true", help="P2: per-cell flat-frame rotation + bevel gain")
    ap.add_argument("--symmetrize", action="store_true", help="P3: piece averaging + seam blending")
    ap.add_argument("--post-only", type=Path, default=None, metavar="NORMAL_PNG",
                    help="skip inference: read this normal atlas and run only the post passes (A/B iteration)")
    args = ap.parse_args()

    dpath = args.kind_dir / DIFFUSE
    if not dpath.exists():
        print(f"atlas_normals: no {dpath}", file=sys.stderr)
        return 2
    grid = {"cols": 4, "rows": 4}
    aj = args.kind_dir / "atlas.json"
    if aj.exists():
        grid.update({k: v for k, v in json.loads(aj.read_text()).items() if k in ("cols", "rows")})
    cols, rows = int(grid["cols"]), int(grid["rows"])

    src = Image.open(dpath).convert("RGBA")
    W, H = src.size
    cw, ch = W // cols, H // rows
    ctx = int(round(args.context_frac * max(cw, ch)))

    rgba = np.asarray(src, dtype=np.uint8)

    if args.post_only is not None:
        out_atlas = np.asarray(Image.open(args.post_only).convert("RGB"), dtype=np.uint8).copy()
        if args.align:
            from atlas_align import align_atlas
            out_atlas = align_atlas(out_atlas, rgba, cols, rows)
        if args.symmetrize:
            from atlas_align import symmetrize_atlas
            out_atlas = symmetrize_atlas(out_atlas, rgba, cols, rows)
        op = args.out if args.out is not None else args.kind_dir / NORMAL
        Image.fromarray(out_atlas, "RGB").save(op)
        print(f"atlas_normals: post-only → {op}", flush=True)
        return 0

    print(f"atlas_normals: {dpath}  ({W}x{H}, {cols}x{rows} cells of {cw}px, context {ctx}px)", flush=True)
    print(f"atlas_normals: loading {args.model} on {args.device} "
          f"({'fp32' if args.no_half else 'fp16'}) ...", flush=True)
    t0 = time.time()
    pipe, torch = load_pipeline(args.model, half=not args.no_half, device=args.device)
    print(f"atlas_normals: model ready in {time.time() - t0:.1f}s", flush=True)

    proc_res = None if args.resolution == 0 else args.resolution
    out_atlas = np.zeros((H, W, 3), dtype=np.uint8)
    out_atlas[:, :] = FLAT_NORMAL_RGB

    if args.engine == "whole":
        # ONE inference over the full atlas — the model keeps its macro-shape signal (the
        # relief), and the post passes below fix the frame drift that inference costs.
        # FLOATED like normals.py: an image touching the frame edge reads as an infinite
        # plane and comes out FLAT (its own docstring; re-measured here — relief 23% → 0%
        # without the margin). Pad transparent, infer, crop back.
        margin = round(0.5 * max(W, H))
        padded = Image.new("RGBA", (W + 2 * margin, H + 2 * margin), (0, 0, 0, 0))
        padded.paste(src, (margin, margin))
        gen = torch.Generator(device=args.device).manual_seed(args.seed)
        pred = pipe(
            padded.convert("RGB"),
            num_inference_steps=args.steps,
            ensemble_size=args.ensemble,
            processing_resolution=proc_res,
            generator=gen,
        )
        nrm = pipe.image_processor.visualize_normals(pred.prediction)[0].convert("RGB")
        if nrm.size != padded.size:
            nrm = nrm.resize(padded.size, Image.BILINEAR)
        nrm = nrm.crop((margin, margin, margin + W, margin + H))
        n = np.asarray(nrm, dtype=np.uint8)
        a = rgba[:, :, 3:4] > 16
        out_atlas = np.where(a, n, np.array(FLAT_NORMAL_RGB, dtype=np.uint8)).astype(np.uint8)
        print(f"  whole-atlas inference done (floated {margin}px)", flush=True)
    else:
      for cell in range(cols * rows):
        cx, cy = (cell % cols) * cw, (cell // cols) * ch
        crop = rgba[cy : cy + ch, cx : cx + cw]
        # REPLICATED-edge context: the model sees the cell's own content continuing —
        # true in-world for connected arms, neutral for flats — never the atlas neighbour.
        padded = np.pad(crop, ((ctx, ctx), (ctx, ctx), (0, 0)), mode="edge")
        rgb = Image.fromarray(padded[:, :, :3], "RGB")

        gen = torch.Generator(device=args.device).manual_seed(args.seed)
        pred = pipe(
            rgb,
            num_inference_steps=args.steps,
            ensemble_size=args.ensemble,
            processing_resolution=proc_res,
            generator=gen,
        )
        nrm = pipe.image_processor.visualize_normals(pred.prediction)[0].convert("RGB")
        if nrm.size != rgb.size:  # some processors return the processing resolution
            nrm = nrm.resize(rgb.size, Image.BILINEAR)
        n = np.asarray(nrm, dtype=np.uint8)[ctx : ctx + ch, ctx : ctx + cw]
        # Flat fill outside the silhouette (alpha from the diffuse cell), matching normals.py.
        a = crop[:, :, 3:4] > 16
        out_atlas[cy : cy + ch, cx : cx + cw] = np.where(a, n, np.array(FLAT_NORMAL_RGB, dtype=np.uint8))
        print(f"  [{cell + 1}/{cols * rows}] cell {cell}", flush=True)

    if args.align:
        from atlas_align import align_atlas  # P2 (lands with its own item)
        out_atlas = align_atlas(out_atlas, rgba, cols, rows)
    if args.symmetrize:
        from atlas_align import symmetrize_atlas  # P3
        out_atlas = symmetrize_atlas(out_atlas, rgba, cols, rows)

    op = args.out if args.out is not None else args.kind_dir / NORMAL
    op.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(out_atlas, "RGB").save(op)
    print(f"atlas_normals: wrote {op}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
