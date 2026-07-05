#!/usr/bin/env python3
"""Marigold surface-normals for master sprites.

The optional (`art normal --marigold`) alternative to the default Laigter engine
(see docs/de-lighting.md). Marigold predicts VIEW-SPACE surface normals of the
depicted geometry — cleaner macro shape than Laigter's luminance-heightfield guess,
but not a valid height field (depth won't integrate) and starved on flat art. Writes
`N.normal.png` co-located with a FLAT OPAQUE background (#8080FF outside the
silhouette); coverage comes from the albedo alpha at render time.

Convention: Marigold normals are view-space — X right, Y up, Z toward the viewer,
[-1,1] mapped to RGB — the SAME OpenGL/+Y-up encoding Laigter emits (verified), so
the renderer reads either directly, no green-flip / tangent-space conversion (see
docs/de-lighting.md). Batch: the model loads once, then every diffuse is processed.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

# Shared discovery / path helpers live in the sibling de-lighter.
from delight import find_diffuse, out_path

DEFAULT_MODEL = "prs-eth/marigold-normals-v1-1"
# Flat facing-viewer normal (0,0,1) in Marigold's view-space encoding — the neutral
# fill for pixels outside the sprite silhouette. See the fill note in main().
FLAT_NORMAL_RGB = (128, 128, 255)


def load_pipeline(model: str, half: bool, device: str):
    try:
        import torch
        from diffusers import MarigoldNormalsPipeline
    except ImportError as e:
        print(
            f"marigold: {e}\n"
            "  MarigoldNormalsPipeline needs diffusers>=0.33 and torch.\n"
            "  Rebuild the venv:  bin/marigold build",
            file=sys.stderr,
        )
        raise SystemExit(2)

    kwargs = {"variant": "fp16", "torch_dtype": torch.float16} if half else {}
    try:
        pipe = MarigoldNormalsPipeline.from_pretrained(model, **kwargs)
    except Exception as e:
        if half:
            print(f"marigold: fp16 variant load failed ({e}); retrying full precision", file=sys.stderr)
            pipe = MarigoldNormalsPipeline.from_pretrained(model)
        else:
            raise
    pipe = pipe.to(device)
    pipe.set_progress_bar_config(disable=True)
    return pipe, torch


def main() -> int:
    ap = argparse.ArgumentParser(description="Marigold surface-normals for master sprites.")
    ap.add_argument("paths", nargs="+", type=Path,
                    help="diffuse PNGs or directories to scan for *.diffuse.png")
    ap.add_argument("--out-dir", type=Path, default=None,
                    help="write normals here (flat) instead of co-located next to the diffuse")
    ap.add_argument("--steps", type=int, default=4, help="denoising steps (default 4)")
    ap.add_argument("--ensemble", type=int, default=5,
                    help="ensemble size; >=3 enables ensembling, sharpens fine structure (default 5)")
    ap.add_argument("--resolution", type=int, default=768,
                    help="processing resolution on the long side; 0 = native (default 768)")
    ap.add_argument("--seed", type=int, default=2024, help="RNG seed, per-image, for reproducible masters")
    ap.add_argument("--model", default=DEFAULT_MODEL, help=f"HF checkpoint (default {DEFAULT_MODEL})")
    ap.add_argument("--device", default="cuda", help="torch device (default cuda)")
    ap.add_argument("--no-half", action="store_true", help="full fp32 instead of the fp16 variant")
    ap.add_argument("--skip-existing", action="store_true", help="skip a sprite if its normal already exists")
    ap.add_argument("--pad-frac", type=float, default=0.5,
                    help="pad the sprite off the frame edges by this fraction of its content size "
                         "per side before inference, then crop back. Objects touching the frame edge "
                         "make monocular models flatten them; floating them fixes it. 0 disables. "
                         "(default 0.5)")
    args = ap.parse_args()

    from PIL import Image

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("marigold: no *.diffuse.png found under the given paths", file=sys.stderr)
        return 1

    if args.out_dir is not None:
        args.out_dir.mkdir(parents=True, exist_ok=True)

    if args.skip_existing:
        pending = [d for d in diffuse if not out_path(d, "normal", args.out_dir).exists()]
        skipped = len(diffuse) - len(pending)
        if skipped:
            print(f"marigold: skipping {skipped} sprite(s) with existing normal")
        diffuse = pending
        if not diffuse:
            print("marigold: nothing to do")
            return 0

    print(f"marigold: loading {args.model} on {args.device} "
          f"({'fp32' if args.no_half else 'fp16'}) ...", flush=True)
    t0 = time.time()
    pipe, torch = load_pipeline(args.model, half=not args.no_half, device=args.device)
    print(f"marigold: model ready in {time.time() - t0:.1f}s; "
          f"normal-mapping {len(diffuse)} sprite(s)", flush=True)

    proc_res = None if args.resolution == 0 else args.resolution
    ok = 0
    for i, d in enumerate(diffuse, 1):
        # Normalise to RGBA so alpha is recovered for palette-mode inputs too;
        # RGB is taken from it (keeps edge-bled colour, no compositing over black).
        src = Image.open(d).convert("RGBA")
        alpha = src.getchannel("A")
        w, h = src.size
        # Objects that run to the frame edge confuse monocular models (they can't
        # tell the object doesn't continue past it) and come out flattened. Pad the
        # sprite so it floats with margin, infer on the padded frame, then crop back
        # to the original canvas. Margin scales with the content size.
        margin = 0
        if args.pad_frac > 0:
            bbox = alpha.getbbox()
            if bbox:
                cw, ch = bbox[2] - bbox[0], bbox[3] - bbox[1]
                margin = round(args.pad_frac * max(cw, ch))
        if margin > 0:
            padded = Image.new("RGBA", (w + 2 * margin, h + 2 * margin), (0, 0, 0, 0))
            padded.paste(src, (margin, margin))
            rgb = padded.convert("RGB")
        else:
            rgb = src.convert("RGB")

        gen = torch.Generator(device=args.device).manual_seed(args.seed)
        pred = pipe(
            rgb,
            num_inference_steps=args.steps,
            ensemble_size=args.ensemble,
            processing_resolution=proc_res,
            generator=gen,
        )
        nrm = pipe.image_processor.visualize_normals(pred.prediction)[0].convert("RGB")
        if margin > 0:
            nrm = nrm.crop((margin, margin, margin + w, margin + h))
        a = alpha if alpha.size == nrm.size else alpha.resize(nrm.size, Image.NEAREST)
        # Outside the silhouette, fill a FLAT facing-viewer normal (0,0,1) = #8080FF
        # (Marigold's own encoding for a camera-facing surface — also "flat ground"
        # in a top-down view) instead of leaving the model's background prediction.
        # We ship the normal with this flat background OPAQUE (not masked to
        # transparent): coverage comes from the albedo alpha at render time, and a
        # flat opaque background keeps edge texels valid under bilinear filtering /
        # mipmaps. Matches the Laigter path's _flatten_normal_bg. See docs/de-lighting.md.
        out = Image.new("RGB", nrm.size, FLAT_NORMAL_RGB)
        out.paste(nrm, (0, 0), a)

        op = out_path(d, "normal", args.out_dir)
        op.parent.mkdir(parents=True, exist_ok=True)
        out.save(op)
        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.name} -> {op.name}", flush=True)

    print(f"marigold: wrote {ok} normal map(s)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
