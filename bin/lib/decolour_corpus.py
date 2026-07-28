#!/usr/bin/env python3
"""Decolourise the sprite corpus through the LAYER pipeline, keeping all structure.

Not a desaturation. Each sprite is split into (residual, layers) by `art split_layers`, then
reconstructed with the renderer's own formula but every tint set to ONE flat grey:

    out = residual + sum(layers.ch * TINT)

The residual holds the black line work — outlines AND the filled markings drawn as black in the
source (tiger stripes, jaguar spots) — and the per-pixel layer coefficients carry brightness. So
hue is the only thing removed, which is exactly the component measured as lossy in the layer
round-trip anyway (brightness survives to ~1/255; see the sprite-eval-trust stream).

TINT = 200 grey, measured: it keeps 78% of the tonal range while leaving a 55-level gap to the
white plate. At 255 the palest subject (bear) lands exactly on 255 and vanishes into the
background; at 128 a dark coat crowds against its own black outline.

Alpha from the source is re-applied so the downstream prep can still crop to the subject bbox.

  python3 bin/lib/decolour_corpus.py                 # animal-lora -> animal-lora-gray
  python3 bin/lib/decolour_corpus.py --tint 160
"""
import argparse, glob, os, shutil, subprocess, sys, tempfile
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, ".staging", "animal-lora")
DST = os.path.join(REPO, ".staging", "animal-lora-gray")

def reconstruct(work, tint):
    """residual + sum(layers*tint), clipped. Returns float RGB in 0..255."""
    res = np.asarray(Image.open(os.path.join(work, "albedo.e.0.png")).convert("RGB"), float) / 255.0
    lay = np.asarray(Image.open(os.path.join(work, "layers.e.0.png")).convert("RGB"), float) / 255.0
    t = np.array([tint] * 3, float) / 255.0
    out = res.copy()
    for i in range(3):
        out += lay[..., i:i+1] * t[None, None, :]
    return np.clip(out, 0.0, 1.0) * 255.0

def main():
    ap = argparse.ArgumentParser(prog="decolour_corpus")
    ap.add_argument("--tint", type=int, default=200, help="flat grey applied to every layer (default 200)")
    ap.add_argument("--src", default=SRC)
    ap.add_argument("--dst", default=DST)
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()

    src = args.src if os.path.isabs(args.src) else os.path.join(REPO, args.src)
    dst = args.dst if os.path.isabs(args.dst) else os.path.join(REPO, args.dst)
    if os.path.isdir(dst): shutil.rmtree(dst)

    sprites = sorted(glob.glob(os.path.join(src, "*", "*.png")))
    if args.limit: sprites = sprites[:args.limit]
    print(f"decolour: {len(sprites)} sprites, tint {args.tint} grey -> {os.path.relpath(dst, REPO)}")

    ok = skipped = 0
    with tempfile.TemporaryDirectory() as tmp:
        # stage every sprite as its own albedo leaf so ONE split_layers walk handles them all
        index = {}
        for i, p in enumerate(sprites):
            d = os.path.join(tmp, f"s{i:04d}"); os.makedirs(d)
            shutil.copy2(p, os.path.join(d, "albedo.e.0.png"))
            index[d] = p
        # ONE INVOCATION PER SPRITE. split_layers processes every path in a single process, so a
        # single bad sprite aborts the run and everything after it silently produces nothing —
        # Elk/ElkFemale_south.png (IndexError, empty axis) cost 417 of 701 sprites on the first
        # attempt and 35 more on the second. Per-sprite is slower but a crash can only lose its own
        # image, and the failure is reported instead of vanishing.
        leaves = sorted(index)
        failed = []
        for n, d in enumerate(leaves, 1):
            r = subprocess.run([sys.executable, os.path.join(HERE, "split_layers.py"),
                                "--normalize", d], capture_output=True, text=True)
            if r.returncode != 0:
                tail = (r.stderr.strip().splitlines() or [""])[-1]
                failed.append((os.path.basename(index[d]), tail[:120]))
            if n % 100 == 0: print(f"  split {n}/{len(leaves)}", flush=True)
        if failed:
            print(f"  {len(failed)} sprite(s) CRASHED split_layers:")
            for name, err in failed[:10]: print(f"    {name}: {err}")

        for d, srcp in index.items():
            if not os.path.exists(os.path.join(d, "layers.e.0.png")):
                skipped += 1; continue                       # split found no material to pull
            rgb = reconstruct(d, args.tint)
            a = np.asarray(Image.open(srcp).convert("RGBA"))[..., 3]   # original alpha
            out = np.dstack([rgb.astype(np.uint8), a])
            rel = os.path.relpath(srcp, src)
            op = os.path.join(dst, rel); os.makedirs(os.path.dirname(op), exist_ok=True)
            Image.fromarray(out, "RGBA").save(op)
            ok += 1
    print(f"  wrote {ok}   skipped {skipped} (no layers produced)")

if __name__ == "__main__":
    main()
