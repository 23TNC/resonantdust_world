#!/usr/bin/env python3
"""Fold the `linked/` texture tree into `biome-tile/` (texture-restructure R2).

Per texture-layout/README.md §5 — a linked object `<form>.<material>` becomes a
biome-tile object: **material → kind, form → variant, biome → subtype (`default`)**:

    linked/<form>.<material>/…  →  biome-tile/default/<material>/<form>/…

The form IS the (named) variant folder. Per linked kind:
  * a **held-whole atlas** variant (has `atlas.json`) → its contents become the form
    leaf directly (`biome-tile/default/<material>/<form>/<map>.l.0.png` + `atlas.json`).
  * **other** content (superseded per-cell dirs, raw source atlases / psd) is **moved,
    not dropped**, under the form leaf — these kinds are un-mastered art that needs a
    `bin/art` re-master to a held-whole atlas; preserving it loses nothing.

In-place under textures/ (gitignored → backed up out-of-tree first). Dry-run default;
`--apply` performs it and removes the emptied `linked/` tree.
"""
import os
import shutil
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LINKED = os.path.join(REPO, "textures", "linked")
DEST = os.path.join(REPO, "textures", "biome-tile", "default")


def plan():
    moves = []          # (src_abs, dst_abs)
    unmastered = set()  # form leaves that still need a re-master
    for kd in sorted(os.listdir(LINKED)):
        kind_dir = os.path.join(LINKED, kd)
        if not os.path.isdir(kind_dir):
            continue
        if "." not in kd:
            print(f"  skip (not <form>.<material>): {kd}", file=sys.stderr)
            continue
        form, material = kd.split(".", 1)
        target = os.path.join(DEST, material, form)
        rel = f"biome-tile/default/{material}/{form}"
        for child in sorted(os.listdir(kind_dir)):
            cp = os.path.join(kind_dir, child)
            if os.path.isdir(cp):
                if os.path.exists(os.path.join(cp, "atlas.json")):       # held-whole atlas
                    for f in sorted(os.listdir(cp)):
                        moves.append((os.path.join(cp, f), os.path.join(target, f)))
                else:                                                    # superseded/unmastered
                    for dp, _dn, fns in os.walk(cp):
                        for f in fns:
                            src = os.path.join(dp, f)
                            moves.append((src, os.path.join(target, os.path.relpath(src, kind_dir))))
                    unmastered.add(rel)
            elif os.path.isfile(cp):                                     # kind-level source file
                moves.append((cp, os.path.join(target, child)))
                unmastered.add(rel)
    return moves, unmastered


def main(argv):
    if not os.path.isdir(LINKED):
        print("no textures/linked/ (already folded?)")
        return 0
    apply = "--apply" in argv
    moves, unmastered = plan()
    print(f"=== migrate_fold: {len(moves)} files → biome-tile/, {len(unmastered)} kinds need re-master ===")
    for src, dst in moves[:14]:
        print(f"  {os.path.relpath(src, REPO)}\n    -> {os.path.relpath(dst, REPO)}")
    if len(moves) > 14:
        print(f"  … +{len(moves) - 14} more")
    if unmastered:
        print("  un-mastered (need `bin/art` re-master to a held-whole atlas):")
        for u in sorted(unmastered):
            print(f"    {u}")
    if not apply:
        print("\n(dry-run — pass --apply)")
        return 0
    for src, dst in moves:
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.move(src, dst)
    shutil.rmtree(LINKED)
    print(f"\napplied: {len(moves)} files folded into biome-tile/default/; linked/ removed.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
