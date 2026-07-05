#!/usr/bin/env python3
"""bin/art emissive — gate + clean a Marigold albedo_residual into an emissive map.

Marigold-IID splits each sprite as  I = A*S + R.  R (`<base>.albedo_residual.png`)
is the non-diffuse term: self-lit glow, painted rim lights and speculars — the
energy the albedo A drops. This turns R into a shippable additive emissive map:

  GATE  — keep only pixels that are genuinely bright (luminance >= --threshold);
          everything dimmer goes to black so matte assets don't faintly glow.
  CLEAN — erode the coverage mask by --erode px to drop the mild 1px silhouette
          colour-fringing Marigold leaves in R.

Writes `<base>.emissive.png` (RGB = gated glow, source alpha kept for coverage).
The source `albedo_residual` is PRESERVED, so this is idempotent and re-tunable —
resweep --threshold/--erode without re-running the (GPU) de-light. Additive at
render, after the lighting pass:  out = albedo * lighting + emissive.

Matte stone/bark → near-black emissive (nothing glows). Glowing eyes / lava /
runes survive as their own self-lit colour.
"""
import argparse, os, sys
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
TEXROOT = os.path.join(REPO, "textures")   # source root; the <group> dir is gone

def _resolve(p):
    """Accept an absolute/cwd path, OR a textures-relative one like remaster:
    `pawns.animal/wolf` -> textures/pawns.animal/wolf. None if neither exists."""
    p = p.rstrip("/")
    if os.path.exists(p):
        return p
    mp = os.path.join(TEXROOT, p)
    if os.path.exists(mp):
        return mp
    return None

def find_residuals(paths):
    out = []
    for p in paths:
        rp = _resolve(p)
        if rp is None:
            print(f"emissive: path not found (tried '{p}' and textures/{p})", file=sys.stderr); continue
        if os.path.isdir(rp):
            out += texpath.find_maps(rp, "albedo_residual")
        elif texpath.is_map(rp, "albedo_residual"):
            out.append(rp)
        else:
            print(f"emissive: not an albedo_residual or directory: {rp}", file=sys.stderr)
    return sorted(set(out))

def _erode(m):
    e = m.copy()
    e[1:, :] &= m[:-1, :]; e[:-1, :] &= m[1:, :]; e[:, 1:] &= m[:, :-1]; e[:, :-1] &= m[:, 1:]
    return e

def gate_residual(im, threshold=0.10, erode=1):
    """R -> emissive. Returns (emissive RGBA image, lit-pixel count)."""
    arr = np.asarray(im.convert("RGBA")).astype(np.float32); rgb = arr[..., :3]; A = arr[..., 3]
    opaque = A > 128
    lum = (0.299*rgb[..., 0] + 0.587*rgb[..., 1] + 0.114*rgb[..., 2]) / 255.0
    # CLEAN: shrink coverage to drop the 1px edge fringe Marigold leaves in R.
    cov = opaque.copy()
    for _ in range(max(0, erode)):
        cov = _erode(cov)
    # GATE: keep only bright interior pixels; the rest go black (additive → no glow).
    keep = cov & (lum >= threshold)
    rgb[~keep] = 0
    out = np.dstack([rgb.astype(np.uint8), A.astype(np.uint8)])
    return Image.fromarray(out, "RGBA"), int(keep.sum())

def main():
    ap = argparse.ArgumentParser(prog="art emissive",
        description="Gate + clean a Marigold albedo_residual into a shippable emissive map; the source is preserved.")
    ap.add_argument("paths", nargs="+", help="*.albedo_residual.png file(s) or directories to scan")
    ap.add_argument("--threshold", type=float, default=0.10, help="luminance floor (0-1): pixels dimmer than this go black (default 0.10)")
    ap.add_argument("--erode", type=int, default=1, help="px to shrink coverage by, killing the silhouette-edge fringe (default 1)")
    ap.add_argument("--force", action="store_true", help="re-gate residuals that already have a .emissive.png (default: skip them)")
    args = ap.parse_args()

    res = find_residuals(args.paths)
    if not res:
        raise SystemExit("emissive: no albedo_residual.png found under the given paths")
    done = skipped = 0
    for r in res:
        emissive_path = texpath.sibling(r, "emissive")
        if os.path.exists(emissive_path) and not args.force:
            skipped += 1; continue
        img, lit = gate_residual(Image.open(r), args.threshold, args.erode)
        img.save(emissive_path)
        done += 1
        print(f"  {os.path.relpath(r, REPO)}: {lit} lit px -> {os.path.basename(emissive_path)}"
              + ("  (matte — near-black)" if lit == 0 else ""))
    tail = f" ({skipped} already gated — use --force to re-gate)" if skipped else ""
    print(f"emissive: done — {done} residual(s) gated{tail}")

if __name__ == "__main__":
    main()
