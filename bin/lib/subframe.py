#!/usr/bin/env python3
"""Measure a stem's MINIMUM opaque bbox per variant and emit the `.rd` subframe lines.

Work `docs/work/2026-08-02-subframe-ingest`. The subframe says which fraction of a letterboxed
square master is actually art; the atlas crops to exactly that rect, identically for all four maps.

**Why this exists rather than the client measuring it.** The runtime bbox is computed from whichever
lod happened to decode first, so the same art measures differently between sessions (stream I7). A
number that placement is registered against cannot move like that. The masters on disk are one size
(`meta.json` `"square": 128`) and measure the same every time, so authoring is done from here and the
result is committed to the corpus.

**Regenerate, do not hand-edit.** The emitted lines are long and numerous; editing one by hand is how
an authored geometry number silently stops matching its art (stream I6).

    python3 bin/lib/subframe.py biome-thing/default/conifer
    python3 bin/lib/subframe.py pawn/animal/wolf --by-direction

Output is TOML entries for the def's `[thing.part.subframe]` table in
content/things.toml (toml-content P5; schema in docs/VARIABLES.md).

Coverage is the SURFACE map's B channel thresholded at 0.35 — the shader's own constant, so the
authored rect describes the same silhouette the renderer silhouette-tests against.
"""
import argparse
import os
import sys

try:
    from PIL import Image
except ImportError:  # pragma: no cover - dev tool
    sys.exit("subframe.py needs Pillow: pip install Pillow")

# The shader's coverage test is `surface.b > 0.35`; 0.35 * 255 = 89.25.
COVERAGE_MIN = 89
TEXTURES = "textures"


def bbox(path):
    """The opaque bbox of one map as `(x, y, w, h)` fractions, or None if fully transparent."""
    im = Image.open(path).convert("RGB")
    w, h = im.size
    bb = im.split()[2].point(lambda v: 255 if v > COVERAGE_MIN else 0).getbbox()
    if not bb:
        return None
    x0, y0, x1, y1 = bb
    return (round(x0 / w, 4), round(y0 / h, 4), round((x1 - x0) / w, 4), round((y1 - y0) / h, 4))


def union(rects):
    """The rect containing every input — the fallback for an index past the mastered set."""
    x0 = min(r[0] for r in rects)
    y0 = min(r[1] for r in rects)
    x1 = max(r[0] + r[2] for r in rects)
    y1 = max(r[1] + r[3] for r in rects)
    return (round(x0, 4), round(y0, 4), round(x1 - x0, 4), round(y1 - y0, 4))


def measure(stem, direction, part):
    """`(variant -> rect)` for every variant folder of `stem` that ships a surface map."""
    root = os.path.join(TEXTURES, stem)
    if not os.path.isdir(root):
        sys.exit(f"no such stem: {root}")
    out = {}
    for name in sorted((d for d in os.listdir(root) if d.isdigit()), key=int):
        path = os.path.join(root, name, f"surface.{direction}.{part}.png")
        if not os.path.exists(path):
            continue
        rect = bbox(path)
        if rect:
            out[int(name)] = rect
    return out


def emit(handle, rects, indent, by_direction, direction):
    """The `.rd` lines. `v<n>` per VARIANT, or aliased by direction for a facing set.

    Two axes (stream I10): a rotation/facing lands in the STEM, a variant in the CELL. Emitting a
    variant as `r<n>` put the wolf's south rect on its east art.
    """
    # TOML entries for a `[thing.part.subframe]` / `[[thing.part]]` table
    # (toml-content P5 — the corpus is TOML now; schema in docs/VARIABLES.md).
    def rect(r):
        x, y, w, h = r
        return f"{{ x = {x}, y = {y}, w = {w}, h = {h} }}"

    lines = []
    if by_direction:
        # A facing set authors ONE rect per direction, under the s/e/n aliases — index 3
        # (west) is deliberately absent: it is the east master mirrored, derived host-side.
        lines.append(f'{indent}"{direction}" = {rect(union(list(rects.values())))}')
        return lines
    lines.append(f'{indent}"default" = {rect(union(list(rects.values())))}')
    for idx in sorted(rects):
        lines.append(f'{indent}"v{idx}" = {rect(rects[idx])}')
    return lines


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("stem", help="texture stem, e.g. biome-thing/default/conifer")
    ap.add_argument("--dir", default="e", dest="direction", help="master direction segment (default e)")
    ap.add_argument("--part", default="0", help="master part segment (default 0)")
    ap.add_argument("--indent", type=int, default=2, help="leading spaces (default 2)")
    ap.add_argument("--by-direction", action="store_true",
                    help="emit ONE aliased rect for this direction instead of per-variant indices")
    a = ap.parse_args()

    rects = measure(a.stem, a.direction, a.part)
    if not rects:
        sys.exit(f"{a.stem}: no surface.{a.direction}.{a.part}.png in any variant folder")

    areas = {i: r[2] * r[3] for i, r in rects.items()}
    lo, hi = min(areas.values()), max(areas.values())
    print(f"# {a.stem} — {len(rects)} variant(s), opaque area {lo:.3f}..{hi:.3f}, "
          f"union {union(list(rects.values()))[2] * union(list(rects.values()))[3]:.3f}", file=sys.stderr)
    print("\n".join(emit(None, rects, " " * a.indent, a.by_direction, a.direction)))


if __name__ == "__main__":
    main()
