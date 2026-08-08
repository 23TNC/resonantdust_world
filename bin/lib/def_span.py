#!/usr/bin/env python3
"""Resolve a texture stem's FRAME SPAN in tiles from the TOML corpus.

The span is what decides the pow2 square a leaf packs into (`span * TILE_PX`), and the corpus is
where it is authored — `content/things.toml`, as `span = <n>` inside the `[[thing.part]]` that also
names the texture with `texture = "<stem>"`. This module only READS it. Nothing here may become a
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

**Why a hand-rolled reader.** The host runs Python 3.10, which predates `tomllib`, and `bin/` tools
must work on a bare checkout with no pip installs. So this walks the corpus line by line, tracking
which TOML table it is inside, and reads three keys out of `[[thing.part]]`. It is NOT a TOML
parser and must not grow into one — if it ever needs more than these keys, call the real loader.
The one hard rule it enforces is [I4](../../docs/work/2026-08-04-content-toml-only/issues.md): an
empty result is a LOUD error, never a silent zero. The old `.rd` reader failed silently for a whole
migration and cost every freshly mastered leaf its span stamp.

  python3 bin/lib/def_span.py biome-thing/default/conifer   # -> 2
  python3 bin/lib/def_span.py --all                          # every stem it can resolve
"""
import argparse, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))

# A TOML table header: `[[thing]]`, `[[thing.part]]`, `[thing.part.subframe]`. Captures the dotted
# path so nested tables (subframes carry their own `x`/`w` keys) never leak into a part's fields.
TABLE_RE = re.compile(r'^\s*\[\[?([\w.]+)\]?\]')
STR_RE = re.compile(r'^\s*(\w+)\s*=\s*"([^"]*)"')
NUM_RE = re.compile(r'^\s*(\w+)\s*=\s*([\d.]+)\s*$')
# `footprint = { w = 1.0, h = 2.0 }` — an inline table, the only one this reads.
FOOT_RE = re.compile(r'^\s*footprint\s*=\s*\{([^}]*)\}')
FOOT_KV_RE = re.compile(r'(\w+)\s*=\s*([\d.]+)')
# `subType = ["default"]` — the first element is the stem's middle segment. An array, so STR_RE
# (which wants a bare `key = "value"`) cannot see it.
ARR_RE = re.compile(r'^\s*(\w+)\s*=\s*\[\s*"([^"]*)"')


def next_pow2(n):
    p = 1
    while p < n:
        p *= 2
    return p


def scan(path=None):
    """{texture_stem: {'def','span_authored','footprint','file'}} for every part naming a texture.

    Keyed by stem, so two parts of one thing sharing a texture (the human's body + head) collapse to
    one row. When they disagree on `span` the LARGER wins — an undersized frame crops the art, an
    oversized one only wastes texels.
    """
    path = path or os.path.join(REPO, "content", "things.toml")
    out = {}
    for thing_def, parts, derived in _things(path):
        for p in parts:
            # The stem is DERIVED from the def's taxonomy (`<type>/<subType[0]>/<kind>`) — the
            # corpus stopped authoring `texture = "<stem>"` on parts when defs became
            # registry-numbered with a derived taxonomy. An explicit `texture` still wins where
            # one is authored. Parts of one thing SHARE the stem; they differ by the `.<part>`
            # filename segment, not by folder.
            stem = p.get("texture") or derived
            # `white` is the flat placeholder, not a texture tree path — it has no leaf to size.
            if not stem or stem == "white":
                continue
            row = out.setdefault(
                stem,
                {"def": thing_def, "file": os.path.relpath(path, REPO), "footprint": p["footprint"]},
            )
            span = p.get("span_authored")
            if span is not None:
                row["span_authored"] = max(row.get("span_authored", 0.0), span)
    if not out:
        # Exit 2, not 1: exit 1 means "this stem has no authored span, fall back" and callers
        # silence it. A stale reader must NOT hide inside that. See content-toml-only I4.
        print(
            f"def_span: {os.path.relpath(path, REPO)} yielded no texture-bearing defs — "
            "the corpus moved or this reader is stale (see content-toml-only I4)",
            file=sys.stderr,
        )
        raise SystemExit(2)
    return out


def _things(path):
    """[(thing_name, [part, …], derived_stem), …] — the corpus grouped, subframe tables skipped.

    `derived_stem` is `<type>/<subType[0]>/<kind>`, which is what the texture tree is laid out by
    and what a part with no explicit `texture` resolves to. None when the def names none of the
    three (nothing in the tree to size)."""
    things, table, cur = [], None, None
    with open(path) as fh:
        for line in fh:
            m = TABLE_RE.match(line)
            if m:
                table = m.group(1)
                if table == "thing":
                    cur = ["?", [], {}]
                    things.append(cur)
                elif table == "thing.part" and cur is not None:
                    cur[1].append({"footprint": [1.0, 1.0]})
                continue
            if cur is None:
                continue
            if table == "thing":
                s = STR_RE.match(line)
                if s and s.group(1) == "name":
                    cur[0] = s.group(2)
                if s and s.group(1) in ("type", "kind"):
                    cur[2][s.group(1)] = s.group(2)
                a = ARR_RE.match(line)
                if a and a.group(1) == "subType":
                    cur[2]["subType"] = a.group(2)
            elif table == "thing.part" and cur[1]:
                part = cur[1][-1]
                s = STR_RE.match(line)
                if s and s.group(1) == "texture":
                    part["texture"] = s.group(2)
                n = NUM_RE.match(line)
                if n and n.group(1) == "span":
                    part["span_authored"] = float(n.group(2))
                f = FOOT_RE.match(line)
                if f:
                    for k, v in FOOT_KV_RE.findall(f.group(1)):
                        if k in ("w", "h"):
                            part["footprint"][0 if k == "w" else 1] = float(v)
    return [(name, parts, _stem_of(tax)) for name, parts, tax in things]


def _stem_of(tax):
    """`<type>/<subType[0]>/<kind>` — the texture tree path a def's art lives under, or None."""
    t, s, k = tax.get("type"), tax.get("subType"), tax.get("kind")
    return "/".join((t, s, k)) if t and s and k else None


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
