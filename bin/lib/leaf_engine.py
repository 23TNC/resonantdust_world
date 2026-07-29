#!/usr/bin/env python3
"""Remember which normal engine made a leaf's normal map, so a re-remaster does not silently
change it.

The engine is a real choice with measured consequences, not a detail. On the `smooth/wall`
autotile atlas, laigter's luminance height field discontinues across the interior cell
boundaries the client crosses constantly (mean |delta| 5.58) while Marigold's learned view-space
normals are nearly continuous there (0.57) — but laigter invents 3x the relief, which is what you
want on art that really is embossed. Neither is correct in general, so the choice has to survive
being made.

Before this, `engine=laigter` was a hardcoded default in `cmd_normal` and `cmd_remaster` and the
choice lived only in the shell history of whoever ran it. Recording it in the leaf makes the tree
self-describing: `art normal --marigold` stamps `normal_engine`, and a later remaster reads it
back rather than reverting to the default.

  leaf_engine.py get <root>            -> "laigter" | "marigold" | "mixed" | "none"
  leaf_engine.py set <root> <engine>   -> stamp every leaf under root that has a normal
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath

KEY = "normal_engine"


def leaves_with_normal(root):
    """Every directory under `root` holding a `normal.<dir>.<part>.png`."""
    out = []
    for dirpath, _d, files in os.walk(root):
        if any(texpath.is_map(f, "normal") for f in files):
            out.append(dirpath)
    return sorted(out)


def read(leaf):
    p = os.path.join(leaf, "meta.json")
    if not os.path.exists(p):
        return None
    try:
        return json.load(open(p)).get(KEY)
    except (json.JSONDecodeError, OSError):
        return None


def write(leaf, engine):
    p = os.path.join(leaf, "meta.json")
    d = {}
    if os.path.exists(p):
        try:
            d = json.load(open(p))
        except (json.JSONDecodeError, OSError):
            d = {}
    d[KEY] = engine
    with open(p, "w") as f:
        json.dump(d, f, indent=1)


def main():
    if len(sys.argv) < 3:
        print(__doc__.strip().splitlines()[-2].strip(), file=sys.stderr)
        return 2
    cmd, root = sys.argv[1], os.path.abspath(sys.argv[2])

    if cmd == "get":
        vals = {read(l) for l in leaves_with_normal(root)}
        vals.discard(None)
        # Unanimity is required to auto-honour: a kind whose leaves disagree must be told
        # explicitly, rather than have one leaf's choice silently imposed on its siblings.
        print("none" if not vals else (vals.pop() if len(vals) == 1 else "mixed"))
        return 0

    if cmd == "set":
        if len(sys.argv) < 4:
            print("leaf_engine: set needs an engine", file=sys.stderr)
            return 2
        engine = sys.argv[3]
        n = 0
        for l in leaves_with_normal(root):
            write(l, engine)
            n += 1
        print(f"art: recorded normal_engine={engine} in {n} leaf/leaves")
        return 0

    print(f"leaf_engine: unknown command '{cmd}'", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
