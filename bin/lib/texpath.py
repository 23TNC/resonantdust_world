"""Canonical folder-per-variant texture paths (bin/lib side).

The single source of truth for the *shape* is docs/texture-paths.md. Leaf layout:

    <category>.<subcategory>/<kind>.<subkind>/<id>.<dir>.<layer>/<variant>/<map>.<ext>

The trailing `<id>.<dir>.<layer>/<variant>/` is the "variant leaf": every
co-located map for that variant is a `<map>.<ext>` file inside it. A map is
discovered and addressed by its FILENAME (`albedo.png`, `normal.png`, …), NOT by
a dotted suffix on a flat filename as in the pre-migration layout.

The marigold venv (marigold/delight.py) re-implements the same trivial shape
natively — it can't import across venvs — so keep the two in step with the doc.
"""
import os

# Normalize-on-write defaults for the fields a sparse reference omits. Match the
# server's canonical instance (1.s.0.1) and bin/art's _parse_sheet_ref.
DIR, LAYER, VARIANT = "s", "0", "1"


def variant_leaf(id, dir=DIR, layer=LAYER, variant=VARIANT):
    """`<id>.<dir>.<layer>/<variant>` — the two-level variant-leaf subpath."""
    return os.path.join(f"{id}.{dir}.{layer}", str(variant))


def map_name(map, ext="png"):
    """The in-leaf file name for a map, e.g. `map_name('albedo')` -> `albedo.png`."""
    return f"{map}.{ext}"


def sibling(a_map_path, map, ext="png"):
    """The `<map>.<ext>` sibling of a map file in the same variant leaf, e.g.
    `sibling('…/1.s.0/1/albedo.png', 'normal')` -> `…/1.s.0/1/normal.png`."""
    return os.path.join(os.path.dirname(a_map_path), map_name(map, ext))


def find_maps(root, map, ext="png"):
    """Every variant-leaf `<map>.<ext>` file under `root` (sorted). Replaces the
    old `*.<map>.png` glob — the map is now an exact filename in a leaf dir."""
    want = map_name(map, ext)
    hits = []
    for dirpath, _dirs, files in os.walk(root):
        if want in files:
            hits.append(os.path.join(dirpath, want))
    return sorted(hits)


def is_map(a_map_path, map, ext="png"):
    """True if `a_map_path` is a `<map>.<ext>` leaf file (basename match)."""
    return os.path.basename(a_map_path) == map_name(map, ext)


def flat_name(a_map_path, map, ext="png"):
    """A collision-free flat name for `--out-dir` mode: `<kind>.<pose>.<variant>.<map>.<ext>`
    where kind/pose/variant are the leaf's last three path components — reconstructs
    the pre-migration flat identity so spikes over many objects don't collide."""
    variant = os.path.basename(os.path.dirname(a_map_path))
    pose = os.path.basename(os.path.dirname(os.path.dirname(a_map_path)))
    kind = os.path.basename(os.path.dirname(os.path.dirname(os.path.dirname(a_map_path))))
    return f"{kind}.{pose}.{variant}.{map_name(map, ext)}"
