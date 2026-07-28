#!/usr/bin/env python3
"""Scale every derived map in a leaf to its target square (`span * TILE_PX`).

The `sprite` master is an ARCHIVE, not a deliverable: it is held at whatever resolution it was
authored at and never resized. What ships is the derived set, and that must be the size the packer
expects — so this normalises in BOTH directions, whenever the map does not already match.

An upscale is reported, not refused. The master stays put, so nothing is lost and the decision is
reversible: re-author the source at a higher resolution later and re-running this produces a better
deliverable with no pipeline change. The warning exists to make undersized sources a visible
worklist rather than a silent softening. This art is coarse, flat-shaded and stylised, which
survives an upscale far better than photographic detail would.

Runs AFTER `leaf-span` (which stamps `square` into `meta.json`) and BEFORE `--pad` (whose guard ring
is measured against the final canvas).

Atlas leaves scale as a whole, which preserves the cell grid by construction — a 4x4 atlas going
320 -> 512 takes its cells from 80 px to 128 px.

  python3 bin/lib/normalize_square.py textures/biome-tile/default/smooth
"""
import argparse, glob, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
from PIL import Image

REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
SKIP = ("sprite", "template")


def is_source(name):
    stem = name[:-4] if name.lower().endswith(".png") else name
    parts = stem.split(".")
    return any(s in (parts[0], parts[-1]) for s in SKIP)


def target_of(leaf):
    """The leaf's target square, from the `square` that `leaf-span` stamped."""
    p = os.path.join(leaf, "meta.json")
    if not os.path.exists(p):
        return None
    try:
        return json.load(open(p)).get("square")
    except (json.JSONDecodeError, OSError):
        return None


def main():
    ap = argparse.ArgumentParser(prog="art normalize-square")
    ap.add_argument("paths", nargs="*")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()
    roots = [os.path.abspath(p) for p in args.paths] or [os.path.join(REPO, "textures")]

    scaled = up = 0
    warned = set()
    for root in roots:
        for dirpath, _d, files in os.walk(root):
            if not any(texpath.parse_map(f) for f in files):
                continue
            target = target_of(dirpath)
            if not target:
                continue
            for f in sorted(files):
                if not f.lower().endswith(".png") or is_source(f):
                    continue
                p = os.path.join(dirpath, f)
                try:
                    im = Image.open(p)
                except OSError:
                    continue
                if im.size == (target, target):
                    continue
                w, h = im.size
                if max(w, h) < target and dirpath not in warned:
                    rel = os.path.relpath(dirpath, os.path.join(REPO, "textures"))
                    print(f"art: normalize-square: {rel} is {w}x{h}, UPSCALING to {target}x{target} "
                          f"— the master is undersized for a {target // 128}-tile span; consider "
                          f"re-authoring it.", file=sys.stderr)
                    warned.add(dirpath)
                    up += 1
                im.resize((target, target), Image.LANCZOS).save(p)
                scaled += 1
    print(f"art: normalize-square -> {scaled} map(s) scaled to their target square "
          f"({up} leaf/leaves upscaled from an undersized master)")


if __name__ == "__main__":
    main()
