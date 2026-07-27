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

def _hull_area(small):
    """Convex-hull area of a boolean mask (monotone chain; no scipy).

    Why this and not bbox `solidity`: the bounding box is a poor stand-in for "is this a shape or a
    blob". A featureless blob nearly fills its box AND its hull; an animal with legs, a snout or
    horns is deeply CONCAVE, so it fills its hull far less. Measuring against the hull therefore
    separates structure from mass, where bbox area conflates them — the defect that let a blobby
    anteater pass and rejected a horned oryx (issues.md I3)."""
    ys, xs = np.where(small)
    if len(xs) < 3: return 0.0
    pts = sorted(set(zip(xs.tolist(), ys.tolist())))
    if len(pts) < 3: return 0.0
    def half(ps):
        out = []
        for p in ps:
            while len(out) >= 2:
                (ax, ay), (bx, by) = out[-2], out[-1]
                if (bx-ax)*(p[1]-ay) - (by-ay)*(p[0]-ax) > 0: break
                out.pop()
            out.append(p)
        return out
    hull = half(pts)[:-1] + half(pts[::-1])[:-1]
    if len(hull) < 3: return 0.0
    a = 0.0
    for i in range(len(hull)):
        x1, y1 = hull[i]; x2, y2 = hull[(i+1) % len(hull)]
        a += x1*y2 - x2*y1
    return abs(a) / 2.0

def iou_control(img, control):
    """IoU between the output silhouette and the CONTROL silhouette it was given.

    Answers "did the output follow the shape it was handed?" — it does NOT answer "is the output a
    good animal", because a faithfully-followed BAD control still scores high. Use it to detect
    drift off the control, not as a quality verdict (issues.md I5)."""
    a = _filled(_mask(img), 128)
    b = _filled(_mask(control), 128)
    inter = float((a & b).sum()); union = float((a | b).sum())
    return inter / union if union else 0.0

def _norm_silhouette(img, grid=128):
    """Filled silhouette, cropped to its own bbox and rescaled into a fixed grid.

    Normalising position AND size before overlap is the whole point: we are asking "is this the same
    SHAPE" — is a lying wolf lying — not "is it in the same place at the same scale". Without the
    crop-and-fit, every sprite would score low for offsets and scale differences that the pipeline
    already handles elsewhere (bbox fill is a separate metric)."""
    m = _filled(_mask(img), grid)
    if not m.any(): return m
    ys, xs = np.where(m)
    sub = m[ys.min():ys.max()+1, xs.min():xs.max()+1]
    return np.asarray(Image.fromarray((sub*255).astype(np.uint8)).resize((grid, grid), Image.BILINEAR)) > 96

def iou_ref(img, ref_img):
    """IoU of the generated silhouette against the REAL corpus sprite for that species+direction.

    Distinct from iou_control (forks F2): that compared against the CONTROL image the generator was
    handed, so a wrong control faithfully obeyed scored HIGH. This compares against ground truth, so
    it can see what no bounding-box statistic can — a sitting wolf overlaps a lying wolf poorly, and
    a framed bust overlaps a full body poorly. Only defined for corpus species."""
    a = _norm_silhouette(img); b = _norm_silhouette(ref_img)
    u = float((a | b).sum())
    return float((a & b).sum()) / u if u else 0.0

def reference_image(folder, stem, direction):
    """The real corpus sprite composited on white, for iou_ref. None if absent."""
    p = os.path.join(SRC, folder, f"{stem}_{direction}.png")
    if not os.path.exists(p):
        d = os.path.join(SRC, folder)
        cands = sorted(os.listdir(d)) if os.path.isdir(d) else []
        for c in cands:
            if c.lower().endswith(f"_{direction}.png"): p = os.path.join(d, c); break
    if not os.path.exists(p): return None
    im = Image.open(p).convert("RGBA")
    bg = Image.new("RGBA", im.size, (255,255,255,255)); bg.alpha_composite(im)
    return bg.convert("RGB")

def measure(img, control=None):
    m = _mask(img); H, W = m.shape
    if m.sum() < 50:
        return dict(blobs=0, bg=1.0, bg_uni=1.0, white=1.0, fill=0.0, aspect=0.0, solidity=0.0,
                    hull_solidity=1.0, iou_control="")
    ys, xs = np.where(m)
    y0, y1, x0, x1 = ys.min(), ys.max(), xs.min(), xs.max()
    bw, bh = (x1-x0+1), (y1-y0+1)
    border = np.concatenate([m[0,:], m[-1,:], m[:,0], m[:,-1]])
    a = np.asarray(img.convert("RGB")).astype(np.float32)
    white = float(max(0.0, 1.0 - np.sqrt(((_bg_color(a) - 255.0)**2).sum()) / 120.0))
    # bg_uni: is the PLATE keyable, independent of how much of the border the subject occupies.
    # `bg` alone conflates two unrelated things — scenery bleed (fatal) and a large sprite simply
    # touching the frame edge (fine, and normal for a well-filled sprite). It rejected a
    # hand-approved bear-north purely for filling the frame. Uniformity of the NON-subject border
    # pixels is the thing that actually decides whether remove_bg_floodfill() can key it out.
    bpx = np.concatenate([a[0,:,:], a[-1,:,:], a[:,0,:], a[:,-1,:]])
    free = bpx[~border]
    bg_uni = 1.0 if free.shape[0] < 16 else float(max(0.0, 1.0 - float(free.std(axis=0).mean()) / 24.0))
    small = _filled(m)                                  # holes filled -> solid silhouette
    g = small.shape[0]
    sy, sx = np.where(small)
    sol = float(small.sum()) / max(1.0, float((sx.max()-sx.min()+1) * (sy.max()-sy.min()+1))) if small.any() else 0.0
    return dict(
        blobs=_components(small),
        bg=float(1.0 - border.mean()),                 # 1.0 = subject does not touch the frame edge
        bg_uni=round(bg_uni, 3),                       # 1.0 = the plate is uniform -> keyable (the real gate)
        white=round(white,3),                          # how close that plate is to pure white
        fill=float(bw*bh) / float(W*H),
        aspect=float(bw)/float(bh),
        solidity=sol,
        hull_solidity=round(float(small.sum()) / ha, 3) if (ha := _hull_area(small)) > 0 else 1.0,
        iou_control=round(iou_control(img, control), 3) if control is not None else "",
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

def d_aspect_signed(g, r):
    """Signed proportion error, in percent: NEGATIVE means the sprite is more COMPACT than the real
    one (upright/sitting — a broken pose convention), POSITIVE means longer (a proportion wobble).
    The unsigned version scored those identically and called run-4's east pose drift a tie
    (issues I1)."""
    if not r: return None
    return 100.0 * (g["aspect"] - r["aspect"]) / max(r["aspect"], 1e-3)

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

# ---------------------------------------------------------------- pipeline sweep (dn/cn)
def sweep_pipeline(args):
    """Sweep denoise x ControlNet through the REAL production pipeline (generate.py's own
    graph builders, so this measures what `bin/art generate` actually does).

    Scoring is deliberately NOT the txt2img composite score: that one rewards matching the
    reference's fill/aspect, so it would rank dn=0 (a perfect template copy) as best — the
    over-prescriptive failure we are trying to escape. Instead:
        VALIDITY is a constraint  — 1 blob, clean keyable bg, aspect close to the template's
        VARIETY  is the objective — mean pairwise difference between seeds
    Best config = most variety among the configs that stay valid.
    """
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import generate as G
    G.LORA, G.LORA_STRENGTH = args.lora, args.lora_strength
    if args.style: G.STYLE = args.style

    kind, d = args.kind.strip("/"), args.dir
    # --control none is the TEMPLATE-FREE path: no template art is read at all, no ControlNet
    # nodes are built, and the geometry gate is judged against the REAL corpus sprite (--ref)
    # instead of a template. dn/cn/cn_end are inert here — txt2img has no i2i latent and no
    # control image — so the sweep degenerates to one config, which the caller should expect.
    tpl = None if args.control == "none" else G.load_template(kind, d, args.part, "0")
    if tpl is not None:
        ref_name  = G._upload(tpl, f"sweep_{d}_ref.png")
        edge_name = G._upload(G.edge_map(tpl, args.edge_thresh), f"sweep_{d}_edge.png")
        gate = measure(tpl); gate_src = "template"
        base_arr, base_size = np.asarray(tpl, dtype=float), tpl.size
    else:
        ref_name = edge_name = None
        gate, gate_src = None, "none"
        if args.ref:
            folder, _, stem = args.ref.partition(":")
            gate = reference(folder, stem or folder, DIRS[d][2])
            gate_src = f"corpus:{folder}"
        base_arr, base_size = None, (args.size, args.size)
    pos = f"{args.positive}, {G.STYLE.format(face=G.FACE[d])}"
    neg = G.GENERIC_NEG
    # --combos gives an explicit config list "dn:cn:cn_end,..." (cn_end optional, defaults to
    # generate.py's CN_END). Otherwise sweep the dns x cns grid at the default cn_end.
    if args.combos:
        combos = []
        for c in args.combos.split(","):
            p = [float(x) for x in c.split(":")]
            combos.append((p[0], p[1], p[2] if len(p) > 2 else G.CN_END))
    else:
        combos = [(dn, cn, G.CN_END) for dn in [float(x) for x in args.dns.split(",")]
                                     for cn in [float(x) for x in args.cns.split(",")]]
    if tpl is None: combos = combos[:1]          # dn/cn are inert with no control image
    out = os.path.join(REPO, args.out); os.makedirs(out, exist_ok=True)
    ga = f"{gate['aspect']:.2f}" if gate else "n/a"
    print(f"control={args.control} gate={gate_src} aspect={ga}  ({len(combos)} configs x {args.seeds} seeds)")

    rows = []; sheet_cells = []
    if True:
        for (dn, cn, cn_end) in combos:
            G.DN, G.CN, G.CN_END = dn, cn, cn_end
            imgs = []
            for i in range(args.seeds):
                if tpl is None:                   # template-free: plain txt2img + LoRA
                    raw = _run(graph(pos, args.lora, args.lora_strength, args.cfg_tf, 9000 + i, size=args.size))
                else:
                    raw = G._run(G.graph_hero(pos, neg, ref_name, edge_name, 9000 + i))
                im = Image.open(io.BytesIO(raw)).convert("RGB")
                fp = os.path.join(out, f"dn{dn:g}_cn{cn:g}_e{cn_end:g}_s{9000+i}.png")
                im.save(fp); imgs.append(im)
                sheet_cells.append((f"dn{dn:g}/cn{cn:g}/e{cn_end:g}", f"seed{9000+i}", fp))
            ms = [measure(i) for i in imgs]
            arrs = [np.asarray(i.resize(base_size), dtype=float) for i in imgs]
            var = float(np.mean([np.abs(arrs[a]-arrs[b]).mean()
                        for a in range(len(arrs)) for b in range(a+1, len(arrs))])) if len(arrs) > 1 else 0.0
            div = float(np.mean([np.abs(a - base_arr).mean() for a in arrs])) if base_arr is not None else 0.0
            ok = sum(1 for m in ms if m["blobs"] == 1 and m["bg"] >= 0.97
                     and (gate is None or
                          abs(m["aspect"]-gate["aspect"])/max(gate["aspect"], 1e-3) <= 0.25))
            rows.append(dict(dn=dn, cn=cn, cn_end=cn_end, valid=ok, n=len(ms), variety=round(var,1),
                             divergence=round(div,1),
                             aspect=round(float(np.mean([m["aspect"] for m in ms])), 2)))
            print(f"  dn={dn:.2f} cn={cn:.2f} cn_end={cn_end:.2f}  valid={ok}/{len(ms)}  variety={var:5.1f}  div={div:5.1f}  asp={rows[-1]['aspect']:.2f}")

    with open(os.path.join(out, "sweep.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
    good = [r for r in rows if r["valid"] == r["n"]]
    print(f"\n=== fully-valid configs ranked by VARIETY ({len(good)}/{len(rows)} passed geometry) ===")
    for r in sorted(good, key=lambda r: -r["variety"])[:8]:
        print(f"  dn={r['dn']:.2f} cn={r['cn']:.2f} e={r['cn_end']:.2f}  variety={r['variety']:5.1f}  divergence={r['divergence']:5.1f}  aspect={r['aspect']:.2f}")
    if not good: print("  (none fully valid — loosen the validity gate or tighten cn)")
    bad = [r for r in rows if r["valid"] < r["n"]]
    if bad:
        print("\n  configs that FAILED geometry (silhouette lost):")
        for r in bad: print(f"    dn={r['dn']:.2f} cn={r['cn']:.2f}  valid={r['valid']}/{r['n']}  aspect={r['aspect']:.2f}")
    if sheet_cells:
        sp = contact_sheet(sheet_cells, os.path.join(out, "sheet.png"))
        print(f"wrote {os.path.relpath(sp, REPO)}   <- LOOK AT THIS before trusting the table")
    print(f"wrote {os.path.join(args.out,'sweep.csv')}")

# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser(prog="art lora-eval")
    ap.add_argument("--loras", default="", help="comma-separated LoRA filenames as ComfyUI sees them")
    ap.add_argument("--strengths", default="0.85", help="comma-separated LoRA strengths (default 0.85)")
    ap.add_argument("--cfgs", default="6.0", help="comma-separated cfg values (default 6.0)")
    ap.add_argument("--out", default=".staging/lora-eval", help="output dir for images + results.csv")
    ap.add_argument("--size", type=int, default=768)
    # --- pipeline (dn/cn) sweep mode: runs the real template+ControlNet pipeline ---
    ap.add_argument("--pipeline", action="store_true", help="sweep dn x cn through the real bin/art generate pipeline instead of txt2img")
    ap.add_argument("--kind", default="pawn/animal/wolf", help="template kind path under textures/ (--pipeline)")
    ap.add_argument("--dir", default="e", help="direction to sweep (--pipeline; default e)")
    ap.add_argument("--part", default="0")
    ap.add_argument("--positive", default="grey timber wolf, yellow eyes", help="creature description (--pipeline)")
    ap.add_argument("--dns", default="0.70,0.80,0.90,1.00", help="denoise values (--pipeline)")
    ap.add_argument("--cns", default="0.20,0.35,0.50,0.65", help="ControlNet strengths (--pipeline)")
    ap.add_argument("--seeds", type=int, default=3, help="seeds per config; >=2 needed to measure variety")
    ap.add_argument("--control", default="template", choices=["template","none"],
                    help="silhouette source: template art (default) or none = txt2img+LoRA, no ControlNet")
    ap.add_argument("--ref", default=None, help="corpus reference for the geometry gate under --control none, e.g. Wolf_Timber[:stem]")
    ap.add_argument("--cfg-tf", dest="cfg_tf", type=float, default=6.0, help="cfg for the template-free txt2img path")
    ap.add_argument("--combos", default=None, help="explicit configs 'dn:cn:cn_end,...' (cn_end optional) — overrides --dns/--cns")
    ap.add_argument("--edge-thresh", type=int, default=30)
    ap.add_argument("--lora", default=None, help="single LoRA for --pipeline mode")
    ap.add_argument("--lora-strength", type=float, default=0.85)
    ap.add_argument("--style", default=None, help="STYLE override (use the LoRA's trained tags)")
    args = ap.parse_args()

    if args.pipeline:
        if not args.lora: args.lora = args.loras.split(",")[0].strip()
        return sweep_pipeline(args)

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

# ---------------------------------------------------------------- contact sheets
def contact_sheet(cells, out_path, cols=None, cell=200, header=22, pad=3):
    """Tile labelled images into one reviewable PNG.

    Exists because the numbers have now been overturned by a hand-made montage four times
    (framed busts passing 10/18, the east pose drift scored a tie, the anteater rated best-as-worst).
    The visual check kept being the thing that found the truth, so it stops depending on someone
    remembering to write a montage script and becomes part of every eval run.

    `cells` is [(col_label, row_label, path_or_image), ...]; column order follows first appearance.
    Missing/unreadable entries leave a blank cell rather than aborting a long run."""
    from PIL import ImageDraw
    order, rows = [], []
    for c, r, _ in cells:
        if c not in order: order.append(c)
        if r not in rows: rows.append(r)
    if cols: order = [c for c in cols if c in order] + [c for c in order if c not in cols]
    W = Image.new("RGB", (len(order)*(cell+pad)+pad, len(rows)*(cell+pad)+pad+header), (70, 70, 70))
    d = ImageDraw.Draw(W)
    for i, c in enumerate(order):
        d.text((pad + i*(cell+pad) + 4, 6), str(c)[:28], fill=(255, 255, 255))
    for c, r, src in cells:
        try:
            im = src if isinstance(src, Image.Image) else Image.open(src)
            if im.mode in ("RGBA", "LA", "P"):     # composite on WHITE, never convert straight to
                im = im.convert("RGBA")           # RGB — that renders transparency BLACK and makes
                w = Image.new("RGBA", im.size, (255, 255, 255, 255))   # a correct sprite look broken
                w.alpha_composite(im); im = w
            im = im.convert("RGB").resize((cell, cell), Image.LANCZOS)
        except Exception:
            continue
        x = pad + order.index(c)*(cell+pad); y = header + pad + rows.index(r)*(cell+pad)
        W.paste(im, (x, y))
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    W.save(out_path)
    return out_path
