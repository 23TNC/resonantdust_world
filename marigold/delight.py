#!/usr/bin/env python3
"""Marigold-IID de-lighting for master sprites.

Replaces the hand-rolled two-pass divide (bin/art `delight-divide`) with
Marigold Intrinsic Image Decomposition — the "lighting" checkpoint, which splits
each diffuse sprite as

    I = A * S + R

into Albedo A (base colour, de-lit), diffuse Shading S, and non-diffuse Residual
R (the additive highlights — painted rim lights / speculars / self-lit glow —
that a division CANNOT remove). We ship A as `<base>.albedo.png`, a drop-in for
the old albedo, carrying the source sprite's alpha. Unlike the divide, this needs
no normal map: it is a learned decomposition, independent of Laigter.

The emitted maps are named for the user-facing target, prefixed `albedo_` for the
IID by-products (see EMIT_TARGETS): S → `<base>.albedo_shading.png`, R →
`<base>.albedo_residual.png`. `albedo_residual` is the raw non-diffuse term that
`art emissive` gates + cleans into the shipped `<base>.emissive.png`.

Batch: the model loads once, then every diffuse under the given paths is
decomposed. Invoked by `bin/art delight` via the `bin/marigold` wrapper, but also
runnable directly for a spike:

    bin/marigold delight textures/master/linked.0/wall_smooth.0 \
        --emit albedo,albedo_shading,albedo_residual --out-dir /tmp/spike

Each `N.diffuse.png` maps to `N.<target>.png` co-located (or flat under --out-dir).
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

# A variant leaf's diffuse member in the new leaf layout — the shape's single source
# of truth is docs/components/dev/textures/design/texture-layout/. This module
# re-implements it natively (the marigold venv can't import bin/lib/texpath). The
# diffuse is a directional map `diffuse.<dir>.<part>.png` in a <variant>/ leaf; each
# emitted map is a `<target>.<dir>.<part>.png` sibling at the same dir/part.
# normals.py / depth.py / normal_depth.py all route through find_diffuse + out_path.


def _diffuse_dir_part(diffuse) -> tuple[str, str]:
    """(<dir>, <part>) parsed from a `diffuse.<dir>.<part>.png` leaf filename."""
    parts = diffuse.name.split(".")   # [diffuse, dir, part, png]
    return (parts[1], parts[2]) if len(parts) == 4 else ("s", "0")


def _is_diffuse(name: str) -> bool:
    """True for a `diffuse.<dir>.<part>.png` directional leaf file."""
    return name.startswith("diffuse.") and name.endswith(".png") and name.count(".") == 3
DEFAULT_MODEL = "prs-eth/marigold-iid-lighting-v1-1"
# User-facing emit targets → the "lighting" checkpoint's visualize_intrinsics keys.
# The IID by-products are prefixed `albedo_` so they (a) read as products of the
# albedo decomposition and (b) don't collide with split_layers' `packed_residual`.
# `albedo_residual` is the non-diffuse (emissive/specular) term R — the source that
# `art emissive` gates + cleans into the shipped `<base>.emissive.png`.
EMIT_TARGETS = {
    "albedo": "albedo",
    "albedo_shading": "shading",
    "albedo_residual": "residual",
}


def find_diffuse(paths: list[Path]) -> list[Path]:
    """Every variant-leaf `diffuse.<dir>.<part>.png` under the given files/dirs (sorted, de-duped)."""
    out: list[Path] = []
    for p in paths:
        if p.is_dir():
            out.extend(sorted(q for q in p.rglob("diffuse.*.png") if _is_diffuse(q.name)))
        elif _is_diffuse(p.name):
            out.append(p)
        else:
            print(f"marigold: skipping non-diffuse path {p}", file=sys.stderr)
    # de-dupe, keep order
    seen: set[Path] = set()
    uniq: list[Path] = []
    for p in out:
        rp = p.resolve()
        if rp not in seen:
            seen.add(rp)
            uniq.append(p)
    return uniq


def out_path(diffuse: Path, target: str, out_dir: Path | None) -> Path:
    """The `<target>.<dir>.<part>.png` sibling in the diffuse's variant leaf (same dir/part),
    or a collision-free flat name under out_dir if given."""
    d, part = _diffuse_dir_part(diffuse)
    if out_dir is None:
        return diffuse.parent / f"{target}.{d}.{part}.png"
    # Flat mode: <kind>.<variant>.<target>.<dir>.<part>.png from the new leaf
    # …/<kind>/<variant>/diffuse.<dir>.<part>.png so spikes over many variants don't collide.
    variant = diffuse.parent.name
    kind = diffuse.parent.parent.name
    return out_dir / f"{kind}.{variant}.{target}.{d}.{part}.png"


def load_pipeline(model: str, half: bool, device: str):
    try:
        import torch
        from diffusers import MarigoldIntrinsicsPipeline
    except ImportError as e:  # diffusers too old, or torch missing
        print(
            f"marigold: {e}\n"
            "  MarigoldIntrinsicsPipeline needs diffusers>=0.33 and torch.\n"
            "  Rebuild the venv:  bin/marigold build",
            file=sys.stderr,
        )
        raise SystemExit(2)

    kwargs = {}
    dtype = None
    if half:
        dtype = torch.float16
        kwargs = {"variant": "fp16", "torch_dtype": dtype}
    try:
        pipe = MarigoldIntrinsicsPipeline.from_pretrained(model, **kwargs)
    except Exception as e:  # fp16 variant absent, etc. — retry plain.
        if half:
            print(f"marigold: fp16 variant load failed ({e}); retrying full precision", file=sys.stderr)
            pipe = MarigoldIntrinsicsPipeline.from_pretrained(model)
            dtype = None
        else:
            raise
    pipe = pipe.to(device)
    pipe.set_progress_bar_config(disable=True)
    return pipe, torch, dtype


def restore_edge(albedo: "Image.Image", diffuse_rgb: "Image.Image", alpha: "Image.Image") -> "Image.Image":
    """Kill the de-lighting halo. Marigold lifts the sprite's near-black AA fringe
    (lum ~0-13 in the diffuse) into a pale ghost ring (~lum 24-50) — a halo the
    source never had. Sub-opaque pixels carry almost no real albedo signal (they're
    AA blends of outline over transparent background), so restore the diffuse's own
    clean, dark edge colour wherever alpha < 255; the fully-opaque interior keeps
    the de-lit albedo untouched. Result: the albedo edge byte-matches the diffuse
    edge, so split_layers never packs a halo."""
    import numpy as np
    from PIL import Image
    df = diffuse_rgb if diffuse_rgb.size == alpha.size else diffuse_rgb.resize(alpha.size, Image.NEAREST)
    out = np.asarray(albedo).copy()                    # HxWx4
    edge = np.asarray(alpha) < 255
    out[edge, :3] = np.asarray(df.convert("RGB"))[edge]
    return Image.fromarray(out, "RGBA")


def main() -> int:
    ap = argparse.ArgumentParser(description="Marigold-IID de-lighting for master sprites.")
    ap.add_argument("paths", nargs="+", type=Path,
                    help="diffuse PNGs or directories to scan for *.diffuse.png")
    ap.add_argument("--emit", default="albedo",
                    help="comma list of targets to write: albedo,albedo_shading,albedo_residual (default albedo)")
    ap.add_argument("--out-dir", type=Path, default=None,
                    help="write maps here (flat) instead of co-located next to the diffuse")
    ap.add_argument("--steps", type=int, default=4, help="denoising steps (default 4)")
    ap.add_argument("--ensemble", type=int, default=5,
                    help="ensemble size for precision; >=3 enables ensembling (default 5)")
    ap.add_argument("--resolution", type=int, default=768,
                    help="processing resolution on the long side; 0 = native (default 768)")
    ap.add_argument("--seed", type=int, default=2024, help="RNG seed, per-image, for reproducible masters")
    ap.add_argument("--model", default=DEFAULT_MODEL, help=f"HF checkpoint (default {DEFAULT_MODEL})")
    ap.add_argument("--device", default="cuda", help="torch device (default cuda)")
    ap.add_argument("--no-half", action="store_true", help="full fp32 instead of the fp16 variant")
    ap.add_argument("--skip-existing", action="store_true", help="skip a sprite if its albedo already exists")
    args = ap.parse_args()

    from PIL import Image

    targets = [t.strip() for t in args.emit.split(",") if t.strip()]
    unknown = [t for t in targets if t not in EMIT_TARGETS]
    if unknown:
        print(f"marigold: unknown emit target(s) {unknown}; valid: {tuple(EMIT_TARGETS)}", file=sys.stderr)
        return 2

    diffuse = find_diffuse(args.paths)
    if not diffuse:
        print("marigold: no *.diffuse.png found under the given paths", file=sys.stderr)
        return 1

    if args.out_dir is not None:
        args.out_dir.mkdir(parents=True, exist_ok=True)

    if args.skip_existing:
        pending = [d for d in diffuse if not out_path(d, "albedo", args.out_dir).exists()]
        skipped = len(diffuse) - len(pending)
        if skipped:
            print(f"marigold: skipping {skipped} sprite(s) with existing albedo")
        diffuse = pending
        if not diffuse:
            print("marigold: nothing to do")
            return 0

    print(f"marigold: loading {args.model} on {args.device} "
          f"({'fp32' if args.no_half else 'fp16'}) ...", flush=True)
    t0 = time.time()
    pipe, torch, _ = load_pipeline(args.model, half=not args.no_half, device=args.device)
    print(f"marigold: model ready in {time.time() - t0:.1f}s; "
          f"decomposing {len(diffuse)} sprite(s) [emit: {','.join(targets)}]", flush=True)

    proc_res = None if args.resolution == 0 else args.resolution
    ok = 0
    for i, d in enumerate(diffuse, 1):
        # Normalise to RGBA first so alpha is recovered correctly for palette-mode
        # ('P' + transparency) inputs too, then split. RGB is taken from the RGBA
        # (keeps the edge-bled colour; does not composite the sprite over black).
        src = Image.open(d).convert("RGBA")
        alpha = src.getchannel("A")
        rgb = src.convert("RGB")

        gen = torch.Generator(device=args.device).manual_seed(args.seed)
        pred = pipe(
            rgb,
            num_inference_steps=args.steps,
            ensemble_size=args.ensemble,
            processing_resolution=proc_res,
            generator=gen,
        )
        vis = pipe.image_processor.visualize_intrinsics(pred.prediction, pipe.target_properties)[0]

        for t in targets:
            img = vis[EMIT_TARGETS[t]].convert("RGBA")   # vis keyed by Marigold name; file named by target
            if alpha is not None:
                a = alpha if alpha.size == img.size else alpha.resize(img.size, Image.NEAREST)
                img.putalpha(a)
                if t == "albedo":
                    img = restore_edge(img, rgb, a)   # strip the de-lit fringe halo
            op = out_path(d, t, args.out_dir)
            op.parent.mkdir(parents=True, exist_ok=True)
            img.save(op)
        ok += 1
        print(f"  [{i}/{len(diffuse)}] {d.name} -> {', '.join(t for t in targets)}", flush=True)

    print(f"marigold: wrote {ok} sprite(s) x {len(targets)} target(s)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
