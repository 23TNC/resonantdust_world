#!/usr/bin/env python3
"""bin/art outline — the per-leaf shadow-cast SILHOUETTE into meta.json.

Runs the `outline` Rust CLI (shared/geometry) on each variant leaf's **diffuse.png**
(its alpha is the sprite silhouette — things are RGBA with a real cutout; opaque tiles
are `L`, so they yield a harmless full square and don't cast anyway), and merges the
resulting `Sidecar` — boundary polygons + earcut triangulation, normalized to the content
box — into that leaf's `meta.json` under `outline`. The tiered-lighting scatter projects
these through each light (docs/work/art-metadata). Skip-if-present unless `--force`.
"""
import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
import meta

REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
OUTLINE_BIN = os.path.join(REPO, "shared", "target", "release", "outline")


def _resolve(p):
    for cand in (p, os.path.join(REPO, "textures", p), os.path.join(REPO, p)):
        if os.path.exists(cand):
            return cand
    return None


def find_diffuses(paths):
    out = []
    for p in paths:
        rp = _resolve(p)
        if rp is None:
            print(f"outline: path not found (tried '{p}' and textures/{p})", file=sys.stderr)
            continue
        if os.path.isdir(rp):
            out += texpath.find_maps(rp, "diffuse")
        elif texpath.is_map(rp, "diffuse"):
            out.append(rp)
    return sorted(set(out))


def main():
    ap = argparse.ArgumentParser(prog="art outline",
        description="Generate the shadow-cast silhouette (boundary polygons + earcut triangulation) into each leaf's meta.json.")
    ap.add_argument("paths", nargs="+", help="diffuse.png file(s) or directories to scan")
    ap.add_argument("--force", action="store_true", help="regenerate even if meta.json already has an outline")
    args = ap.parse_args()

    if not os.path.exists(OUTLINE_BIN):
        raise SystemExit(
            f"outline: missing the CLI at {OUTLINE_BIN}\n"
            f"  build it: docker run --rm -v \"$PWD\":/workspace -v rd-sim-cargo:/usr/local/cargo/registry "
            f"-w /workspace/shared/geometry rd-sim-builder cargo build --release --features generate --bin outline")

    diffs = find_diffuses(args.paths)
    if not diffs:
        raise SystemExit("outline: no diffuse.png found under the given paths")
    done = skipped = failed = 0
    for d in diffs:
        if "outline" in meta.load(d) and not args.force:
            skipped += 1
            continue
        res = subprocess.run([OUTLINE_BIN, d], capture_output=True, text=True)
        if res.returncode != 0:
            print(f"  {os.path.relpath(d, REPO)}: FAILED — {res.stderr.strip()}", file=sys.stderr)
            failed += 1
            continue
        sidecar = json.loads(res.stdout)
        meta.update(d, outline=sidecar)
        polys = sidecar.get("polygons", [])
        tris = sum(len(p.get("triangles", [])) // 3 for p in polys)
        done += 1
        print(f"  {os.path.relpath(d, REPO)}: {len(polys)} polygon(s), {tris} triangle(s) -> meta.json")
    tail = f" ({skipped} already have an outline — use --force)" if skipped else ""
    if failed:
        tail += f" ({failed} failed)"
    print(f"outline: done — {done} outline(s){tail}")


if __name__ == "__main__":
    main()
