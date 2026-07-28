#!/usr/bin/env python3
"""Shrink each derived map's content by PAD px per side, replicating the edge into the ring.

The canvas size never changes — a power-of-two stays a power of two for the quadtree packer.
Only the CONTENT shrinks, and the freed ring is filled with the content's own edge pixels
(corners included, by replicating both axes).

Why the ring exists: at non-integer zoom the sampler reads just past a sprite's footprint, and
without a guard that read lands on whatever the packer placed next door. Filling it with the
sprite's own edge colour turns the spill into more of itself instead of a neighbour's fringe.

Why this is Python and not `convert -distort`: an identity distort still RESAMPLES through the
active filter, so the ring came out subtly different from the row it was supposed to copy
(measured: columns matched, rows did not). `np.pad(mode="edge")` is an exact copy by
construction, and gets the corners right for any pad width in one call.

Authoring sources (sprite/template) are skipped — they are inputs to a later re-remaster, so
shrinking them would compound on every run.

ATLAS leaves are skipped entirely. A canvas-edge ring is the wrong guard for a cell grid twice
over: the boundaries a sampler actually crosses are the INTERIOR ones, which a canvas ring never
touches, and shrinking the content pulls every cell off its pitch (measured on an 8x8 sheet: 1024
canvas preserved, but 1022 px of content across 8 cells = 127.75 px/cell instead of 128). The
per-cell guard for an atlas is the GRID_INSET_FRAC *sampling* inset — recorded as padU/padV in
atlas.json, folded into the manifest, and trimmed off each cell's UV rect by the client — which
costs no pixels and no resampling. See the stream's F3.

  python3 bin/lib/pad_maps.py <root> --pad 1
"""
import argparse, os, sys
import numpy as np
from PIL import Image

SKIP = ("sprite", "template")


def is_pow2(n):
    return n > 0 and (n & (n - 1)) == 0


def is_source(name):
    """True for an authoring source: `sprite.png`, `sprite.<...>.png`, `<...>.sprite.png`."""
    stem = name[:-4] if name.lower().endswith(".png") else name
    parts = stem.split(".")
    return any(s in (parts[0], parts[-1]) for s in SKIP)


def is_atlas_leaf(dirpath):
    """True if this leaf is a held-whole cell grid — it carries an `atlas.json` sidecar."""
    return os.path.exists(os.path.join(dirpath, "atlas.json"))


def pad_one(path, pad):
    im = Image.open(path)
    mode = im.mode
    a = np.asarray(im)
    h, w = a.shape[:2]
    iw, ih = w - 2 * pad, h - 2 * pad
    if iw < 1 or ih < 1:
        return None, f"--pad {pad} leaves no content in {os.path.basename(path)} ({w}x{h})"
    inner = im.resize((iw, ih), Image.LANCZOS)
    b = np.asarray(inner)
    pads = ((pad, pad), (pad, pad)) + ((0, 0),) * (b.ndim - 2)
    out = np.pad(b, pads, mode="edge")
    Image.fromarray(out, mode).save(path)
    return (w, h), None


def main():
    ap = argparse.ArgumentParser(prog="art pad_maps")
    ap.add_argument("root")
    ap.add_argument("--pad", type=int, default=1)
    args = ap.parse_args()
    if args.pad <= 0:
        return

    done, warned, skipped_atlas = 0, False, 0
    for dirpath, _d, files in os.walk(args.root):
        if is_atlas_leaf(dirpath):
            skipped_atlas += 1
            continue
        for f in sorted(files):
            if not f.lower().endswith(".png") or is_source(f):
                continue
            p = os.path.join(dirpath, f)
            size, err = pad_one(p, args.pad)
            if err:
                print(f"art: {err} — skipped", file=sys.stderr)
                continue
            w, h = size
            if not warned and not (is_pow2(w) and is_pow2(h)):
                print(f"art: --pad: {w}x{h} is not a power of two — padding preserves the size, "
                      f"so this map still will not pack; regenerate it at a pow2 size.",
                      file=sys.stderr)
                warned = True
            done += 1
    note = ""
    if skipped_atlas:
        note = (f"; skipped {skipped_atlas} atlas leaf/leaves — a cell grid is guarded by the "
                f"per-cell padU/padV inset, not a canvas ring")
    print(f"art: --pad {args.pad} -> {done} map(s) shrunk to fit, "
          f"edges replicated into the {args.pad}px guard{note}")


if __name__ == "__main__":
    main()
