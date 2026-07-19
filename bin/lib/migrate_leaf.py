#!/usr/bin/env python3
"""Migrate `<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>` leaves to the new
`<kind>/<variant>/<map>.<dir>.<part>.<ext>` shape (texture-restructure P3).

Handles every current layout: drop `<id>`, fold `<dir>`/`<part>` into the filename,
collapse the direction/part folders into ONE `<variant>/` folder, and resolve `<subkind>`:

  * directional maps `<map>.<ext>`  → `<variant>/<map>.<dir>.<part>.<ext>`
  * per-variant `*.json` (meta.json) → `<variant>/<name>`  (no dir/part)
  * `subkind == "default"` **drops** (kind stays `<kind>`); any other subkind **folds into
    the kind name** `<kind>.<subkind>` — matching `linked/<form>.<material>` — so
    `pawn/human/male/fat/…` → `pawn/human/male.fat/…` (body-type becomes a distinct kind).
  * `linked/` (no subkind) is reshaped in place: `linked/<kind>/<variant>/<map>.l.0.<ext>`.

Note: the full biome-tile **fold** (rename `linked/` → `biome-tile/`, material→kind) is
NOT here — it's object-model-coupled (phase 2). This does the leaf reshape only.

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


def _idp_moves(idp_root: str, target_kind_dir: str) -> list:
    """Reshape every `<id.dir.part>/<variant>/<file>` under `idp_root` into
    `target_kind_dir/<variant>/<map>.<dir>.<part>.<ext>` (per-variant `*.json` kept as-is)."""
    moves = []
    for idp in sorted(os.listdir(idp_root)):
        idp_dir = os.path.join(idp_root, idp)
        if not (os.path.isdir(idp_dir) and _is_iddirpart(idp)):
            continue  # stray source atlas / psd — left in place
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
                dst_name = fn if ext == "json" else f"{stem}.{d}.{part}.{ext}"
                moves.append((src, os.path.join(target_kind_dir, variant, dst_name)))
    return moves


def _has_idp(d: str) -> bool:
    return any(_is_iddirpart(x) and os.path.isdir(os.path.join(d, x)) for x in os.listdir(d))


def plan(kind_dir: str) -> tuple[list, list]:
    """Return (moves, prune_dirs) for one `<kind>` dir, → `<kind>/<variant>/<map>.<dir>.<part>.<ext>`:
      * **no-subkind** (linked): `<kind>/<id.dir.part>/…` reshaped in place.
      * **subkind**: each subkind reshaped — `default` **drops** (kind stays `<kind>`); any other
        subkind **folds into the kind name** `<kind>.<subkind>` (a sibling dir), matching the
        `linked/<form>.<material>` dotted-kind convention. e.g. `pawn/human/male/fat/…` →
        `pawn/human/male.fat/<variant>/…`. Emptied `<kind>/` dirs are cleaned up by the caller.
    Already-reshaped kinds (variant dirs, no id.dir.part) yield no moves (idempotent)."""
    dirs = [d for d in sorted(os.listdir(kind_dir)) if os.path.isdir(os.path.join(kind_dir, d))]
    if any(_is_iddirpart(d) for d in dirs):         # no-subkind (linked): id-dirs are direct children
        return _idp_moves(kind_dir, kind_dir), [os.path.join(kind_dir, d) for d in dirs if _is_iddirpart(d)]
    kind_name = os.path.basename(kind_dir)
    parent = os.path.dirname(kind_dir)
    moves, prune = [], []
    for sub in dirs:
        sub_dir = os.path.join(kind_dir, sub)
        if not _has_idp(sub_dir):
            continue  # not a leaf-holding subkind (e.g. an already-reshaped variant dir)
        target = kind_dir if sub == "default" else os.path.join(parent, f"{kind_name}.{sub}")
        moves += _idp_moves(sub_dir, target)
        prune.append(sub_dir)
    return moves, prune


def main(argv: list[str]) -> int:
    apply = "--apply" in argv
    roots = [a for a in argv if not a.startswith("--")] or ["textures/biome-thing", "textures/pawn"]
    moves: list[tuple[str, str]] = []
    prune_dirs: list[str] = []
    skipped: list[str] = []
    for root in roots:
        rabs = os.path.join(REPO, root)
        if not os.path.isdir(rabs):
            print(f"skip (absent): {root}", file=sys.stderr); continue
        # `linked` kinds sit at depth 1 (linked/<kind>); others at depth 2 (<type>/<subtype>/<kind>).
        if os.path.basename(root) == "linked":
            kind_dirs = [os.path.join(rabs, k) for k in sorted(os.listdir(rabs))
                         if os.path.isdir(os.path.join(rabs, k))]
        else:
            kind_dirs = []
            for subtype in sorted(os.listdir(rabs)):
                st = os.path.join(rabs, subtype)
                if os.path.isdir(st):
                    kind_dirs += [os.path.join(st, k) for k in sorted(os.listdir(st))
                                  if os.path.isdir(os.path.join(st, k))]
        for kd in kind_dirs:
            try:
                km, kp = plan(kd)
            except ValueError as e:
                skipped.append(f"{os.path.relpath(kd, REPO)}  ({e})")
                continue
            moves.extend(km)
            prune_dirs.extend(kp)

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
    for old in prune_dirs:                       # a subkind dir or a linked <id.dir.part> dir
        if os.path.isdir(old):
            shutil.rmtree(old)
    # remove any dir left empty by the fold (e.g. pawn/human/male/ after its subkinds folded to siblings)
    removed = 0
    for root in roots:
        rabs = os.path.join(REPO, root)
        for dp, _dn, _fn in os.walk(rabs, topdown=False):
            if dp != rabs and not os.listdir(dp):
                os.rmdir(dp); removed += 1
    print(f"\napplied: {len(moves)} files copied, {len(prune_dirs)} dirs pruned, {removed} empty dirs removed.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
