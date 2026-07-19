"""Canonical texture leaf paths (bin/lib side).

Shape source-of-truth: docs/components/dev/textures/design/texture-layout/. Full path:

    <type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>

The trailing `<variant>/` is the "variant leaf": ONE folder holding every co-located
file for that variant, so a plain `ls` groups by map → dir → part. Two file kinds live
there:

  * **directional maps** `<map>.<dir>.<part>.<ext>` — albedo / normal / diffuse /
    emissive / layers / sprite / template / …  (dir ∈ s/e/n/l, part = sprite piece).
  * **per-variant files** `<name>.<ext>` — `meta.json` (outline/bbox/tints), and for
    a linked/autotile kind `atlas.json` (the held-whole grid sidecar). No dir/part.

No `<id>` or `<subkind>` segment (both dropped vs the old
`<id>.<dir>.<layer>/<variant>/<map>.<ext>` leaf). marigold/delight.py re-implements this
shape natively (separate venv) — keep the two in step.
"""
import os

# Normalize-on-write defaults for the fields a sparse reference omits.
DIR, PART, VARIANT = "s", "0", "1"


def variant_leaf(variant=VARIANT):
    """`<variant>` — the single variant-leaf folder (was `<id>.<dir>.<layer>/<variant>`)."""
    return str(variant)


def map_name(map, dir=DIR, part=PART, ext="png"):
    """A directional map's in-leaf filename: `map_name('albedo','e','1')` -> `albedo.e.1.png`."""
    return f"{map}.{dir}.{part}.{ext}"


def parse_map(filename):
    """Split a directional leaf filename `<map>.<dir>.<part>.<ext>` -> (map, dir, part, ext),
    or None if it isn't one (e.g. a per-variant `meta.json`)."""
    parts = filename.split(".")
    if len(parts) != 4:
        return None
    return (parts[0], parts[1], parts[2], parts[3])


def sibling(a_map_path, map, ext="png"):
    """The `<map>.<dir>.<part>.<ext>` sibling of a directional map in the same leaf, at the
    SAME dir/part, e.g. `sibling('…/3/albedo.e.1.png', 'normal')` -> `…/3/normal.e.1.png`."""
    p = parse_map(os.path.basename(a_map_path))
    d, part = (p[1], p[2]) if p else (DIR, PART)
    return os.path.join(os.path.dirname(a_map_path), map_name(map, d, part, ext))


def leaf_file(a_map_path, name, ext):
    """A **per-variant** file (no dir/part) beside `a_map_path` in the same variant leaf:
    `leaf_file('…/3/albedo.e.1.png', 'meta', 'json')` -> `…/3/meta.json`."""
    return os.path.join(os.path.dirname(a_map_path), f"{name}.{ext}")


def find_maps(root, map, ext="png"):
    """Every directional `<map>.<dir>.<part>.<ext>` file under `root` (sorted). The map is
    the FIRST dotted segment; matches any dir/part."""
    prefix, suffix = f"{map}.", f".{ext}"
    hits = []
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            if f.startswith(prefix) and f.endswith(suffix) and f.count(".") == 3:
                hits.append(os.path.join(dirpath, f))
    return sorted(hits)


def is_map(a_map_path, map, ext="png"):
    """True if `a_map_path` is a `<map>.<dir>.<part>.<ext>` leaf file for `map`."""
    p = parse_map(os.path.basename(a_map_path))
    return p is not None and p[0] == map and p[3] == ext


def flat_name(a_map_path):
    """A collision-free flat name for `--out-dir` mode: `<kind>.<variant>.<map>.<dir>.<part>.<ext>`
    — the kind + variant folder names prepended to the leaf filename."""
    fn = os.path.basename(a_map_path)
    variant = os.path.basename(os.path.dirname(a_map_path))
    kind = os.path.basename(os.path.dirname(os.path.dirname(a_map_path)))
    return f"{kind}.{variant}.{fn}"
