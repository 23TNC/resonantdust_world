#!/usr/bin/env python3
"""Phase 2 of the folder-per-variant migration (docs/texture-paths.md).

Restructures the legacy `textures/{master,sprites,templates}/…` flat-file trees into
the group-less folder-per-variant layout `textures/<cat>/<kind>/<id>.<dir>.<layer>/<variant>/<map>.<ext>`.

NON-DESTRUCTIVE: it COPIES into the new top-level `<cat>/…` dirs and leaves the old
`master/`/`sprites/`/`templates/` trees untouched, so both layouts coexist on disk and
the migration stays rollback-safe until the old trees are explicitly removed (a separate,
later step). Dry-run by default — prints the full classification + rename table and writes
NOTHING; pass --apply to perform the copies.

Classification (see docs/texture-paths.md):
  master/*      → every file is a per-variant map → LEAF
  sprites/*     → numeric-id `sprite` → LEAF; sheets (non-numeric id), source atlases
                  (`diffuse`), `.psd`, `.alias` → LOOSE (flat, group dropped)
  templates/*   → `.template.{png,psd}` → LEAF; `.prompt.txt` → LOOSE

LEAF path: the trailing dot-field of the stem is the <variant> (mechanical split — lossless
even for the malformed `1.l.0.l.0.N` wall.smooth stems), the rest is the <pose> dir.
"""
import argparse
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath

REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
TEX = os.path.join(REPO, "textures")
GROUPS = ("master", "sprites", "templates")


class Item:
    __slots__ = ("src", "rel", "action", "dest", "note")

    def __init__(self, src, rel):
        self.src = src          # absolute source path
        self.rel = rel          # path under textures/ (e.g. master/world/conifer/1.s.0.1.albedo.png)
        self.action = None      # "leaf" | "loose" | "anomaly"
        self.dest = None        # path under textures/ (new), or None for anomaly
        self.note = ""


def split_stem_variant(stem):
    """(pose, variant) from a stem, splitting the trailing dot-field as the variant.
    Returns (None, None) if the stem has no variant field to split (single field)."""
    if "." not in stem:
        return None, None
    pose, variant = stem.rsplit(".", 1)
    return pose, variant


def classify(it):
    group, rest = it.rel.split("/", 1)
    dirs = os.path.dirname(rest)          # <cat>/<kind> path under the group
    fname = os.path.basename(rest)
    name, ext = os.path.splitext(fname)   # ext includes the leading '.'
    ext = ext.lstrip(".")

    # --- alias sidecars: always loose ---
    if ext == "alias":
        it.action, it.dest, it.note = "loose", os.path.join(dirs, fname), "alias sidecar"
        return

    # --- sprites-group authoring PSDs: arbitrary names (Rock_Atlas.psd) that don't
    # parse as <stem>.<map>; always loose. (Template PSDs are `.template.psd` and
    # parse normally into a leaf below.) ---
    if group == "sprites" and ext == "psd":
        it.action, it.dest, it.note = "loose", os.path.join(dirs, fname), "authoring PSD"
        return

    parts = name.split(".")
    maptok = parts[-1]
    stem_parts = parts[:-1]
    # The map token must be non-numeric (variants are numeric). A numeric trailing
    # field means the file carries no map token — an anomaly we don't guess at.
    if maptok.isdigit() or not stem_parts:
        it.action, it.note = "anomaly", f"no map token (trailing field '{maptok}')"
        return
    mp = maptok
    stem = ".".join(stem_parts)
    idtok = stem_parts[0]

    def leaf_dest():
        pose, variant = split_stem_variant(stem)
        if pose is None:
            return None
        return os.path.join(dirs, pose, variant, texpath.map_name(mp, ext))

    if group == "master":
        d = leaf_dest()
        if d is None:
            it.action, it.note = "anomaly", f"master stem '{stem}' has no variant field"
            return
        it.action, it.dest = "leaf", d
        if len(stem_parts) != 4:
            it.note = f"irregular stem ({len(stem_parts)} fields, expected id.dir.layer.variant)"
        return

    if group == "templates":
        if mp == "prompt":
            it.action, it.dest, it.note = "loose", os.path.join(dirs, fname), "generation prompt (seed-keyed sidecar)"
            return
        if mp == "template":
            d = leaf_dest()
            if d is None:
                it.action, it.note = "anomaly", f"template stem '{stem}' has no variant field"
                return
            it.action, it.dest, it.note = "leaf", d, "pose template"
            return
        it.action, it.note = "anomaly", f"unexpected template map '{mp}'"
        return

    if group == "sprites":
        if not idtok.isdigit():
            it.action, it.dest, it.note = "loose", os.path.join(dirs, fname), "sprite sheet (named id)"
            return
        if mp == "sprite":
            d = leaf_dest()
            if d is None:
                it.action, it.note = "anomaly", f"sprite stem '{stem}' has no variant field"
                return
            it.action, it.dest, it.note = "leaf", d, "per-variant sprite"
            return
        if mp == "diffuse":
            it.action, it.dest, it.note = "loose", os.path.join(dirs, fname), "source atlas (grid input)"
            return
        it.action, it.note = "anomaly", f"unexpected sprite map '{mp}'"
        return

    it.action, it.note = "anomaly", f"unknown group '{group}'"


def main():
    ap = argparse.ArgumentParser(description="Phase 2: migrate textures/ to the folder-per-variant layout (docs/texture-paths.md).")
    ap.add_argument("--apply", action="store_true", help="perform the copies (default: dry-run, writes nothing)")
    ap.add_argument("--table", default=os.path.join(REPO, "scratch-migrate-table.tsv"),
                    help="write the full src<TAB>action<TAB>dest table here (dry-run)")
    args = ap.parse_args()

    items = []
    for group in GROUPS:
        root = os.path.join(TEX, group)
        if not os.path.isdir(root):
            continue
        for dp, _dirs, files in os.walk(root):
            for f in files:
                src = os.path.join(dp, f)
                rel = os.path.relpath(src, TEX)
                it = Item(src, rel)
                classify(it)
                items.append(it)

    leaves = [i for i in items if i.action == "leaf"]
    loose = [i for i in items if i.action == "loose"]
    anomalies = [i for i in items if i.action == "anomaly"]
    irregular = [i for i in leaves if i.note.startswith("irregular")]

    # Destination collisions: two distinct sources mapping to the same new path.
    dest_map = {}
    collisions = []
    for i in leaves + loose:
        if i.dest in dest_map:
            collisions.append((dest_map[i.dest], i))
        else:
            dest_map[i.dest] = i

    def show(items_, n=8):
        for i in items_[:n]:
            print(f"    {i.rel}")
            print(f"      -> {i.dest}   [{i.note}]" if i.dest else f"      -> (skipped)   [{i.note}]")
        if len(items_) > n:
            print(f"    … +{len(items_) - n} more")

    print(f"Scanned {len(items)} files under textures/{{{','.join(GROUPS)}}}")
    print(f"  LEAF   : {len(leaves)}  ({len(irregular)} irregular-stem)")
    print(f"  LOOSE  : {len(loose)}")
    print(f"  ANOMALY: {len(anomalies)}  (skipped)")
    print(f"  dest collisions: {len(collisions)}")
    print()
    print("── LEAF (sample) ──"); show(leaves)
    print("── LOOSE (sample) ──"); show(loose)
    if irregular:
        print("── IRREGULAR STEMS (migrated losslessly, but pre-existing non-canonical — consider re-mastering) ──")
        show(irregular, n=len(irregular))
    if anomalies:
        print("── ANOMALIES (skipped — review) ──")
        show(anomalies, n=len(anomalies))
    if collisions:
        print("── DEST COLLISIONS (ERROR — two sources → one dest) ──")
        for a, b in collisions:
            print(f"    {a.rel}\n    {b.rel}\n      -> {b.dest}")

    # Always write the full table for review.
    with open(args.table, "w") as fh:
        for i in items:
            fh.write(f"{i.rel}\t{i.action}\t{i.dest or ''}\t{i.note}\n")
    print(f"\nFull table: {os.path.relpath(args.table, REPO)}")

    if not args.apply:
        print("\nDRY-RUN — nothing written. Re-run with --apply to copy (old trees are left intact).")
        return 0

    if collisions:
        print("\nREFUSING to apply: resolve the dest collisions above first.", file=sys.stderr)
        return 1

    copied = 0
    for i in leaves + loose:
        dst = os.path.join(TEX, i.dest)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(i.src, dst)
        copied += 1
    print(f"\nAPPLIED — copied {copied} file(s) into the new layout. Old trees left intact for rollback.")
    print(f"Skipped {len(anomalies)} anomalies (see above).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
