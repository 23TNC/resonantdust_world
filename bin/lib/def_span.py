#!/usr/bin/env python3
"""Resolve a texture stem's FRAME SPAN in tiles from the DSL corpus.

The span is what decides the pow2 square a leaf packs into (`span * TILE_PX`), and the corpus is
where it is authored — `content/**/*.rd`, as `<n> &thing.span set` inside the def that also names
the texture with `"<stem> &thing.texture set`. This module only READS it. Nothing here may become a
second place to author a span; see the stream's F1.

Two traps this deliberately avoids:

  * **`span` is not `size`.** Both are "in tiles" and they mean different things — `size` is the
    sprite's draw scale (the wolf is 1.125, which is not even pow2), `span` is the frame's world
    extent. Sizing a texture off `size` would be wrong for every non-integer def.
  * **`span` is not `footprint`.** `footprint (w,h)` is the tiles a prim OCCUPIES for movement and
    hit-testing. The conifer is `footprint 1x1, span 2` — it occupies one tile and draws over two.
    Deriving the square from the footprint would size it 128 instead of 256.

`span` is documented as pow2 (VARIABLES.md `frame_span`) but the loader never validates it, so this
rounds UP to the next pow2 and reports when it had to — better a too-large frame than a fractional
px-per-unit.

  python3 bin/lib/def_span.py biome-thing/default/conifer   # -> 2
  python3 bin/lib/def_span.py --all                          # every stem it can resolve
"""
import argparse, glob, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))

DEF_RE = re.compile(r'^\s*::([\w-]+)>')
TEX_RE = re.compile(r'"([\w/.-]+)\s+&thing\.texture set')
SPAN_RE = re.compile(r'([\d.]+)\s+&thing\.span set')
FOOT_RE = re.compile(r'([\d.]+)\s+&thing\.footprint\.([wh]) set')


def next_pow2(n):
    p = 1
    while p < n:
        p *= 2
    return p


def scan(root=None):
    """{texture_stem: {'def','span','span_authored','footprint','file'}} for every def naming a texture."""
    root = root or os.path.join(REPO, "content")
    out = {}
    for path in sorted(glob.glob(os.path.join(root, "**", "*.rd"), recursive=True)):
        cur, blocks = None, {}
        for line in open(path):
            m = DEF_RE.match(line)
            if m:
                cur = m.group(1)
                blocks.setdefault(cur, {"def": cur, "file": os.path.relpath(path, REPO),
                                        "footprint": [1.0, 1.0]})
            if cur is None:
                continue
            b = blocks[cur]
            t = TEX_RE.search(line)
            if t:
                b["texture"] = t.group(1)
            s = SPAN_RE.search(line)
            if s:
                b["span_authored"] = float(s.group(1))
            f = FOOT_RE.search(line)
            if f:
                b["footprint"][0 if f.group(2) == "w" else 1] = float(f.group(1))
        for b in blocks.values():
            stem = b.get("texture")
            # `white` is the flat placeholder, not a texture tree path — it has no leaf to size.
            if not stem or stem == "white":
                continue
            out[stem] = b
    return out


def span_for(stem, table=None):
    """(span, authored) — the pow2 frame span for `stem`, and whether the corpus declared it.

    Returns (None, False) when the stem is unknown to the corpus: the caller must then fall back
    (stream F5) rather than assume 1, because a silent 1 would size every unlisted leaf to one tile.
    """
    t = table if table is not None else scan()
    b = t.get(stem)
    if b is None:
        return None, False
    raw = b.get("span_authored")
    if raw is None:
        return None, False
    return next_pow2(int(raw)) if raw == int(raw) else next_pow2(int(raw) + 1), True


def main():
    ap = argparse.ArgumentParser(prog="def_span")
    ap.add_argument("stem", nargs="?")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--tile-px", type=int, default=int(os.environ.get("RD_TILE_PX", 128)))
    args = ap.parse_args()
    table = scan()
    if args.all:
        print(f"{'texture stem':<34} {'def':<12} {'span':>5} {'square':>7}  footprint")
        for stem, b in sorted(table.items()):
            sp, authored = span_for(stem, table)
            sq = f"{sp * args.tile_px}" if sp else "—"
            mark = "" if authored else "  (no span authored)"
            print(f"{stem:<34} {b['def']:<12} {sp or '—':>5} {sq:>7}  "
                  f"{b['footprint'][0]:g}x{b['footprint'][1]:g}{mark}")
        return
    if not args.stem:
        ap.error("give a texture stem or --all")
    sp, authored = span_for(args.stem, table)
    if sp is None:
        sys.exit(1)          # unknown / unauthored: caller falls back
    print(sp)


if __name__ == "__main__":
    main()
