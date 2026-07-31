#!/usr/bin/env python3
"""Mark a texture leaf as HAND-AUTHORED so `art remaster` refuses to regenerate it.

`remaster` is destructive by design: it re-keys the source sheet, re-splits it into variant
leaves and re-derives every map, overwriting whatever was there. That is exactly right for
generated art and exactly wrong for a leaf someone painted, hand-corrected, or tuned by eye —
and the two live side by side in the same tree, so a broad `art remaster <type>/<subtype>`
would take the hand-authored ones with it. Nothing in the tree recorded which was which.

The lock lives in the leaf's own `meta.json` (the same sidecar that already carries
`normal_engine`, `span`, `outline`, `channel_tints`), under `locked`:

    {"locked": true, "locked_note": "hand-painted face; do not regenerate"}

`meta.json` is the right home because it TRAVELS WITH THE LEAF — `textures/` is gitignored,
so a lock list kept anywhere else would be the copy that goes stale. A leaf that is moved or
copied keeps its lock; one that is regenerated from scratch has no lock, which is the correct
default (nothing is protected until someone says so).

    leaf_lock.py check <root>...          -> list locked leaves under the roots; exit 3 if any
    leaf_lock.py set <root> [--note TEXT] -> lock every leaf under root
    leaf_lock.py clear <root>             -> unlock every leaf under root
    leaf_lock.py list <root>              -> list locked leaves (exit 0 either way)

A LEAF is a directory holding a `meta.json`. `set`/`clear` walk the whole subtree, so a root
may be one leaf, a kind, or a whole type — the same granularity `remaster` itself takes.
"""
import argparse
import json
import os
import sys

KEY = "locked"
NOTE_KEY = "locked_note"
REPO = os.environ.get("RD_REPO_ROOT", os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))


def leaves(root):
    """Every directory at or under `root` holding a `meta.json`."""
    if os.path.isfile(root):
        root = os.path.dirname(root)
    out = []
    for dirpath, _dirs, files in os.walk(root):
        if "meta.json" in files:
            out.append(dirpath)
    return sorted(out)


def _read(leaf):
    try:
        with open(os.path.join(leaf, "meta.json")) as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return {}


def _write(leaf, d):
    with open(os.path.join(leaf, "meta.json"), "w") as f:
        json.dump(d, f, separators=(",", ":"), sort_keys=True)
        f.write("\n")


def is_locked(leaf):
    return bool(_read(leaf).get(KEY))


def locked_under(roots):
    """Every locked leaf under any of `roots`, as (leaf, note) pairs — deduped and sorted."""
    seen, out = set(), []
    for root in roots:
        for leaf in leaves(root):
            if leaf in seen or not is_locked(leaf):
                continue
            seen.add(leaf)
            out.append((leaf, _read(leaf).get(NOTE_KEY) or ""))
    return sorted(out)


def rel(p):
    try:
        return os.path.relpath(p, REPO)
    except ValueError:
        return p


def main():
    ap = argparse.ArgumentParser(description="Lock texture leaves against `art remaster`.")
    ap.add_argument("cmd", choices=["check", "list", "set", "clear"])
    ap.add_argument("roots", nargs="+", help="leaf / kind / type directories (or a file inside one)")
    ap.add_argument("--note", default="", help="why this is locked (stored as locked_note)")
    a = ap.parse_args()

    if a.cmd in ("check", "list"):
        hits = locked_under(a.roots)
        for leaf, note in hits:
            print(f"  {rel(leaf)}{'  — ' + note if note else ''}")
        # `check` is the GATE: a nonzero exit is the signal, so a caller can branch on it
        # without parsing stdout. `list` is the human read and never fails.
        return 3 if (hits and a.cmd == "check") else 0

    lock = a.cmd == "set"
    n = 0
    for root in a.roots:
        for leaf in leaves(root):
            d = _read(leaf)
            if lock:
                d[KEY] = True
                if a.note:
                    d[NOTE_KEY] = a.note
            else:
                d.pop(KEY, None)
                d.pop(NOTE_KEY, None)
            _write(leaf, d)
            n += 1
    if n == 0:
        print(f"art: no leaves under {', '.join(rel(r) for r in a.roots)} (a leaf is a dir with a meta.json)", file=sys.stderr)
        return 1
    print(f"art: {'locked' if lock else 'unlocked'} {n} leaf/leaves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
