#!/usr/bin/env python3
"""bin/art split_layers — decompose a flat albedo into a channel-packed tint map.

A de-lit albedo is a near-flat, tiny-palette image, so its materials separate
cleanly by colour. This strips up to 4 materials into a packed RGBA map (each
channel = that region's stretched brightness) and writes the leftover to a
SEPARATE `.packed_residual.png` (extracted regions zeroed; outline + un-stripped
pixels kept — the "residual" of the PACKING step, distinct from Marigold's
`albedo_residual`). The source `.albedo.png` is left UNTOUCHED, so this is
idempotent and can be re-run with a different --threshold to re-derive both maps.

Renderer:  out = packed_residual + packed.R*tint0 + packed.G*tint1
                                 + packed.B*tint2 + packed.A*tint3   (masked by albedo alpha)

Clustering is by CHROMA: all low-saturation pixels are one "neutral coat" material
(shading preserved as brightness); saturated accents (eyes, markings) split by hue.
The black outline is excluded so it stays dark under any tint.

Idempotent — always reads the original albedo, so re-running just overwrites the
derived `.packed_residual.png` + `.packed.png`. No `art delight` reset needed.
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
    `pawns.animal/wolf` -> textures/pawns.animal/wolf. Returns None if neither exists."""
    p = p.rstrip("/")
    if os.path.exists(p):
        return p
    mp = os.path.join(TEXROOT, p)
    if os.path.exists(mp):
        return mp
    return None

def find_albedos(paths):
    out = []
    for p in paths:
        rp = _resolve(p)
        if rp is None:
            print(f"split_layers: path not found (tried '{p}' and textures/{p})", file=sys.stderr); continue
        if os.path.isdir(rp):
            out += texpath.find_maps(rp, "albedo")
        elif texpath.is_map(rp, "albedo"):
            out.append(rp)
        else:
            print(f"split_layers: not an albedo or directory: {rp}", file=sys.stderr)
    return sorted(set(out))

def _erode(m):
    e = m.copy()
    e[1:, :] &= m[:-1, :]; e[:-1, :] &= m[1:, :]; e[:, 1:] &= m[:, :-1]; e[:, :-1] &= m[:, 1:]
    return e

def _dilate(m):
    d = m.copy()
    d[1:, :] |= m[:-1, :]; d[:-1, :] |= m[1:, :]; d[:, 1:] |= m[:, :-1]; d[:, :-1] |= m[:, 1:]
    return d

def _open(m):
    """Morphological open (erode then dilate): erases scattered speckles / 1px AA
    fringes but keeps compact blobs. Used to SCORE cluster coherence."""
    return _dilate(_erode(m))

def _hsv(rgb):
    mx = rgb.max(2); mn = rgb.min(2); d = mx - mn
    S = d / (mx + 1e-6); V = mx / 255.0
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    H = np.zeros(mx.shape, np.float32); dm = d > 1e-6
    i1 = (mx == r) & dm; H[i1] = ((g[i1]-b[i1]) / d[i1]) % 6
    i2 = (mx == g) & dm; H[i2] = ((b[i2]-r[i2]) / d[i2]) + 2
    i3 = (mx == b) & dm; H[i3] = ((r[i3]-g[i3]) / d[i3]) + 4
    return (H / 6.0) % 1.0, S, V

def split_albedo(im, channels=4, sat=0.20, stretch=0.35, outline_v=0.18, hue_buckets=8, min_region=10):
    arr = np.asarray(im.convert("RGBA")).astype(np.float32); rgb = arr[..., :3]; A = arr[..., 3]
    opaque = A > 128
    lum = (0.299*rgb[...,0] + 0.587*rgb[...,1] + 0.114*rgb[...,2]) / 255.0
    outline = opaque & (lum < outline_v)
    cand = opaque & ~outline
    packed = np.zeros((*lum.shape, 4), np.float32); info = []
    if cand.sum() > 0:
        H, S, _ = _hsv(rgb)
        key = np.full(lum.shape, -999, np.int64)
        neutral = cand & (S < sat); chrom = cand & ~neutral
        key[neutral] = -1
        key[chrom] = (H[chrom] * hue_buckets).astype(np.int64) % hue_buckets
        # Score each colour cluster by its COHERENT size (morphological open erases
        # scattered speckles / AA fringe) so a compact blob outweighs the same pixel
        # count sprinkled as noise. Rank by that, take the top `channels`.
        clusters = []
        for k in np.unique(key[cand]):
            m = cand & (key == k)
            clusters.append((int(k), m, int(_open(m).sum())))
        clusters.sort(key=lambda c: -c[2])
        sel = [c for c in clusters if c[2] >= min_region][:max(1, channels)]
        if not sel:
            sel = clusters[:1]                               # degenerate: keep the single biggest
        # Strip ONLY the selected clusters into packed. Everything else (over-budget
        # colours, speckles) is LEFT in the residual untouched: the renderer tints
        # against it (out = residual + packed.R*tint0 + ...), so an un-stripped colour
        # must survive in the residual — folding it into ch0 would tint it too.
        masks = [m for _, m, _ in sel][:4]
        assigned = np.zeros(lum.shape, bool)
        for i, m in enumerate(masks):
            v = lum[m]; vmin, vmax = float(v.min()), float(v.max())
            packed[m, i] = stretch + (1 - stretch) * (lum[m] - vmin) / (vmax - vmin + 1e-6)
            assigned |= m
            k = sel[i][0]
            info.append(("neutral" if k == -1 else f"hue{int(k)}", int(m.sum()),
                         rgb[m].mean(0).round().astype(int).tolist()))
        rgb[assigned] = 0                                    # subtract stripped regions from albedo
    residual = Image.fromarray(np.dstack([rgb.astype(np.uint8), A.astype(np.uint8)]), "RGBA")
    # R,G,B are always tint layers. The alpha channel is a 4th tint layer ONLY when
    # --channels >= 4; otherwise it's OPAQUE padding (255) so the packed PNG is viewable
    # and survives premultiply/discard on texture upload (alpha=0 would zero the RGB data).
    prgb = (np.clip(packed[..., :3], 0, 1) * 255).astype(np.uint8)
    if channels >= 4:
        pa = (np.clip(packed[..., 3], 0, 1) * 255).astype(np.uint8)   # 4th material (renderer: load raw, no premultiply)
    else:
        pa = np.full(lum.shape, 255, np.uint8)                        # opaque padding
    packed_img = Image.fromarray(np.dstack([prgb, pa]), "RGBA")
    return residual, packed_img, info

def main():
    ap = argparse.ArgumentParser(prog="art split_layers",
        description="Decompose a flat albedo into a packed RGBA tint map + a separate packed_residual; the source albedo is preserved.")
    ap.add_argument("paths", nargs="+", help="*.albedo.png file(s) or directories to scan")
    ap.add_argument("--channels", type=int, default=4, help="max materials to strip (default 4)")
    ap.add_argument("--threshold", "--sat", dest="sat", type=float, default=0.20, help="saturation threshold: below = neutral coat, above = colored accent — tunes the range pulled from the albedo (default 0.20)")
    ap.add_argument("--stretch", type=float, default=0.35, help="brightness floor so tints read bright (default 0.35)")
    ap.add_argument("--outline-v", type=float, default=0.18, help="value below which opaque pixels are outline (kept dark; default 0.18)")
    ap.add_argument("--hue-buckets", type=int, default=8, help="hue buckets for accent clustering (default 8)")
    ap.add_argument("--min-region", type=int, default=10, help="min COHERENT pixels (after speckle-removal) for a colour to earn its own channel; smaller clusters fold into the primary (default 10)")
    ap.add_argument("--force", action="store_true", help="re-split albedos that already have a .packed_residual.png (default: skip them; re-runs are non-destructive since the source albedo is preserved)")
    args = ap.parse_args()

    albs = find_albedos(args.paths)
    if not albs:
        raise SystemExit("split_layers: no albedo.png found under the given paths")
    done = skipped = 0
    for alb in albs:
        residual_path = texpath.sibling(alb, "packed_residual")
        packed_path = texpath.sibling(alb, "packed")
        if os.path.exists(residual_path) and not args.force:
            skipped += 1; continue      # already split — don't redo (--force to re-derive)
        res, packed, info = split_albedo(Image.open(alb), args.channels, args.sat, args.stretch, args.outline_v, args.hue_buckets, args.min_region)
        res.save(residual_path)     # residual -> separate map (albedo left untouched)
        packed.save(packed_path)    # co-located packed map
        done += 1
        print(f"  {os.path.relpath(alb, REPO)}: {len(info)} layer(s) -> {os.path.basename(packed_path)} + {os.path.basename(residual_path)}")
        for i, (kind, n, seed) in enumerate(info):
            print(f"     ch{i}: {kind:8s} px={n} seed={seed}")
    tail = f" ({skipped} already split — use --force to re-split)" if skipped else ""
    print(f"split_layers: done — {done} albedo(s) split{tail}")

if __name__ == "__main__":
    main()
