#!/usr/bin/env python3
"""bin/art generate-tile — ground/terrain tile sheets from the ComfyUI box.

Terrain is a different problem from creature sprites: no silhouette, no direction, no alpha —
just a plane of texture that must survive being cut into small tiles and repeated. So this does
NOT reuse generate.py's ControlNet/IP path; it is a plain txt2img at high resolution, then a
deterministic cut.

Pipeline (defaults in brackets):

    generate SIZE [1024] --> downscale to GRID*TILE [4*63 = 252] --> optional greyscale
      --> cut GRID x GRID [4x4] tiles of TILE [63] px
      --> pad each tile by PAD [1] px of REPLICATED edge  (cell = TILE + 2*PAD = 65)
      --> assemble one sprite map --> textures/<kind path>/<variant>/<map>.<dir>.<part>.png

Why greyscale by default: the tile DSL already colours terrain by tint —
`::grass> "white &tile.texture set  #4b573e &tile.tint set` — so a neutral pattern tinted per
biome is what the renderer wants, and one sheet then serves grass/dirt/sand by tint alone.

Why edge-replicate rather than leave hard edges: at non-integer zoom the sampler reads just
outside a tile's footprint; a replicated border makes that read the tile's own edge colour
instead of its neighbour, which is the standard fix for atlas bleed.

The output is a SPRITE MAP on purpose — `bin/art remaster` splits sheets into per-variant
leaves, so generated terrain enters the same re-mastering path as hand-authored art.

  bin/art generate-tile --kind biome-tile/default/grass --positive "dense short grass turf"
  bin/art generate-tile --kind biome-tile/default/stone --positive "cracked stone slabs" --candidates 3
"""
import argparse, io, json, os, sys, time, uuid
import urllib.request, urllib.parse
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")
MODEL = "sdxl/cyberrealisticXL_v80.safetensors"

STYLE = ("top-down seamless ground texture, flat cel shading, hand-painted 2D game art, "
         "even flat lighting, no shadows, no objects, no horizon")
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

def cut_tiles(img, grid, tile, pad):
    """Downscale to grid*tile, cut, and edge-replicate each tile by `pad`."""
    n = grid * tile
    src = img.resize((n, n), Image.LANCZOS)
    a = np.asarray(src)
    cells = []
    for gy in range(grid):
        for gx in range(grid):
            t = a[gy*tile:(gy+1)*tile, gx*tile:(gx+1)*tile]
            if pad > 0:
                t = np.pad(t, ((pad, pad), (pad, pad)) + ((0, 0),) * (a.ndim - 2), mode="edge")
            cells.append(t)
    return cells

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
    ap.add_argument("--tile", type=int, default=63, help="tile content px (default 63; grid*tile is the downscale target)")
    ap.add_argument("--pad", type=int, default=1, help="edge-replicated border px per side (default 1 -> 65px cells)")
    ap.add_argument("--colour", action="store_true", help="keep RGB (default: greyscale for tinting)")
    ap.add_argument("--lora", default=None)
    ap.add_argument("--lora-strength", type=float, default=0.85)
    ap.add_argument("--cfg", type=float, default=6.0)
    ap.add_argument("--steps", type=int, default=26)
    ap.add_argument("--map", dest="map_name", default="albedo", help="map name in the leaf (default albedo)")
    ap.add_argument("--dir", dest="dirn", default="l", help="direction field (default l = omni/linked)")
    ap.add_argument("--part", default="0")
    ap.add_argument("--keep-full", action="store_true", help="also save the raw generation beside the sheet")
    args = ap.parse_args()

    import random
    seed0 = args.seed if args.seed is not None else random.randint(1, 2**31 - 1)
    kind = args.kind.strip("/")
    out_root = os.path.join(REPO, "textures", kind)
    pos = f"{args.positive}, {STYLE}"
    neg = ", ".join(x for x in (args.negative.strip(), NEG) if x)
    cell = args.tile + 2 * args.pad

    print(f"generate-tile: kind={kind} seed={seed0} candidates={args.candidates}")
    print(f"  {args.size}px -> {args.grid*args.tile}px -> {args.grid}x{args.grid} tiles of "
          f"{args.tile}px +{args.pad}px pad = {cell}px cells -> {args.grid*cell}px sheet")
    print(f"  mode: {'RGB' if args.colour else 'greyscale (tintable)'}"
          + (f"  lora={args.lora}@{args.lora_strength}" if args.lora else "  no lora"))
    print(f"  positive -> {pos}")

    for k in range(max(1, args.candidates)):
        seed = seed0 + k
        raw = _run(graph(pos, neg, args.lora, args.lora_strength, args.cfg, args.steps, seed, args.size))
        full = Image.open(io.BytesIO(raw)).convert("RGB")
        src = full if args.colour else to_grey(full)
        cells = cut_tiles(src, args.grid, args.tile, args.pad)
        sheet = sheet_from(cells, args.grid, cell)

        variant = args.variant if (args.variant and args.candidates == 1) else str(seed)
        leaf = os.path.join(out_root, texpath.variant_leaf(variant))
        os.makedirs(leaf, exist_ok=True)
        outp = os.path.join(leaf, texpath.map_name(args.map_name, args.dirn, args.part))
        sheet.save(outp)
        with open(os.path.join(leaf, "atlas.json"), "w") as f:
            json.dump({"grid": [args.grid, args.grid], "tile": args.tile,
                       "pad": args.pad, "cell": cell, "source_size": args.size,
                       "greyscale": not args.colour, "seed": seed}, f, indent=2)
        if args.keep_full:
            full.save(os.path.join(leaf, f"source.{args.dirn}.{args.part}.png"))
        print(f"  wrote {os.path.relpath(outp, REPO)}  ({sheet.size[0]}x{sheet.size[1]}, "
              f"{args.grid*args.grid} tiles) + atlas.json")
    print(f"generate-tile: done ({kind})")

if __name__ == "__main__":
    main()
