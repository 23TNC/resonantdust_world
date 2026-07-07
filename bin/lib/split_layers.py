#!/usr/bin/env python3
"""bin/art split_layers — decompose a flat albedo into a channel-packed tint map.

A de-lit albedo is a near-flat, tiny-palette image, so its materials separate
cleanly by colour. This strips up to 3 materials into a packed RGB map `layers.png`
(each channel = that region's stretched brightness). The leftover (the residual)
BECOMES `albedo.png` — the render base the client reconstructs from — and the de-lit
source is preserved as `albedo_marigold.png` (the re-split / reconstruction source).

Renderer:  out.rgb = albedo(residual) + layers.R*tint0 + layers.G*tint1 + layers.B*tint2
           out.a   = surface.B   (the visual alpha, applied AFTER the sum)
Both `albedo` and `layers` are RGB (no alpha): the client loads them STRAIGHT (no
premultiply) so the sum stays exact, and the silhouette/transparency lives in the
surface map's B channel — see pixijs/materialBakeShader.ts. Packing is LINEAR
UNMIXING: layers.<ch> holds each pixel's coefficient on a material (a pixel can be
nonzero in two channels — a gradient blend), and `residual = albedo - sum(coeff *
material_base_colour)`. So under the identity tint (tint = base colour) the renderer
reconstructs the de-lit albedo exactly; a new tint shifts only the material's share
and the gradient blends smoothly across channels.

Segmentation: DIVIDE into regions, cluster region MEANS, then spatial cleanup:
  1. divide into spatially-coherent sections — sharp colour edges (--edge) + chroma
     cells (--threshold) cut the boundaries. A region's mean colour is stable even where
     the per-pixel de-lit chroma is noisy (global per-pixel clustering shatters on it).
  2. cluster the region MEANS into materials in chroma space, merged by --group-threshold
     (brightness-invariant, so lit/shaded patches of one material group; 3 white patches
     -> 1 white channel). These are the channels.
  3. absorb small connected specks into the LARGE region they physically border (spatial,
     not by colour): a near-white speck in the green mane becomes green. A speck below
     --min-region with no large neighbour drops to the residual.
  4. UNMIX the top --channels materials by area: each pixel gets non-negative coefficients
     on its 2 nearest materials (overlap => smooth gradients), and the leftover goes to the
     residual. The black outline is excluded so it stays dark under any tint.

Re-run safe: a standalone re-run SKIPS an already-split leaf (albedo_marigold.png
present) so it can't re-split the residual; --force re-derives from the fresh de-lit
in albedo.png (the remaster path, where `maps` just regenerated it).
"""
import argparse, os, shutil, sys
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

_NBRS = [(0, 1), (0, -1), (1, 1), (1, -1)]

def _shift(a, ax, sh, fill):
    """Roll `a` by one along an axis WITHOUT wrap: the vacated edge is set to `fill`."""
    r = np.roll(a, sh, axis=ax)
    if ax == 0:
        r[0 if sh == 1 else -1, :] = fill
    else:
        r[:, 0 if sh == 1 else -1] = fill
    return r

def _cc_label(mask, cell=None):
    """4-connected connected-component labels via min-index propagation (pure numpy).
    Each component gets the smallest flat-index among its pixels; -1 outside the mask.
    If `cell` is given, two neighbours connect only when they share a cell value, so a
    single flood is split by colour cell as well as by the mask."""
    H, W = mask.shape
    lab = np.where(mask, np.arange(H * W).reshape(H, W), -1)
    for _ in range(H + W):                                   # cap = worst-case diameter
        cur = lab
        for ax, sh in _NBRS:
            nb = _shift(lab, ax, sh, -1)
            ok = mask & (nb >= 0)
            if cell is not None:
                ok = ok & (_shift(cell, ax, sh, np.int64(-1 << 60)) == cell)
            cur = np.where(ok & ((cur < 0) | (nb < cur)), nb, cur)
        if np.array_equal(cur, lab):
            break
        lab = cur
    return lab

def _chroma_xy(col):
    """RGB(0-255) -> a point on the chroma plane: neutral ~origin, hue by angle,
    radius = saturation. Brightness-invariant, so a lit and a shaded patch of the
    same material land together (unlike raw-RGB distance)."""
    r, g, b = np.asarray(col, np.float32) / 255.0
    mx = max(r, g, b); mn = min(r, g, b); d = mx - mn
    if d < 1e-6:
        return np.array([0.0, 0.0], np.float32)
    s = d / (mx + 1e-6)
    if mx == r:   h = ((g - b) / d) % 6
    elif mx == g: h = (b - r) / d + 2
    else:         h = (r - g) / d + 4
    h = (h / 6.0) % 1.0
    return np.array([s * np.cos(2 * np.pi * h), s * np.sin(2 * np.pi * h)], np.float32)

def _chroma_ab_map(rgb):
    """Per-pixel chroma-plane coords (a, b) — the vectorised form of _chroma_xy.
    Brightness drops out; only hue-angle * saturation-radius remain, so quantising
    (a, b) buckets pixels by colour regardless of how light/dark the shading is."""
    r, g, b = rgb[..., 0] / 255.0, rgb[..., 1] / 255.0, rgb[..., 2] / 255.0
    mx = np.maximum(np.maximum(r, g), b); mn = np.minimum(np.minimum(r, g), b); d = mx - mn
    s = d / (mx + 1e-6)
    hh = np.zeros(mx.shape, np.float32); dm = d > 1e-6
    ir = (mx == r) & dm; hh[ir] = ((g[ir] - b[ir]) / d[ir]) % 6
    ig = (mx == g) & dm; hh[ig] = ((b[ig] - r[ig]) / d[ig]) + 2
    ib = (mx == b) & dm; hh[ib] = ((r[ib] - g[ib]) / d[ib]) + 4
    hh = (hh / 6.0) % 1.0
    return s * np.cos(2 * np.pi * hh), s * np.sin(2 * np.pi * hh)

def _cluster_regions(grown, a, b, group_tol):
    """Cluster REGION MEAN colours into materials. A region's mean chroma is stable
    where per-pixel chroma is noisy, so this avoids the micro-cluster shatter of global
    per-pixel clustering. Agglomerative (largest region first) + a pairwise post-merge so
    no two materials are closer than group_tol. Returns per-pixel material label + centroids."""
    H, W = grown.shape
    flat = grown.ravel(); mask = flat >= 0
    if not mask.any():
        return np.full((H, W), -1, np.int64), np.zeros((0, 2), np.float32)
    uid, inv = np.unique(flat[mask], return_inverse=True)
    area = np.bincount(inv).astype(np.float64)
    rma = np.bincount(inv, weights=a.ravel()[mask]) / area   # per-region mean chroma
    rmb = np.bincount(inv, weights=b.ravel()[mask]) / area
    cents = []; mat = np.full(len(uid), -1, np.int64)
    for i in np.argsort(-area):                              # largest region seeds first
        best, bd = -1, 1e9
        for k, c in enumerate(cents):
            d = np.hypot(rma[i] - c[0], rmb[i] - c[1])
            if d < bd:
                bd, best = d, k
        if best >= 0 and bd <= group_tol:
            c = cents[best]; w = c[2]
            c[0] = (c[0] * w + rma[i] * area[i]) / (w + area[i])
            c[1] = (c[1] * w + rmb[i] * area[i]) / (w + area[i]); c[2] = w + area[i]; mat[i] = best
        else:
            cents.append([rma[i], rmb[i], area[i]]); mat[i] = len(cents) - 1
    changed = True                                           # pairwise post-merge to convergence
    while changed and len(cents) > 1:
        changed = False
        for i in range(len(cents)):
            for jj in range(i + 1, len(cents)):
                if np.hypot(cents[i][0] - cents[jj][0], cents[i][1] - cents[jj][1]) <= group_tol:
                    wi, wj = cents[i][2], cents[jj][2]
                    cents[i] = [(cents[i][0] * wi + cents[jj][0] * wj) / (wi + wj),
                                (cents[i][1] * wi + cents[jj][1] * wj) / (wi + wj), wi + wj]
                    mat[mat == jj] = i; mat[mat > jj] -= 1; del cents[jj]; changed = True; break
            if changed:
                break
    lbl = np.full(H * W, -1, np.int64); lbl[np.where(mask)[0]] = mat[inv]
    return lbl.reshape(H, W), np.array([[c[0], c[1]] for c in cents], np.float32)

def _absorb_specs(lbl, cand, min_region):
    """Dissolve small connected components into the LARGE region they border (spatial,
    not by colour): a near-white speck inside the green mane becomes green. A speck with
    no large neighbour is left unassigned (-1) -> it drops to the residual."""
    H, W = lbl.shape
    comp = _cc_label(cand & (lbl >= 0), cell=lbl)            # components split per material
    _, inv, cnt = np.unique(comp, return_inverse=True, return_counts=True)
    size = cnt[inv].reshape(H, W)
    small = cand & (lbl >= 0) & (comp >= 0) & (size < min_region)
    out = np.where(small, np.int64(-1), lbl)                 # clear specks, keep large regions
    for _ in range(H + W):                                   # grow large labels into the speck holes
        holes = cand & small & (out < 0)
        if not holes.any():
            break
        cur = out
        for ax, sh in _NBRS:
            nb = _shift(out, ax, sh, -1)
            cur = np.where((cur < 0) & cand & small & (nb >= 0), nb, cur)
        if np.array_equal(cur, out):
            break                                            # remaining specks are isolated -> residual
        out = cur
    return out

def _unmix(pa, pb, Cpix, Cc, Bcol):
    """2-nearest-material unmix driven by CHROMA MEMBERSHIP. For each pixel, t is how far
    its chroma sits from its nearest material toward its 2nd-nearest (0..1); the split is
    (1-t):t and the magnitude is the pixel's brightness projected onto the interpolated
    base colour. So a pixel squarely on one material gets t=0 (no bleed into the other),
    while a genuine in-between pixel blends smoothly and continuously. Returns N x K."""
    N, K = Cpix.shape[0], len(Bcol)
    ps = np.zeros((N, K))
    if K == 1:
        B0 = Bcol[0]; ps[:, 0] = np.clip((Cpix @ B0) / (B0 @ B0 + 1e-9), 0, 1); return ps
    d = (pa[:, None] - Cc[None, :, 0]) ** 2 + (pb[:, None] - Cc[None, :, 1]) ** 2
    near = np.argsort(d, axis=1)[:, :2]; m1, m2 = near[:, 0], near[:, 1]
    P = np.stack([pa, pb], 1); C1, C2 = Cc[m1], Cc[m2]
    seg = C2 - C1; ss = (seg * seg).sum(1)
    t = np.clip(np.where(ss > 1e-9, ((P - C1) * seg).sum(1) / (ss + 1e-9), 0.0), 0.0, 1.0)
    Binterp = (1 - t)[:, None] * Bcol[m1] + t[:, None] * Bcol[m2]
    mag = np.clip((Cpix * Binterp).sum(1) / ((Binterp * Binterp).sum(1) + 1e-9), 0, 1)
    np.add.at(ps, (np.arange(N), m1), (1 - t) * mag)
    np.add.at(ps, (np.arange(N), m2), t * mag)
    return ps

def _outline_mask(opaque, lum, core_v, rim_v):
    """Outline = the dark core (lum < core_v = --outline-v) grown by HYSTERESIS into
    adjacent still-dark rim pixels (lum < rim_v = --outline-rim). This swallows the
    anti-aliased halo around the black line — which is dark, near-neutral, and otherwise
    clusters as a bogus 'dark material' — WITHOUT eating a material's interior shadow
    (that's dark but not connected to the line through dark pixels)."""
    core = opaque & (lum < core_v); weak = opaque & (lum < rim_v)
    out = core.copy()
    for _ in range(lum.shape[0] + lum.shape[1]):
        cur = out.copy()
        for ax, sh in _NBRS:
            cur |= weak & _shift(out, ax, sh, False)
        if np.array_equal(cur, out):
            break
        out = cur
    return out

def split_albedo(im, channels=4, section_tol=0.15, group_tol=0.10, edge_thr=42.0, outline_v=0.18, outline_rim=0.35, min_region=25, blend_radius=10):
    arr = np.asarray(im.convert("RGBA")).astype(np.float32); rgb = arr[..., :3]; A = arr[..., 3]
    opaque = A > 128
    lum = (0.299*rgb[...,0] + 0.587*rgb[...,1] + 0.114*rgb[...,2]) / 255.0
    outline = _outline_mask(opaque, lum, outline_v, outline_rim)   # dark line + its AA halo: excluded
    cand = opaque & ~outline
    H, W = lum.shape
    packed = np.zeros((H, W, 4), np.float32); info = []
    if cand.sum() > 0:
        a, b = _chroma_ab_map(rgb)
        # 1. DIVIDE into spatially-coherent, chroma-denoised sections: sharp colour
        #    edges (--edge) + chroma cells (--threshold) cut the boundaries; a region's
        #    mean colour is then stable even where the per-pixel de-lit chroma is noisy.
        grad = np.zeros((H, W), np.float32)
        for ax, sh in _NBRS:
            grad = np.maximum(grad, np.abs(rgb - _shift(rgb, ax, sh, 0.0)).sum(2))
        interior = cand & (grad <= edge_thr)
        cell = (np.round(a / section_tol).astype(np.int64) * 100003
                + np.round(b / section_tol).astype(np.int64))
        grown = _cc_label(interior, cell)
        for _ in range(16):                                  # grow labels over the thin edge bands
            if not (cand & (grown < 0)).any():
                break
            cur = grown
            for ax, sh in _NBRS:
                nb = _shift(grown, ax, sh, -1)
                cur = np.where(cand & (cur < 0) & (nb >= 0), nb, cur)
            grown = cur
        # 2. Cluster REGION MEAN colours into materials (--group-threshold).
        lbl, cents = _cluster_regions(grown, a, b, group_tol)
        # 3. Absorb small specks into the large region they physically border (spatial,
        #    not by colour); a speck with no large neighbour drops to the residual.
        lbl = _absorb_specs(lbl, cand, min_region)
        # 4. Rank materials by area; keep the top --channels that clear --min-region.
        labs, areas = np.unique(lbl[lbl >= 0], return_counts=True)
        kept = [int(l) for l, ar in sorted(zip(labs.tolist(), areas.tolist()), key=lambda x: -x[1])
                if ar >= min_region][:max(1, channels)]
        K = len(kept)
        # Each material's base colour Bi (brightest representative, so coefficients stay
        # <= 1) + its chroma centroid (for the nearest-2 pick).
        Bcol = np.zeros((K, 3), np.float64); Cc = np.zeros((K, 2), np.float64)
        for i, l in enumerate(kept):
            m = (lbl == l)
            hi = m & (lum >= np.percentile(lum[m], 98))
            src = hi if hi.any() else m
            Bcol[i] = rgb[src].mean(0) / 255.0
            Cc[i] = [a[m].mean(), b[m].mean()]
        # 5. UNMIX only pixels whose material made the top-N cut into non-negative
        #    coefficients on their 2 nearest KEPT materials (channel overlap smooths
        #    gradients). Pixels of a DROPPED material are NOT forced into a kept channel —
        #    they stay fully in the residual (baked). residual = albedo - sum c*Bi, so the
        #    identity tint (tint=Bi) reconstructs the albedo exactly; outline + un-kept
        #    colour ride along in the residual.
        idx = np.where(np.isin(lbl, kept))
        ps = _unmix(a[idx], b[idx], rgb[idx] / 255.0, Cc, Bcol)
        # Confine each material's coefficient to its region grown by --blend-radius (the
        # gradient blend zone). Without this the unmix sprinkles faint 2nd-material specks
        # onto noisy pixels far from that material (green flecks on the white head); the
        # removed share falls back into the residual.
        for i, l in enumerate(kept):
            zone = (lbl == l)
            for _ in range(blend_radius):
                z = zone.copy()
                for ax, sh in _NBRS:
                    z |= _shift(zone, ax, sh, False)
                zone = z
            ps[~zone[idx], i] = 0.0
        for i in range(min(K, 4)):
            ch = np.zeros((H, W), np.float32); ch[idx] = np.clip(ps[:, i], 0, 1); packed[..., i] = ch
        recon = (ps[:, :, None] * Bcol[None, :, :]).sum(1)
        resid = rgb / 255.0
        resid[idx] = np.clip(rgb[idx] / 255.0 - recon, 0.0, 1.0)
        rgb = resid * 255.0                                  # residual -> albedo.png (base subtracted)
        for i in range(min(K, 4)):
            rad = float(np.hypot(Cc[i, 0], Cc[i, 1])); ang = int(np.degrees(np.arctan2(Cc[i, 1], Cc[i, 0]))) % 360
            info.append(("neutral" if rad < 0.04 else f"hue{ang}", int((ps[:, i] > 0.05).sum()),
                         (Bcol[i] * 255).round().astype(int).tolist()))
    # Both outputs are RGB — no alpha. The residual's alpha (the diffuse silhouette) now lives
    # in the surface map's B channel, and a 4th (alpha) material layer fought the client's
    # upload-premultiply + on-demand downscale path. The client loads BOTH straight (no
    # premultiply) so the reconstruction sum `residual + Σ layersᵢ·tint` stays exact.
    residual = Image.fromarray(rgb.astype(np.uint8), "RGB")
    prgb = (np.clip(packed[..., :3], 0, 1) * 255).astype(np.uint8)   # 3 tint layers in RGB
    layers_img = Image.fromarray(prgb, "RGB")
    return residual, layers_img, info

def main():
    ap = argparse.ArgumentParser(prog="art split_layers",
        description="Decompose a flat albedo into layers.png (RGB weights); the residual becomes albedo.png, the de-lit preserved as albedo_marigold.png.")
    ap.add_argument("paths", nargs="+", help="*.albedo.png file(s) or directories to scan")
    ap.add_argument("--channels", type=int, default=3, help="max materials (channels) to strip (RGB, max 3)")
    ap.add_argument("--threshold", dest="section_tol", type=float, default=0.15, help="DIVIDE: chroma cell size for sectioning — smaller cuts finer sections (default 0.15)")
    ap.add_argument("--group-threshold", dest="group_tol", type=float, default=0.10, help="GROUP: how close two region colours must be to count as ONE material/channel — higher merges more colours together (default 0.10)")
    ap.add_argument("--edge", dest="edge_thr", type=float, default=42.0, help="DIVIDE: colour-gradient edge cutoff (sum-of-abs RGB diff to a neighbour); lower = more/smaller sections (default 42)")
    ap.add_argument("--min-region", type=int, default=25, help="min pixels for a colour region; smaller connected specks are absorbed into the large region they border, else dropped to the residual if isolated (default 25)")
    ap.add_argument("--blend-radius", type=int, default=10, help="how far (px) a material's tint may reach past its own region — the gradient blend zone; keeps faint coefficients from sprinkling specks onto far pixels (default 10; 0 = hard region edges)")
    ap.add_argument("--outline-v", type=float, default=0.18, help="value below which an opaque pixel is the outline CORE (kept dark; default 0.18)")
    ap.add_argument("--outline-rim", type=float, default=0.35, help="hysteresis rim: dark pixels up to this value that TOUCH the core are also outline — swallows the black line's AA halo (default 0.35; lower for dark-furred creatures)")
    ap.add_argument("--force", action="store_true", help="re-split leaves that already have albedo_marigold.png, re-deriving from the fresh de-lit in albedo.png (default: skip them)")
    args = ap.parse_args()

    albs = find_albedos(args.paths)
    if not albs:
        raise SystemExit("split_layers: no albedo.png found under the given paths")
    done = skipped = 0
    for alb in albs:
        # The residual IS the render base, so it takes the `albedo.png` name; the de-lit source
        # is preserved once as `albedo_marigold.png` (the reconstruction input, and what a
        # re-split reads from). `layers.png` holds the weight channels. Already-split leaves
        # (marigold present) skip unless --force, which re-derives from the preserved de-lit.
        marigold_path = texpath.sibling(alb, "albedo_marigold")
        layers_path = texpath.sibling(alb, "layers")
        # `albedo.png` holds the DE-LIT until we split it, then the RESIDUAL — so a standalone
        # re-run must SKIP an already-split leaf (marigold present), or it would re-split the
        # residual. `--force` is for the remaster path, where `cmd_maps` has just written a FRESH
        # de-lit to albedo.png, so we always read (and re-preserve) THAT, never the stale marigold.
        if os.path.exists(marigold_path) and not args.force:
            skipped += 1; continue
        res, layers, info = split_albedo(Image.open(alb).convert("RGBA"), min(args.channels, 3), args.section_tol, args.group_tol, args.edge_thr, args.outline_v, args.outline_rim, args.min_region, args.blend_radius)
        shutil.copy2(alb, marigold_path)   # preserve this de-lit (the reconstruction/re-split source)
        res.save(alb)               # residual -> albedo.png (the render base)
        layers.save(layers_path)    # co-located weight map
        done += 1
        print(f"  {os.path.relpath(alb, REPO)}: {len(info)} layer(s) -> {os.path.basename(layers_path)} + albedo.png (residual) + albedo_marigold.png")
        for i, (kind, n, seed) in enumerate(info):
            print(f"     ch{i}: {kind:8s} px={n} seed={seed}")
    tail = f" ({skipped} already split — use --force to re-split)" if skipped else ""
    print(f"split_layers: done — {done} albedo(s) split{tail}")

if __name__ == "__main__":
    main()
