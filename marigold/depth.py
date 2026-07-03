#!/usr/bin/env python3
"""Marigold depth (height field) for master sprites.

Writes `N.depth.png` co-located — a 16-bit grayscale height field, the source for
baked AO and (later) heightfield-raymarched dynamic shadows / parallax. See
docs/de-lighting.md.

Two conventions to know (both settled at render time — "build the renderer to
match", like the normal convention):

- **Affine-invariant, PER IMAGE.** Marigold normalises depth per sprite: 0 = the
  nearest plane, 1 = the farthest, planes chosen by the model for THAT image. So a
  sprite's depths are internally consistent (fine for per-sprite AO / self-shadow)
  but NOT calibrated across sprites — compositing into one world height buffer for
  cross-object shadows needs a per-sprite scale/offset the renderer supplies.
- **No alpha.** 16-bit grayscale carries no silhouette; coverage comes from the
  shared sprite mask (the albedo/normal alpha). Transparent regions hold whatever
  the model predicted for the edge-bled background — the renderer masks them out
  via the albedo alpha, so they never sample.

Batch: the model loads once, then every diffuse under the given paths is processed.
`--visualize` also writes an 8-bit colour-mapped `N.depth-viz.png` for eyeballing.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

from delight import find_diffuse, out_path

DEFAULT_MODEL = "prs-eth/marigold-depth-v1-1"


def load_pipeline(model: str, half: bool, device: str):
    try:
        import torch
        from diffusers import MarigoldDepthPipeline
    except ImportError as e:
        print(
            f"marigold: {e}\n"
            "  MarigoldDepthPipeline needs diffusers>=0.33 and torch.\n"
            "  Rebuild the venv:  bin/marigold build",
            file=sys.stderr,
        )
        raise SystemExit(2)

    kwargs = {"variant": "fp16", "torch_dtype": torch.float16} if half else {}
    try:
        pipe = MarigoldDepthPipeline.from_pretrained(model, **kwargs)
    except Exception as e:
        if half:
            print(f"marigold: fp16 variant load failed ({e}); retrying full precision", file=sys.stderr)
            pipe = MarigoldDepthPipeline.from_pretrained(model)
        else:
            raise
    pipe = pipe.to(device)
    pipe.set_progress_bar_config(disable=True)
    return pipe, torch


def main() -> int:
    ap = argparse.ArgumentParser(description="Marigold depth (height field) for master sprites.")
    ap.add_argument("paths", nargs="+", type=Path,
                    help="diffuse PNGs or directories to scan for *.diffuse.png")
    ap.add_argument("--out-dir", type=Path, default=None,
                    help="write depth here (flat) instead of co-located next to the diffuse")
    ap.add_argument("--steps", type=int, default=4, help="denoising steps (default 4)")
    ap.add_argument("--ensemble", type=int, default=5,
                    help="ensemble size; >=3 enables ensembling (default 5)")
    ap.add_argument("--resolution", type=int, default=768,
                    help="processing resolution on the long side; 0 = native (default 768)")
    ap.add_argument("--seed", type=int, default=2024, help="RNG seed, per-image, for reproducible masters")
    ap.add_argument("--model", default=DEFAULT_MODEL, help=f"HF checkpoint (default {DEFAULT_MODEL})")
    ap.add_argument("--device", default="cuda", help="torch device (default cuda)")
    ap.add_argument("--no-half", action="store_true", help="full fp32 instead of the fp16 variant")
    ap.add_argument("--skip-existing", action="store_true", help="skip a sprite if its depth already exists")
    ap.add_argument("--pad-frac", type=float, default=0.5,
                    help="pad the sprite off the frame edges by this fraction of its content size "
                         "per side before depth, then crop back. Objects touching the frame edge "
                         "make monocular depth flatten them; floating them fixes it. 0 disables. "
                         "(default 0.5)")
    ap.add_argument("--visualize", action="store_true",
                    help="also write an 8-bit colour-mapped N.depth-viz.png for inspection")
    args = ap.parse_args()

    from PIL import Image

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

    print(f"marigold: loading {args.model} on {args.device} "
          f"({'fp32' if args.no_half else 'fp16'}) ...", flush=True)
    t0 = time.time()
    pipe, torch = load_pipeline(args.model, half=not args.no_half, device=args.device)
    print(f"marigold: model ready in {time.time() - t0:.1f}s; "
          f"depth-mapping {len(diffuse)} sprite(s)", flush=True)

    proc_res = None if args.resolution == 0 else args.resolution
    ok = 0
    for i, d in enumerate(diffuse, 1):
        src = Image.open(d).convert("RGBA")
        w, h = src.size
        # Objects that run to the frame edge confuse monocular depth (it can't tell
        # they don't continue past the frame) and come out flat. Pad the sprite so
        # it floats with margin, run depth on the padded frame, then crop back to the
        # original canvas. Margin scales with the content size so small/large sprites
        # get proportional breathing room.
        margin = 0
        if args.pad_frac > 0:
            bbox = src.getchannel("A").getbbox()
            if bbox:
                cw, ch = bbox[2] - bbox[0], bbox[3] - bbox[1]
                margin = round(args.pad_frac * max(cw, ch))
        if margin > 0:
            padded = Image.new("RGBA", (w + 2 * margin, h + 2 * margin), (0, 0, 0, 0))
            padded.paste(src, (margin, margin))
            rgb = padded.convert("RGB")
        else:
            rgb = src.convert("RGB")  # depth is single-channel; coverage from albedo alpha

        gen = torch.Generator(device=args.device).manual_seed(args.seed)
        pred = pipe(
            rgb,
            num_inference_steps=args.steps,
            ensemble_size=args.ensemble,
            processing_resolution=proc_res,
            generator=gen,
        )
        # 16-bit grayscale height field — the data channel. Crop the padding back off.
        img16 = pipe.image_processor.export_depth_to_16bit_png(pred.prediction)[0]
        if margin > 0:
            img16 = img16.crop((margin, margin, margin + w, margin + h))
        op = out_path(d, "depth", args.out_dir)
        op.parent.mkdir(parents=True, exist_ok=True)
        img16.save(op)

        if args.visualize:
            viz = pipe.image_processor.visualize_depth(pred.prediction)[0]
            if margin > 0:
                viz = viz.crop((margin, margin, margin + w, margin + h))
            vp = op.with_name(op.name.replace(".depth.png", ".depth-viz.png"))
            viz.save(vp)
        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.name} -> {op.name}"
              f"{' (+viz)' if args.visualize else ''}", flush=True)

    print(f"marigold: wrote {ok} depth map(s)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
