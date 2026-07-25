#!/usr/bin/env python3
"""bin/art lora-eval — objective quality harness for the sprite LoRAs.

Generates a fixed subject x direction matrix through ComfyUI for one or more LoRA
checkpoints / strengths / cfg values, then SCORES each sprite against the metrics that
encode what a good sprite actually is. This replaces eyeballing 5 samples: it catches
the failures we kept hitting (multi-subject sheets, background bleed, scale drift,
bust-instead-of-body) as numbers, so epoch + strength + cfg can be chosen objectively.

Metrics (all pure numpy/PIL, no scipy):
  blobs       connected components of the subject mask  -> 1 is correct; >1 = sprite sheet
  bg          fraction of BORDER pixels that are white  -> 1.0 is clean; <1 = background bleed
  fill        subject bbox area / frame area            -> compared to the real sprite's fill
  aspect      bbox width/height                         -> catches bust (tall/square) vs body (wide)
  solidity    mask pixels / bbox area                   -> very low = wispy/fragmented
  d_fill      |fill - reference fill|                   -> scale drift vs the training sprite
  d_aspect    |aspect - reference aspect|               -> proportion drift (the bust detector)
  score       0-100 composite; higher is better

Reference values come from the REAL training sprite for that subject+direction under
.staging/animal-lora/<Folder>/, so "correct" is defined by your own art, not a guess.

Usage:
  python3 bin/lib/lora_eval.py --loras rd_quadruped_e07.safetensors,rd_quadruped_e08.safetensors
  python3 bin/lib/lora_eval.py --loras rd_quadruped_e07.safetensors --strengths 0.7,0.85,1.0
"""
import argparse, json, os, sys, time, uuid, io, csv
import urllib.request, urllib.parse
from collections import deque
import numpy as np
from PIL import Image

REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")
MODEL = "sdxl/cyberrealisticXL_v80.safetensors"
SRC   = os.path.join(REPO, ".staging", "animal-lora")

# subject -> (family, reference folder under .staging/animal-lora, file stem prefix, seed)
SUBJECTS = [
    ("wolf",  "canine", "Wolf_Timber", "Wolf_Timber", 1001),
    ("tiger", "feline", "Tiger",       "Tiger",       1002),
    ("bear",  "bear",   "Bear",        "Bear",        1003),
    ("cat",   "feline", "Cat",         "Cat",         1004),
]
DIRS = {
    "e": ("rd_east",  "side profile, side view, facing right",         "east"),
    "s": ("rd_south", "front view, facing the viewer, facing forward", "south"),
    "n": ("rd_north", "back view, facing away, seen from behind",      "north"),
}
NEG = ("realistic, photo, photorealistic, 3d render, blurry, multiple, sprite sheet, grid, "
       "collage, close-up, portrait, logo, emblem, frame, background scenery, text, watermark")

# ---------------------------------------------------------------- comfy
def _run(graph, timeout=240):
    req = urllib.request.Request(COMFY + "/prompt",
        data=json.dumps({"prompt": graph, "client_id": uuid.uuid4().hex}).encode(),
        headers={"Content-Type": "application/json"})
    pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
    t0 = time.time()
    while time.time() - t0 < timeout:
        h = json.load(urllib.request.urlopen(f"{COMFY}/history/{pid}", timeout=20))
        if pid in h:
            im = h[pid]["outputs"]["9"]["images"][0]
            q = urllib.parse.urlencode({"filename": im["filename"],
                 "subfolder": im.get("subfolder", ""), "type": im["type"]})
            return urllib.request.urlopen(f"{COMFY}/view?{q}", timeout=60).read()
        time.sleep(2)
    raise SystemExit("lora-eval: timed out waiting for ComfyUI")

def graph(pos, lora, strength, cfg, seed, size=768, steps=24):
    return {
     "4":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":MODEL}},
     "10":{"class_type":"LoraLoader","inputs":{"model":["4",0],"clip":["4",1],
           "lora_name":lora,"strength_model":strength,"strength_clip":strength}},
     "6":{"class_type":"CLIPTextEncode","inputs":{"text":pos,"clip":["10",1]}},
     "7":{"class_type":"CLIPTextEncode","inputs":{"text":NEG,"clip":["10",1]}},
     "5":{"class_type":"EmptyLatentImage","inputs":{"width":size,"height":size,"batch_size":1}},
     "3":{"class_type":"KSampler","inputs":{"seed":seed,"steps":steps,"cfg":cfg,
           "sampler_name":"euler_ancestral","scheduler":"normal","denoise":1.0,
           "model":["10",0],"positive":["6",0],"negative":["7",0],"latent_image":["5",0]}},
     "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["4",2]}},
     "9":{"class_type":"SaveImage","inputs":{"filename_prefix":"loraeval","images":["8",0]}}}

# ---------------------------------------------------------------- metrics
def _bg_color(a, cs=6):
    """Background colour taken from the four corners — mirrors remove_bg_floodfill() in
    generate.py, which keys the background by detected corner colour, not by pure white.
    A uniform off-white/grey plate is therefore FINE (it keys out cleanly); what actually
    breaks the pipeline is a NON-UNIFORM background (scenery/gradient)."""
    c = np.vstack([a[:cs,:cs].reshape(-1,3), a[:cs,-cs:].reshape(-1,3),
                   a[-cs:,:cs].reshape(-1,3), a[-cs:,-cs:].reshape(-1,3)])
    return np.median(c, axis=0)

def _mask(img, thresh=40):
    """Subject mask = pixels far from the DETECTED background colour."""
    a = np.asarray(img.convert("RGB")).astype(np.float32)
    return np.sqrt(((a - _bg_color(a))**2).sum(axis=2)) > thresh

def _filled(mask, grid=128):
    """Downsample, then FILL enclosed regions: flood the background inward from the border
    through non-mask pixels; anything unreached is interior. Without this a pale animal
    inside a dark outline (white bear/cat) reads as a hollow ring and scores as several
    blobs — the outline is the mask, the light body is not."""
    small = np.asarray(Image.fromarray((mask * 255).astype(np.uint8)).resize((grid, grid),
                       Image.BILINEAR)) > 96
    bg = np.zeros_like(small, bool)
    q = deque()
    for i in range(grid):
        for (y, x) in ((0,i),(grid-1,i),(i,0),(i,grid-1)):
            if not small[y,x] and not bg[y,x]: bg[y,x] = True; q.append((y,x))
    while q:
        cy, cx = q.popleft()
        for dy, dx in ((1,0),(-1,0),(0,1),(0,-1)):
            ny, nx = cy+dy, cx+dx
            if 0 <= ny < grid and 0 <= nx < grid and not small[ny,nx] and not bg[ny,nx]:
                bg[ny,nx] = True; q.append((ny,nx))
    return ~bg

def _components(small, min_frac=0.004):
    """Count connected blobs on an already-filled small mask."""
    grid = small.shape[0]
    seen = np.zeros_like(small, bool); n = 0; total = small.sum() or 1
    for y in range(grid):
        for x in range(grid):
            if small[y, x] and not seen[y, x]:
                q = deque([(y, x)]); seen[y, x] = True; sz = 0
                while q:
                    cy, cx = q.popleft(); sz += 1
                    for dy, dx in ((1,0),(-1,0),(0,1),(0,-1)):
                        ny, nx = cy+dy, cx+dx
                        if 0 <= ny < grid and 0 <= nx < grid and small[ny,nx] and not seen[ny,nx]:
                            seen[ny,nx] = True; q.append((ny,nx))
                if sz / total >= min_frac: n += 1
    return n

def measure(img):
    m = _mask(img); H, W = m.shape
    if m.sum() < 50:
        return dict(blobs=0, bg=1.0, white=1.0, fill=0.0, aspect=0.0, solidity=0.0)
    ys, xs = np.where(m)
    y0, y1, x0, x1 = ys.min(), ys.max(), xs.min(), xs.max()
    bw, bh = (x1-x0+1), (y1-y0+1)
    border = np.concatenate([m[0,:], m[-1,:], m[:,0], m[:,-1]])
    a = np.asarray(img.convert("RGB")).astype(np.float32)
    white = float(max(0.0, 1.0 - np.sqrt(((_bg_color(a) - 255.0)**2).sum()) / 120.0))
    small = _filled(m)                                  # holes filled -> solid silhouette
    g = small.shape[0]
    sy, sx = np.where(small)
    sol = float(small.sum()) / max(1.0, float((sx.max()-sx.min()+1) * (sy.max()-sy.min()+1))) if small.any() else 0.0
    return dict(
        blobs=_components(small),
        bg=float(1.0 - border.mean()),                 # 1.0 = uniform keyable plate
        white=round(white,3),                          # how close that plate is to pure white
        fill=float(bw*bh) / float(W*H),
        aspect=float(bw)/float(bh),
        solidity=sol,
    )

def reference(folder, stem, direction):
    """Metrics of the REAL training sprite (composited on white), or None."""
    p = os.path.join(SRC, folder, f"{stem}_{direction}.png")
    if not os.path.exists(p):
        for cand in sorted(os.listdir(os.path.join(SRC, folder))) if os.path.isdir(os.path.join(SRC, folder)) else []:
            if cand.lower().endswith(f"_{direction}.png"): p = os.path.join(SRC, folder, cand); break
    if not os.path.exists(p): return None
    im = Image.open(p).convert("RGBA")
    bg = Image.new("RGBA", im.size, (255,255,255,255)); bg.alpha_composite(im)
    return measure(bg.convert("RGB"))

def score(g, r):
    """0-100 composite. Blobs and background are hard requirements; fill/aspect are
    graded against the real sprite so 'correct' means 'looks like our art'."""
    s = 100.0
    s -= 40.0 * max(0, g["blobs"] - 1)          # extra subjects: heavily penalised
    if g["blobs"] == 0: return 0.0
    s -= 60.0 * max(0.0, 1.0 - g["bg"]) * 2.0   # non-uniform bg (scenery) — breaks the key
    s -= 8.0  * max(0.0, 1.0 - g.get("white", 1.0))  # off-white plate: minor, it still keys out
    if r:
        s -= 60.0 * min(1.0, abs(g["fill"] - r["fill"]) / max(r["fill"], 1e-3))
        s -= 40.0 * min(1.0, abs(g["aspect"] - r["aspect"]) / max(r["aspect"], 1e-3))
    s -= 20.0 * max(0.0, 0.35 - g["solidity"]) / 0.35
    return max(0.0, min(100.0, s))

# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser(prog="art lora-eval")
    ap.add_argument("--loras", required=True, help="comma-separated LoRA filenames as ComfyUI sees them")
    ap.add_argument("--strengths", default="0.85", help="comma-separated LoRA strengths (default 0.85)")
    ap.add_argument("--cfgs", default="6.0", help="comma-separated cfg values (default 6.0)")
    ap.add_argument("--out", default=".staging/lora-eval", help="output dir for images + results.csv")
    ap.add_argument("--size", type=int, default=768)
    args = ap.parse_args()

    out = os.path.join(REPO, args.out); os.makedirs(out, exist_ok=True)
    loras = [x.strip() for x in args.loras.split(",") if x.strip()]
    strengths = [float(x) for x in args.strengths.split(",")]
    cfgs = [float(x) for x in args.cfgs.split(",")]

    refs = {(s[0], d): reference(s[2], s[3], DIRS[d][2]) for s in SUBJECTS for d in DIRS}
    rows = []
    for lora in loras:
        for st in strengths:
            for cfg in cfgs:
                tag = f"{os.path.splitext(lora)[0]}_s{st:g}_c{cfg:g}"
                tot = []
                for name, fam, folder, stem, seed in SUBJECTS:
                    for d, (rd, phrase, _) in DIRS.items():
                        pos = (f"rd_style, rd_animal, rd_quadruped, {rd}, {phrase}, {fam}, {name}, "
                               f"single creature, full body, white background")
                        img = Image.open(io.BytesIO(_run(graph(pos, lora, st, cfg, seed, args.size))))
                        img.save(os.path.join(out, f"{tag}__{name}_{d}.png"))
                        g = measure(img.convert("RGB")); r = refs[(name, d)]
                        sc = score(g, r); tot.append(sc)
                        rows.append(dict(lora=lora, strength=st, cfg=cfg, subject=name, dir=d,
                                         **{k: round(v,4) for k,v in g.items()}, score=round(sc,1)))
                        print(f"  {tag:<34} {name}_{d}: blobs={g['blobs']} bg={g['bg']:.3f} "
                              f"fill={g['fill']:.3f} asp={g['aspect']:.2f} -> {sc:.0f}")
                print(f"== {tag}  MEAN SCORE {sum(tot)/len(tot):.1f} ==")

    with open(os.path.join(out, "results.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)

    print("\n=== SUMMARY (mean score per config) ===")
    agg = {}
    for r in rows: agg.setdefault((r["lora"], r["strength"], r["cfg"]), []).append(r["score"])
    for k, v in sorted(agg.items(), key=lambda kv: -sum(kv[1])/len(kv[1])):
        print(f"  {k[0]:<34} s={k[1]:<5} cfg={k[2]:<4} mean={sum(v)/len(v):5.1f}")
    print(f"\nwrote {os.path.join(args.out,'results.csv')}")

if __name__ == "__main__":
    main()
