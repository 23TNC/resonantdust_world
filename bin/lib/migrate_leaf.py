#!/usr/bin/env python3
"""Migrate `<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>` leaves to the new
`<kind>/<variant>/<map>.<dir>.<part>.<ext>` shape (texture-restructure P3).

This first pass handles the **clean `subkind == "default"`** kinds (biome-thing + the
default-subkind pawns like wolf): drop `<subkind>` + `<id>`, fold `<dir>`/`<part>` into
the filename, collapse the direction/part folders into ONE `<variant>/` folder.

  * directional maps `<map>.<ext>`  → `<variant>/<map>.<dir>.<part>.<ext>`
  * per-variant `*.json` (meta.json) → `<variant>/<name>`  (no dir/part)

**Skipped** (phase 2, need per-kind decisions): `subkind != "default"` (human pawn
body-types collide on a blind drop) and `linked/` (held-whole atlas + registry fold).

In-place under textures/ (backed up out-of-tree first — textures/ is gitignored, no git
rollback). Dry-run by default; `--apply` performs it. Usage:

    python3 bin/lib/migrate_leaf.py [--apply] [textures/biome-thing textures/pawn ...]
"""
from __future__ import annotations
import os
import shutil
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def _is_iddirpart(name: str) -> bool:
    """A `<id>.<dir>.<part>` folder, e.g. `1.e.0`."""
    p = name.split(".")
    return len(p) == 3 and p[0].isdigit()


def plan(kind_dir: str):
    """Yield (src_abs, dst_abs) moves for one `<type>/<subtype>/<kind>` dir, or raise for
    a non-default subkind (caller skips + logs)."""
    subs = [d for d in sorted(os.listdir(kind_dir)) if os.path.isdir(os.path.join(kind_dir, d))]
    for sub in subs:
        if sub == "default":
            continue
        raise ValueError(f"non-default subkind {sub!r}")
    idp_root = os.path.join(kind_dir, "default")
    if not os.path.isdir(idp_root):
        return
    for idp in sorted(os.listdir(idp_root)):
        idp_dir = os.path.join(idp_root, idp)
        if not (os.path.isdir(idp_dir) and _is_iddirpart(idp)):
            continue  # stray source atlas / psd at the subkind level — left in place
        _id, d, part = idp.split(".")
        for variant in sorted(os.listdir(idp_dir)):
            vdir = os.path.join(idp_dir, variant)
            if not os.path.isdir(vdir):
                continue
            for fn in sorted(os.listdir(vdir)):
                src = os.path.join(vdir, fn)
                if not os.path.isfile(src):
                    continue
                stem, _, ext = fn.rpartition(".")
                if ext == "json":                       # per-variant (meta.json): no dir/part
                    dst_name = fn
                else:                                   # directional map: fold dir/part in
                    dst_name = f"{stem}.{d}.{part}.{ext}"
                yield src, os.path.join(kind_dir, variant, dst_name)


def main(argv: list[str]) -> int:
    apply = "--apply" in argv
    roots = [a for a in argv if not a.startswith("--")] or ["textures/biome-thing", "textures/pawn"]
    moves: list[tuple[str, str]] = []
    prune_dirs: list[str] = []
    skipped: list[str] = []
    for root in roots:
        rabs = os.path.join(REPO, root)
        # a kind dir = <type>/<subtype>/<kind>; walk two levels under the type root
        if not os.path.isdir(rabs):
            print(f"skip (absent): {root}", file=sys.stderr); continue
        for subtype in sorted(os.listdir(rabs)):
            st = os.path.join(rabs, subtype)
            if not os.path.isdir(st):
                continue
            for kind in sorted(os.listdir(st)):
                kd = os.path.join(st, kind)
                if not os.path.isdir(kd):
                    continue
                try:
                    km = list(plan(kd))
                except ValueError as e:
                    skipped.append(f"{os.path.relpath(kd, REPO)}  ({e})")
                    continue
                moves.extend(km)
                if km:
                    prune_dirs.append(os.path.join(kd, "default"))  # this kind's subkind dir only

    print(f"=== migrate_leaf: {len(moves)} files, {len(skipped)} kinds skipped ===")
    for s in skipped:
        print(f"  SKIP {s}")
    for src, dst in moves[:12]:
        print(f"  {os.path.relpath(src, REPO)}\n    -> {os.path.relpath(dst, REPO)}")
    if len(moves) > 12:
        print(f"  … +{len(moves) - 12} more")
    if not apply:
        print("\n(dry-run — pass --apply to perform; old default/ dirs removed after copy)")
        return 0

    for src, dst in moves:
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(src, dst)
    for old in prune_dirs:                       # each is exactly <kind>/default, never a subtype
        if os.path.isdir(old):
            shutil.rmtree(old)
    print(f"\napplied: {len(moves)} files copied, {len(prune_dirs)} <kind>/default dirs pruned.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
