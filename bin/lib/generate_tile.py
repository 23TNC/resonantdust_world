#!/usr/bin/env python3
"""bin/art generate-tile — ground/terrain tile sheets from the ComfyUI box.

Terrain is a different problem from creature sprites: no silhouette, no direction, no alpha —
just a plane of texture that must survive being cut into small tiles and repeated. So this does
NOT reuse generate.py's ControlNet/IP path; it is a plain txt2img at high resolution, then a
deterministic cut.

Pipeline (defaults in brackets):

    generate SIZE [1024] --> optional greyscale --> downscale to GRID*(TILE+OVERLAP) [4*93]
      --> cut GRID x GRID [4x4] blocks --> wrap each into a TILE [62] px toroidal tile
      --> pad each by PAD [1] px of WRAPPED edge  (cell = TILE + 2*PAD = 64)
      --> assemble one sprite map --> textures/<kind path>/<variant>/<map>.<dir>.<part>.png

Why the tiles are made to wrap here rather than by the model: a seamless-generation patch can
only make the whole 1024px plane wrap, but the game repeats a single 63px CELL, and every cell
edge is an arbitrary cut through the middle of that plane. (ComfyUI's one such node,
`Model Patch Seamless (mtb)`, also segfaults this box.) So each cell is joined to itself along a
minimum-error boundary cut, using surplus texture the downscale would otherwise have thrown away.
`--seamless sheet` instead wraps the whole plane, for a sheet meant to be laid down as a 4x4 unit.

Why greyscale by default: the tile DSL already colours terrain by tint —
`::grass> "white &tile.texture set  #4b573e &tile.tint set` — so a neutral pattern tinted per
biome is what the renderer wants, and one sheet then serves grass/dirt/sand by tint alone.

Why pad at all: at non-integer zoom the sampler reads just outside a tile's footprint, and
without a guard that read lands on the neighbouring cell in the sheet. The guard WRAPS rather
than replicating, because on a toroidal tile the pixel past an edge genuinely is the opposite
edge — replicating there would contradict the continuity the wrap just established, at exactly
the boundary the sampler reaches for.

The output is a SPRITE MAP on purpose — `bin/art remaster` splits sheets into per-variant
leaves, so generated terrain enters the same re-mastering path as hand-authored art.

  bin/art generate-tile --kind biome-tile/default/grass --positive "dense short grass turf"
  bin/art generate-tile --kind biome-tile/default/stone --positive "cracked stone slabs" --candidates 3
"""
import argparse, io, json, os, sys, time, uuid
import urllib.request, urllib.parse
import numpy as np
from PIL import Image, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")
MODEL = "sdxl/cyberrealisticXL_v80.safetensors"

# "seamless ground texture" is deliberately ABSENT. It used to be here, from when we hoped the
# model would hand us tileable output — but we build the wrap ourselves now, and the phrase costs
# us the only thing that matters: it makes SDXL paint a fine all-over repeat, which the shrink to
# a 62px tile then averages into mush (measured 1.6px features with it, ~13px without). Ask for
# the SUBJECT at the scale you want to see it; the tiling is our job, not the model's.
STYLE = ("flat cel shading, hand-painted 2D game art, even flat lighting, no shadows, no horizon")
NEG = ("realistic, photo, photorealistic, 3d render, blurry, vignette, border, frame, "
       "object, creature, plant stems, horizon, perspective, depth of field, text, watermark, "
       "drop shadow, signature")

# ---------------------------------------------------------------- comfy
def _run(graph, timeout=300):
    req = urllib.request.Request(COMFY + "/prompt",
        data=json.dumps({"prompt": graph, "client_id": uuid.uuid4().hex}).encode(),
        headers={"Content-Type": "application/json"})
    try:
        pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
    except urllib.error.HTTPError as e:
        detail = ""
        try: detail = json.dumps(json.loads(e.read().decode()).get("node_errors", {}))[:500]
        except Exception: pass
        raise SystemExit(f"generate-tile: ComfyUI rejected the workflow ({e.code}). {detail}")
    t0 = time.time()
    while time.time() - t0 < timeout:
        h = json.load(urllib.request.urlopen(f"{COMFY}/history/{pid}", timeout=20))
        if pid in h:
            outs = h[pid].get("outputs", {})
            if "9" not in outs or not outs["9"].get("images"):
                raise SystemExit(f"generate-tile: run produced no image (status {h[pid].get('status')}). "
                                 "If training is running, the GPU may be full.")
            im = outs["9"]["images"][0]
            q = urllib.parse.urlencode({"filename": im["filename"],
                 "subfolder": im.get("subfolder", ""), "type": im["type"]})
            return urllib.request.urlopen(f"{COMFY}/view?{q}", timeout=60).read()
        time.sleep(2)
    raise SystemExit("generate-tile: timed out waiting for ComfyUI")

def graph(pos, neg, lora, strength, cfg, steps, seed, size):
    """A plain txt2img. Seamlessness is NOT requested from the model — see `make_toroidal`.

    ComfyUI's only circular-padding node, `Model Patch Seamless (mtb)`, deep-copies the whole
    UNet and segfaults this box (CUDA error inside `copy.deepcopy`, taking the server down with
    it). It would not have helped regardless: it wraps the 1024px *generation*, but we cut that
    into sixteen 62px cells and the game tiles at the cell, so every cut edge would still be
    arbitrary. Wrapping has to happen where the cells are made."""
    g = {"4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": MODEL}}}
    model_ref, clip_ref = ["4", 0], ["4", 1]
    if lora:
        g["10"] = {"class_type": "LoraLoader", "inputs": {"model": ["4", 0], "clip": ["4", 1],
                   "lora_name": lora, "strength_model": strength, "strength_clip": strength}}
        model_ref, clip_ref = ["10", 0], ["10", 1]
    g.update({
     "6": {"class_type": "CLIPTextEncode", "inputs": {"text": pos, "clip": clip_ref}},
     "7": {"class_type": "CLIPTextEncode", "inputs": {"text": neg, "clip": clip_ref}},
     "5": {"class_type": "EmptyLatentImage", "inputs": {"width": size, "height": size, "batch_size": 1}},
     "3": {"class_type": "KSampler", "inputs": {"seed": seed, "steps": steps, "cfg": cfg,
           "sampler_name": "euler_ancestral", "scheduler": "normal", "denoise": 1.0,
           "model": model_ref, "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0]}},
     "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
     "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": "rdtile", "images": ["8", 0]}}})
    return g

# ---------------------------------------------------------------- cutting
def to_grey(img, keep_range=(16, 240)):
    """Luminance, then stretched into keep_range.

    Headroom at both ends is deliberate: a tint multiplies, so a tile that already contains pure
    white or pure black cannot be darkened or lightened by tinting — it clips. Leaving the extremes
    free keeps every tile fully tintable, which is the whole reason terrain is greyscale here."""
    a = np.asarray(img.convert("L"), dtype=np.float32)
    lo, hi = float(a.min()), float(a.max())
    if hi - lo < 1e-3: return img.convert("L")
    lo_t, hi_t = keep_range
    a = (a - lo) / (hi - lo) * (hi_t - lo_t) + lo_t
    return Image.fromarray(a.clip(0, 255).astype(np.uint8), "L")

# ---------------------------------------------------------------- seamless
def _mincut(cost):
    """Cheapest 8-connected top-to-bottom path through `cost` (H, o). Returns a column per row.

    Dynamic programming, the Efros-Freeman minimum-error boundary cut. Picking a *path* rather
    than cross-fading matters here: a fade averages two unrelated pieces of texture into a soft
    band, which on stone reads as a smear straight down the tile edge. A cut keeps every pixel
    verbatim and simply chooses the line where the two sides already agree."""
    H, o = cost.shape
    acc = cost.astype(np.float64).copy()
    back = np.zeros((H, o), np.int32)
    for y in range(1, H):
        prev = acc[y-1]
        cand = np.stack([np.r_[np.inf, prev[:-1]], prev, np.r_[prev[1:], np.inf]])  # -1, 0, +1
        back[y] = np.argmin(cand, 0) - 1
        acc[y] += cand.min(0)
    path = np.empty(H, np.int32)
    path[-1] = int(np.argmin(acc[-1]))
    for y in range(H - 1, 0, -1):
        path[y-1] = path[y] + back[y, path[y]]
    return path

def _blend(a, b, path, feather=2.0):
    """Merge two overlapping strips along a GIVEN cut path: starts as `a`, ends as `b`."""
    xs = np.arange(a.shape[1], dtype=np.float64)[None, :]
    m = np.clip((xs - path[:, None] + 0.5) / feather + 0.5, 0.0, 1.0)   # 0 -> a, 1 -> b
    if a.ndim == 3: m = m[..., None]
    return (a * (1.0 - m) + b * m).round().astype(a.dtype)

def _toroidal_axis(maps, ov):
    """Each (H, W[,C]) -> (H, W-ov[,C]) whose first column continues from its last.

    The output is [ join(right_strip, left_strip) | interior ]. Its last column is the source
    column just before `right_strip`, and its first column IS the first column of `right_strip` —
    so the wrap point is a join the source already made. The join then walks back to the left
    strip, which meets the interior at the source's own column `ov`. Both ends continuous.

    `maps` travel together and are cut along ONE path, derived from maps[0]. Companion maps must
    never choose their own: a layers map cut on its own error surface would take a different route
    through the band than the albedo did, and every pixel between the two routes would carry one
    material's tint over the other material's colour."""
    W = maps[0].shape[1]
    d = np.abs(maps[0][:, W-ov:].astype(np.float64) - maps[0][:, :ov].astype(np.float64))
    if d.ndim == 3: d = d.mean(2)
    path = _mincut(d)
    return [np.concatenate([_blend(m[:, W-ov:], m[:, :ov], path), m[:, ov:W-ov]], axis=1)
            for m in maps]

def make_toroidal(a, ov):
    """(n+ov, n+ov[,C]) -> (n, n[,C]) tiling cleanly against itself on both axes.

    Pass a LIST to carry companion maps (layers, normal, ...) through the identical cut; the
    geometry is decided by the first and every map is sliced the same way. Returns whatever kind
    it was given."""
    one = not isinstance(a, (list, tuple))
    maps = [a] if one else list(a)
    maps = _toroidal_axis(maps, ov)
    maps = [m.swapaxes(0, 1) for m in _toroidal_axis([m.swapaxes(0, 1) for m in maps], ov)]
    return maps[0] if one else maps

def feature_size(t):
    """Dominant feature width in px: first zero-crossing of the horizontal autocorrelation.

    This is the number that decides whether a material still READS after the cut. SDXL paints a
    roughly fixed number of features per frame whatever the canvas, so the same "grass turf"
    prompt gives ~10px blades at 512 and ~2px blades at 1024 — and 2px blades average into grey
    mush. Bigger canvas therefore means LESS surviving texture, not more, which is the opposite
    of the intuition. Coarse materials (flagstone) are unaffected; fine ones (grass, gravel,
    sand) need the smaller canvas."""
    a = np.asarray(t, np.float64)
    if a.ndim == 3: a = a.mean(2)
    r = a - a.mean()
    if not r.any(): return float(a.shape[1])
    ac = [float((r[:, :-k] * r[:, k:]).mean()) for k in range(1, 13)]
    v0 = float((r * r).mean())
    return float(next((k for k, c in enumerate(ac, 1) if c <= 0), 13)) if v0 else 0.0

def seam_energy(t):
    """Mean |delta| across the wrap boundary / mean |delta| inside. ~1.0 means the seam is
    statistically indistinguishable from ordinary texture, i.e. invisible."""
    a = np.asarray(t, np.float64)
    if a.ndim == 3: a = a.mean(2)
    wrap = np.abs(a[:, 0] - a[:, -1]).mean() + np.abs(a[0, :] - a[-1, :]).mean()
    inner = np.abs(np.diff(a, axis=1)).mean() + np.abs(np.diff(a, axis=0)).mean()
    return float(wrap / inner) if inner > 1e-9 else 0.0

def flatten_field(a, radius, amount=1.0):
    """Divide out luminance variation broader than `radius`, preserving hue.

    The model lights the 1024px plane softly across its whole area, so cells cut from opposite
    corners arrive at different base brightness — 43/255 apart on the first flagstone sheet. Each
    cell may tile perfectly with itself and the ground still reads as a patchwork, because the
    game picks cells independently and lays them side by side. A minimum-error cut cannot fix
    that: it chooses *where* to join, never changes a value.

    It is also wrong to keep: the renderer lights terrain from the lightmap, so baked-in
    illumination would be lit twice. What we want from the model is the material, not its lighting."""
    if amount <= 0: return a
    f = a.astype(np.float64)
    lum = f.mean(2) if f.ndim == 3 else f
    lo = np.asarray(Image.fromarray(lum.astype(np.uint8), "L")
                    .filter(ImageFilter.GaussianBlur(radius)), np.float64)
    gain = np.where(lo > 1.0, lum.mean() / np.maximum(lo, 1.0), 1.0)
    gain = 1.0 + (gain - 1.0) * amount
    return np.clip(f * (gain[..., None] if f.ndim == 3 else gain), 0, 255).astype(a.dtype)

def reconstruct(res, lay, tints):
    """What the renderer will actually show: residual + sum(layer_weight * material_tint).

    Every quality number must be measured on THIS, never on the residual alone. For a
    single-material texture split_layers moves the entire image into one layer channel and leaves
    a residual of pure black — measuring that reports a flawless seam on a blank tile."""
    out = np.asarray(res, np.float64)
    for i, t in enumerate(tints[:3]):
        out = out + (np.asarray(lay, np.float64)[..., i:i+1] / 255.0) * np.array(t, np.float64)
    return np.clip(out, 0, 255).astype(np.uint8)

def split_plane(rgb, dirn, part):
    """Run `art split_layers` on the WHOLE plane; returns (residual_rgb, layers_rgb, tints).

    On the whole plane, never per tile. The split clusters colours in the chroma plane to decide
    which material owns which channel, so splitting each 62px tile on its own would let tile A put
    grass in R and tile B put grass in G — the sheet would tint into confetti. One split for the
    sheet fixes the assignment once, and the cut then carries both maps through together."""
    import subprocess, tempfile, shutil
    with tempfile.TemporaryDirectory() as d:
        Image.fromarray(rgb, "RGB").save(os.path.join(d, f"albedo.{dirn}.{part}.png"))
        r = subprocess.run([sys.executable, os.path.join(HERE, "split_layers.py"), "--normalize", d],
                           capture_output=True, text=True)
        lp = os.path.join(d, f"layers.{dirn}.{part}.png")
        if r.returncode != 0 or not os.path.exists(lp):
            tail = (r.stderr.strip().splitlines() or ["no layers produced"])[-1]
            raise SystemExit(f"generate-tile: split_layers failed — {tail[:200]}")
        res = np.asarray(Image.open(os.path.join(d, f"albedo.{dirn}.{part}.png")).convert("RGB"))
        lay = np.asarray(Image.open(lp).convert("RGB"))
        tints = []
        mp = os.path.join(d, "meta.json")
        if os.path.exists(mp):
            tints = json.load(open(mp)).get("channel_tints", [])
        return res, lay, tints

# ---------------------------------------------------------------- cutting
def cut_tiles(img, grid, tile, pad, seamless="cell", ov=16, flatten=1.0, match_cells=False,
              companion=None):
    """Downscale, cut into grid*grid tiles, pad each for atlas bleed.

    `seamless` picks what wraps:
      cell  - every tile wraps against ITSELF, so any tile may be repeated anywhere. This is what
              the game does with ground: it picks a cell per world tile independently.
      sheet - the whole grid*tile plane wraps, then it is cut. Neighbouring cells then join
              perfectly in the sheet's own 4x4 order, but a single cell repeated alone will seam.
      none  - straight cut.

    Padding follows: on a wrapping tile the pixel just outside an edge IS the opposite edge, so
    the bleed guard must wrap too. Replicating there would contradict the continuity we just built,
    at exactly the boundary the sampler reaches for."""
    mode = "wrap" if seamless in ("cell", "sheet") else "edge"
    # `companion` rides along untouched by flatten (a layers map holds material weights, not
    # light) but is resized and cut in exact lockstep with the albedo.
    def prep(px):
        a = flatten_field(np.asarray(img.resize((px, px), Image.LANCZOS)), tile, flatten)
        if companion is None: return [a]
        return [a, np.asarray(Image.fromarray(companion, "RGB").resize((px, px), Image.LANCZOS))]

    if seamless == "cell":
        blk = tile + ov
        ms = prep(grid*blk)
        raw = [make_toroidal([m[gy*blk:(gy+1)*blk, gx*blk:(gx+1)*blk] for m in ms], ov)
               for gy in range(grid) for gx in range(grid)]
    else:
        n = grid * tile
        ms = prep(n + (ov if seamless == "sheet" else 0))
        if seamless == "sheet":
            ms = make_toroidal(ms, ov)
        raw = [[m[gy*tile:(gy+1)*tile, gx*tile:(gx+1)*tile] for m in ms]
               for gy in range(grid) for gx in range(grid)]
    raw, comp = [r[0] for r in raw], [r[1] for r in raw] if companion is not None else None
    if match_cells:
        # A uniform offset shifts both edges of a tile equally, so self-tiling is untouched
        # (measured: seam energy 0.94 before and after). What it costs is real material variety —
        # a flagstone cell that genuinely is one big pale slab gets pulled to the mean. Off by
        # default for that reason; `cell_brightness_spread` in atlas.json says when to reach for it.
        tgt = float(np.mean([np.asarray(t, np.float64).mean() for t in raw]))
        raw = [np.clip(np.asarray(t, np.float64) + (tgt - np.asarray(t, np.float64).mean()),
                       0, 255).astype(t.dtype) for t in raw]
    def guard(ts):
        if pad <= 0: return ts
        return [np.pad(t, ((pad, pad), (pad, pad)) + ((0, 0),) * (t.ndim - 2), mode=mode) for t in ts]
    return guard(raw) if comp is None else (guard(raw), guard(comp))

def sheet_from(cells, grid, cell_px):
    """Assemble cells into one grid x grid sprite map."""
    first = cells[0]
    shape = (grid*cell_px, grid*cell_px) + ((first.shape[2],) if first.ndim == 3 else ())
    out = np.zeros(shape, dtype=first.dtype)
    for i, c in enumerate(cells):
        gy, gx = divmod(i, grid)
        out[gy*cell_px:(gy+1)*cell_px, gx*cell_px:(gx+1)*cell_px] = c
    return Image.fromarray(out, "L" if first.ndim == 2 else "RGB")

# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser(prog="art generate-tile",
        description="Generate ground/terrain tile sheets and place them in the texture tree.")
    ap.add_argument("--kind", required=True,
                    help="kind path under textures/, e.g. biome-tile/default/grass")
    ap.add_argument("--positive", required=True, help="what the ground is made of")
    ap.add_argument("--negative", default="", help="extra negatives (generic ones are always added)")
    ap.add_argument("--variant", default=None, help="variant folder (default: the seed)")
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--candidates", type=int, default=1, help="generate N sheets, one variant each")
    ap.add_argument("--size", type=int, default=1024, help="generation resolution (default 1024, SDXL native)")
    ap.add_argument("--grid", type=int, default=4, help="tiles per side (default 4 -> 16 tiles)")
    ap.add_argument("--tile", type=int, default=62, help="tile content px (default 62 -> 64 with pad, the quadtree packer's cell)")
    ap.add_argument("--pad", type=int, default=1, help="bleed-guard border px per side, wrapped (default 1 -> 64px cells)")
    ap.add_argument("--colour", action="store_true", help="keep RGB (default: greyscale for tinting)")
    ap.add_argument("--layers", action="store_true",
                    help="also split the plane into a channel-packed layers map (implies --colour), "
                         "so each material can be tinted per biome instead of the whole tile at once")
    ap.add_argument("--lora", default=None)
    ap.add_argument("--lora-strength", type=float, default=0.85)
    ap.add_argument("--cfg", type=float, default=6.0)
    ap.add_argument("--steps", type=int, default=26)
    ap.add_argument("--map", dest="map_name", default="albedo", help="map name in the leaf (default albedo)")
    ap.add_argument("--dir", dest="dirn", default="l", help="direction field (default l = omni/linked)")
    ap.add_argument("--part", default="0")
    ap.add_argument("--seamless", choices=("cell", "sheet", "none"), default="cell",
                    help="what wraps: each tile against itself (default), the whole sheet, or nothing")
    ap.add_argument("--overlap", type=int, default=0,
                    help="px of surplus texture the wrap join may cut through (default 0 = tile//2)")
    ap.add_argument("--flatten", type=float, default=1.0, metavar="AMOUNT",
                    help="divide out lighting broader than a tile so cells match in brightness "
                         "(default 1.0 = full; 0 keeps the model's baked lighting)")
    ap.add_argument("--match-cells", action="store_true",
                    help="pull every cell to the same mean brightness — evens out a patchwork "
                         "look at the cost of real material variety (seams are unaffected)")
    ap.add_argument("--keep-full", action="store_true", help="also save the raw generation beside the sheet")
    args = ap.parse_args()

    import random
    if args.layers: args.colour = True
    seed0 = args.seed if args.seed is not None else random.randint(1, 2**31 - 1)
    kind = args.kind.strip("/")
    out_root = os.path.join(REPO, "textures", kind)
    pos = f"{args.positive}, {STYLE}"
    neg = ", ".join(x for x in (args.negative.strip(), NEG) if x)
    cell = args.tile + 2 * args.pad
    # tile//2 measured best on both a structured texture (flagstone 1.16 -> 0.94 seam energy) and
    # an unstructured one (grass 1.01 -> 0.99), and both got worse either side of it: too narrow a
    # band leaves the cut no route around a hard feature like a mortar line, while too wide a one
    # pushes the downscale target up so the strips being joined carry finer detail to disagree on.
    ov = min(args.tile - 1, args.overlap if args.overlap > 0 else max(4, args.tile // 2))
    down = args.grid * (args.tile + ov) if args.seamless == "cell" else \
           args.grid * args.tile + (ov if args.seamless == "sheet" else 0)

    if down > args.size:
        raise SystemExit(
            f"generate-tile: grid {args.grid} needs a {down}px plane but --size is {args.size}, so "
            f"every tile would be UPSCALED from too little source and come out soft.\n"
            f"  either drop to --grid {args.size // (args.tile + ov)} (max at this size), "
            f"or raise --size to {down} or more.")

    print(f"generate-tile: kind={kind} seed={seed0} candidates={args.candidates}")
    print(f"  {args.size}px -> {down}px ({args.size/down:.2f}x shrink) -> {args.grid}x{args.grid} tiles of "
          f"{args.tile}px +{args.pad}px pad = {cell}px cells -> {args.grid*cell}px sheet")
    print(f"  seamless: {args.seamless}"
          + (f" (min-error cut, {ov}px overlap, wrap padding)" if args.seamless != "none" else ""))
    print(f"  mode: {'RGB' if args.colour else 'greyscale (tintable)'}"
          + (f"  lora={args.lora}@{args.lora_strength}" if args.lora else "  no lora"))
    print(f"  positive -> {pos}")

    for k in range(max(1, args.candidates)):
        seed = seed0 + k
        raw = _run(graph(pos, neg, args.lora, args.lora_strength, args.cfg, args.steps,
                         seed, args.size))
        full = Image.open(io.BytesIO(raw)).convert("RGB")
        src = full if args.colour else to_grey(full)
        companion, tints = None, []
        if args.layers:
            plane = np.asarray(src.convert("RGB"))
            resid, companion, tints = split_plane(
                flatten_field(plane, max(8, int(args.tile * args.size / down)), args.flatten),
                args.dirn, args.part)
            src = Image.fromarray(resid, "RGB")      # the residual becomes the render base
        out = cut_tiles(src, args.grid, args.tile, args.pad, args.seamless, ov,
                        0.0 if args.layers else args.flatten, args.match_cells, companion)
        cells, lay_cells = out if args.layers else (out, None)
        sheet = sheet_from(cells, args.grid, cell)
        # unpadded content is what repeats; measure the seam on that, not on the bleed guard
        shown = cells if lay_cells is None else [reconstruct(a, l, tints)
                                                for a, l in zip(cells, lay_cells)]
        inner = [c[args.pad:c.shape[0]-args.pad, args.pad:c.shape[1]-args.pad] if args.pad else c
                 for c in shown]
        se = [seam_energy(c) for c in inner]
        mu = [float(np.asarray(c, float).mean()) for c in inner]
        fs = sum(feature_size(c) for c in inner) / len(inner)

        variant = args.variant if (args.variant and args.candidates == 1) else str(seed)
        leaf = os.path.join(out_root, texpath.variant_leaf(variant))
        os.makedirs(leaf, exist_ok=True)
        outp = os.path.join(leaf, texpath.map_name(args.map_name, args.dirn, args.part))
        sheet.save(outp)
        with open(os.path.join(leaf, "atlas.json"), "w") as f:
            json.dump({"grid": [args.grid, args.grid], "tile": args.tile,
                       "pad": args.pad, "cell": cell, "source_size": args.size,
                       "greyscale": not args.colour, "seed": seed,
                       "seamless": args.seamless, "overlap": ov, "flatten": args.flatten,
                       "match_cells": args.match_cells,
                       "seam_energy": round(sum(se)/len(se), 3), "feature_size": round(fs, 1),
                       "cell_brightness_spread": round(max(mu)-min(mu), 1)}, f, indent=2)
        if lay_cells is not None:
            sheet_from(lay_cells, args.grid, cell).save(
                os.path.join(leaf, texpath.map_name("layers", args.dirn, args.part)))
            with open(os.path.join(leaf, "meta.json"), "w") as f:
                json.dump({"channel_tints": tints}, f)
            print(f"    layers: {len(tints)} material(s) {tints} -> layers map beside the albedo")
        if args.keep_full:
            full.save(os.path.join(leaf, f"source.{args.dirn}.{args.part}.png"))
        print(f"  wrote {os.path.relpath(outp, REPO)}  ({sheet.size[0]}x{sheet.size[1]}, "
              f"{args.grid*args.grid} tiles) + atlas.json")
        print(f"    seam energy {sum(se)/len(se):.2f} (worst {max(se):.2f}) — 1.0 = seam "
              f"indistinguishable from ordinary texture; cell brightness spread {max(mu)-min(mu):.0f}/255")
        print(f"    feature size {fs:.1f}px" + ("" if fs >= 3.0 else
              f"  ** too fine to read at {args.tile}px — this material's detail is averaging into "
              f"mush. SDXL paints a fixed number of features per frame, so try --size "
              f"{max(512, args.size // 2)} --grid {max(2, (args.size // 2) // (args.tile + ov))} "
              f"to make each feature bigger (a LARGER canvas makes this worse, not better)."))
    print(f"generate-tile: done ({kind})")

if __name__ == "__main__":
    main()
