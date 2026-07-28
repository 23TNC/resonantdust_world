#!/usr/bin/env python3
"""Stamp each variant leaf's FRAME SPAN and packing square into its `meta.json`.

`span` decides the pow2 square a leaf packs into (`span * TILE_PX`). It is AUTHORED in the DSL
corpus (`thing.span`) and merely CACHED here, so the texture tree never becomes a second place to
change it — `textures/` is gitignored, which would make that copy the unversioned one.

Resolution order for a kind:

  1. the corpus, via `def_span` — the authoritative answer;
  2. failing that, the leaf's own `atlas.json` — a held-whole atlas STATES its tile count
     (`cols x rows`, or generate_tile's `grid`), which is a fact about the file rather than a
     measurement of it;
  3. failing that, INFER from the art: `next_pow2(ceil(max(w,h) / TILE_PX))` over the leaf's
     diffuse, stamped `span_inferred: true` and warned about.

The fallback is not an edge case. Exactly one def in the corpus authors a span today (stream I5),
so inference is what runs for nearly everything — which is why it is marked in the output rather
than applied silently: the marked leaves ARE the worklist of defs still to be authored.

  python3 bin/lib/leaf_span.py textures/biome-thing/default/conifer
  python3 bin/lib/leaf_span.py --all
"""
import argparse, glob, json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import def_span, meta, texpath
from PIL import Image

REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
TILE_PX = int(os.environ.get("RD_TILE_PX", 128))
TEX = os.path.join(REPO, "textures")


def stem_of(leaf):
    """The texture stem (`<type>/<subtype>/<kind>`) a variant leaf belongs to — its path minus the
    variant folder. Matches what a def's `&thing.texture set` names."""
    rel = os.path.relpath(leaf, TEX)
    parts = rel.split(os.sep)
    return "/".join(parts[:-1]) if len(parts) > 1 else rel


def atlas_span(leaf):
    """Span from the leaf's own `atlas.json`, for a held-whole atlas.

    An atlas states its tile count directly: a linked form is `cols x rows` cells and a ground sheet
    is `grid x grid`, so the texture is exactly that many tiles across. That is a FACT about the
    file, not an inference from its pixel size, and it outranks measuring the art — a 320px 4x4
    linked atlas and a 512px one are both 4 tiles wide."""
    p = os.path.join(leaf, "atlas.json")
    if not os.path.exists(p):
        return None
    try:
        d = json.load(open(p))
    except (json.JSONDecodeError, OSError):
        return None
    if "cols" in d and "rows" in d:                 # the manifest's schema
        return def_span.next_pow2(max(int(d["cols"]), int(d["rows"])))
    g = d.get("grid")                                # generate_tile's schema
    if isinstance(g, (list, tuple)) and len(g) == 2:
        return def_span.next_pow2(max(int(g[0]), int(g[1])))
    return None


def infer_span(leaf):
    """Smallest pow2 tile span that contains the leaf's art, from any map present."""
    for cand in sorted(glob.glob(os.path.join(leaf, "diffuse.*.png"))) or \
                sorted(glob.glob(os.path.join(leaf, "albedo.*.png"))):
        try:
            w, h = Image.open(cand).size
        except OSError:
            continue
        return def_span.next_pow2(max(1, math.ceil(max(w, h) / TILE_PX)))
    return None


def stamp(leaf, table, quiet=False):
    stem = stem_of(leaf)
    span, authored = def_span.span_for(stem, table)
    source, inferred = "corpus", False
    if span is None:
        span, source = atlas_span(leaf), "atlas"
    if span is None:
        span, source, inferred = infer_span(leaf), "art", True
        if span is None:
            return None
        if not quiet:
            print(f"art: leaf-span: {stem} declares no `thing.span` — inferred {span} "
                  f"({span * TILE_PX}px) from the art. Author it in the corpus to make it exact.",
                  file=sys.stderr)
    # meta.update keys off a MAP path in the leaf, so hand it any map that exists
    anchor = (sorted(glob.glob(os.path.join(leaf, "diffuse.*.png"))) or
              sorted(glob.glob(os.path.join(leaf, "*.png"))))
    if not anchor:
        return None
    keys = {"span": span, "square": span * TILE_PX, "tile_px": TILE_PX,
            "span_from": source, "span_inferred": inferred}
    meta.update(anchor[0], **keys)
    return keys


def leaves_under(root):
    """Every variant leaf under `root` — a directory holding at least one `<map>.<dir>.<part>.png`."""
    out = []
    for dirpath, _d, files in os.walk(root):
        if any(texpath.parse_map(f) for f in files):
            out.append(dirpath)
    return sorted(out)


def main():
    ap = argparse.ArgumentParser(prog="art leaf-span")
    ap.add_argument("paths", nargs="*", help="kind dirs or variant leaves (default: whole tree)")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    roots = [os.path.abspath(p) for p in args.paths] or [TEX]
    if args.all:
        roots = [TEX]
    table = def_span.scan()
    n = inf = 0
    src = {}
    for r in roots:
        for leaf in leaves_under(r):
            k = stamp(leaf, table, args.quiet)
            if k:
                n += 1
                inf += 1 if k["span_inferred"] else 0
                src[k["span_from"]] = src.get(k["span_from"], 0) + 1
    detail = ", ".join(f"{v} from the {k}" for k, v in sorted(src.items()))
    print(f"art: leaf-span -> {n} leaf/leaves stamped at {TILE_PX}px tiles ({detail})")


if __name__ == "__main__":
    main()
