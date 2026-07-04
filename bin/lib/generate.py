#!/usr/bin/env python3
"""bin/art generate — creature sprite generator.

Drives the ComfyUI box (SDXL i2i + ControlNet edge, east hero + IP-Adapter
anchored south/north) to paint directional sprites from a template set.

Naming (co-located-type convention, base = <id>.<rotation>.<part>):
  templates in  textures/templates/<from>/<template-id>.<dir>.template.png
  sprites  out  textures/sprites/<to-or-from>/<seed>.<dir>.0.sprite.png   (transparent RGBA)
  prompt   out  textures/templates/<from>/<seed>.prompt.txt               (reusable, --prompt)

Background is removed by a corner flood-fill (colour auto-detected from the four
corners, stops at the sharp outline transition). Prompts are used verbatim unless
--llm is passed, which expands the terse --positive/--negative via Claude (needs
ANTHROPIC_API_KEY or bin/keys/anthropic.env). Generic negatives are always added.

Env: COMFYUI_URL (default http://172.16.10.10:8188), ANTHROPIC_API_KEY.
"""
import argparse, json, os, sys, time, uuid, random, io, urllib.request, urllib.parse
from PIL import Image, ImageFilter
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")

# ---- proven recipe (see the sprite-gen memory / docs/sprite-gen-plan.md) ----
MODEL   = "sdxl/cyberrealisticXL_v80.safetensors"
CN_MODEL= "sdxl/diffusion_pytorch_model.safetensors"
STEPS, CFG, SAMPLER, SCHED = 26, 6.0, "dpmpp_2m", "karras"
DN, CN, CN_END = 0.80, 0.50, 0.60
IP_WEIGHT = 0.6
DIRS = ["e", "s", "n"]                       # e generated first = the IP hero
FACE = {
  "e": "lying down facing right, side profile",
  "s": "facing down toward the viewer, front view, head chest and face visible",
  "n": "facing up away from the viewer, back view, seen from behind, back of head body and tail toward the camera",
}
STYLE = ("oblique top-down view game creature sprite, {face}, flat cel shading, bold dark outline, "
         "hand-painted 2D game art, cartoon, on a plain solid white background")
GENERIC_NEG = "realistic, photo, photorealistic, watermark, text, signature, 3d render, blurry, jpeg artifacts"

# ---------------------------------------------------------------- http helpers
def _req(path, data=None, timeout=120):
    url = COMFY + path
    if data is not None:
        r = urllib.request.Request(url, data=json.dumps(data).encode(), headers={"Content-Type": "application/json"})
    else:
        r = urllib.request.Request(url)
    return json.load(urllib.request.urlopen(r, timeout=timeout))

def _fetch(im):
    q = urllib.parse.urlencode({"filename": im["filename"], "subfolder": im.get("subfolder", ""), "type": im["type"]})
    return urllib.request.urlopen(f"{COMFY}/view?{q}", timeout=120).read()

def _upload(pil, name):
    buf = io.BytesIO(); pil.convert("RGB").save(buf, "PNG"); b = "----rd"; body = io.BytesIO()
    def w(s): body.write(s.encode() if isinstance(s, str) else s)
    w(f'--{b}\r\nContent-Disposition: form-data; name="image"; filename="{name}"\r\nContent-Type: image/png\r\n\r\n'); w(buf.getvalue())
    w(f'\r\n--{b}\r\nContent-Disposition: form-data; name="overwrite"\r\n\r\ntrue\r\n--{b}--\r\n')
    req = urllib.request.Request(COMFY + "/upload/image", data=body.getvalue(), headers={"Content-Type": f"multipart/form-data; boundary={b}"})
    return json.load(urllib.request.urlopen(req, timeout=60))["name"]

def _run(graph, timeout=240):
    try:
        pid = _req("/prompt", {"prompt": graph, "client_id": uuid.uuid4().hex})["prompt_id"]
    except urllib.error.HTTPError as e:
        detail = ""
        try: detail = json.dumps(json.loads(e.read().decode()).get("node_errors", {}))[:600]
        except Exception: pass
        raise SystemExit(f"generate: ComfyUI rejected the workflow ({e.code}). {detail}")
    t0 = time.time()
    while time.time() - t0 < timeout:
        h = _req(f"/history/{pid}")
        if pid in h:
            outs = h[pid]["outputs"]
            if "9" in outs and outs["9"].get("images"):
                return _fetch(outs["9"]["images"][0])
            raise SystemExit(f"generate: run produced no image (status {h[pid].get('status')})")
        time.sleep(2.0)
    raise SystemExit("generate: timed out waiting for ComfyUI")

# ---------------------------------------------------------------- prompt expand (--llm)
def _anthropic_key():
    k = os.environ.get("ANTHROPIC_API_KEY")
    if k: return k.strip()
    f = os.path.join(REPO, "bin", "keys", "anthropic.env")
    if os.path.exists(f):
        for line in open(f):
            line = line.strip()
            if line.startswith("ANTHROPIC_API_KEY="):
                return line.split("=", 1)[1].strip().strip('"').strip("'")
    return None

POS_SYS = ("You expand a terse creature description into a rich comma-separated VISUAL prompt fragment for a "
           "flat 2D cartoon game sprite (RimWorld style). Describe ONLY appearance: species, fur/skin colour and "
           "pattern, distinctive features, eye colour. Do NOT mention pose, camera angle, background, or art style "
           "— those are added separately. Output ONLY the fragment, no preamble, no quotes.")
NEG_SYS = ("You expand a terse 'avoid' list into a concise comma-separated list of things a flat 2D cartoon game "
           "sprite generator should avoid, based on the user's input. Output ONLY the comma-separated list, no preamble.")

def _claude(key, system, user, model="claude-haiku-4-5-20251001", max_tokens=220):
    body = {"model": model, "max_tokens": max_tokens, "system": system,
            "messages": [{"role": "user", "content": user}]}
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(),
        headers={"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"})
    r = json.load(urllib.request.urlopen(req, timeout=60))
    return "".join(b.get("text", "") for b in r.get("content", [])).strip()

def _expand(key, text, kind):
    text = (text or "").strip()
    if not text: return text
    try:
        out = _claude(key, POS_SYS if kind == "pos" else NEG_SYS, text)
        return out or text
    except Exception as e:
        print(f"generate: prompt expansion failed ({e}); using verbatim", file=sys.stderr)
        return text

# ---------------------------------------------------------------- prompt files
def _resolve_prompt_ref(ref, from_path):
    if os.sep in ref or ref.endswith(".txt"):
        p = ref if os.path.isabs(ref) else os.path.join(REPO, ref)
    else:
        p = os.path.join(REPO, "textures", "templates", from_path, f"{ref}.prompt.txt")
    if not os.path.exists(p):
        raise SystemExit(f"generate: --prompt file not found: {p}")
    return p

def read_prompt_file(path):
    pos, neg, cur = [], [], None
    for line in open(path):
        s = line.rstrip("\n")
        if s.strip().startswith("#"): continue
        low = s.strip().lower()
        if low == "[positive]": cur = pos; continue
        if low == "[negative]": cur = neg; continue
        if cur is not None: cur.append(s)
    return "\n".join(pos).strip(), "\n".join(neg).strip()

def write_prompt_file(path, pos, neg, seed, from_path):
    with open(path, "w") as f:
        f.write(f"# art generate prompt — id {seed}, from {from_path}\n")
        f.write("# Reusable creature prompt. Style boilerplate + generic negatives are added at\n")
        f.write("# generation time, not stored here. Reuse with:  art generate ... --prompt " + str(seed) + "\n\n")
        f.write("[positive]\n" + (pos or "") + "\n\n")
        f.write("[negative]\n" + (neg or "") + "\n")

def resolve_prompts(args, from_path):
    if args.prompt:
        pth = _resolve_prompt_ref(args.prompt, from_path)
        fpos, fneg = read_prompt_file(pth)
        print(f"generate: reusing prompt {os.path.relpath(pth, REPO)} (no LLM)")
        return (args.positive.strip() or fpos), (args.negative.strip() or fneg)
    pos, neg = args.positive.strip(), args.negative.strip()
    if not args.llm:
        return pos, neg
    key = _anthropic_key()
    if not key:
        print("generate: --llm set but no ANTHROPIC_API_KEY (bin/keys/anthropic.env); using verbatim", file=sys.stderr)
        return pos, neg
    print("generate: expanding prompts via Claude (--llm)")
    return _expand(key, pos, "pos") or pos, _expand(key, neg, "neg") or neg

# ---------------------------------------------------------------- templates / bg
def load_template(from_path, tid, d):
    p = os.path.join(REPO, "textures", "templates", from_path, f"{tid}.{d}.template.png")
    if not os.path.exists(p):
        raise SystemExit(f"generate: template not found: {p}")
    t = Image.open(p).convert("RGBA").resize((512, 512), Image.LANCZOS)
    f = Image.new("RGBA", (512, 512), (255, 255, 255, 255)); f.alpha_composite(t)
    return f.convert("RGB")

def load_hero_from_disk(out_dir, seed):
    """An already-generated east sprite (out_dir/<seed>.e.0.sprite.png), flattened
    onto white, for use as the IP anchor when east isn't regenerated this run."""
    p = os.path.join(out_dir, f"{seed}.e.0.sprite.png")
    if not os.path.exists(p):
        return None
    im = Image.open(p).convert("RGBA")
    w = Image.new("RGBA", im.size, (255, 255, 255, 255)); w.alpha_composite(im)
    return w.convert("RGB")

def edge_map(rgb):
    e = (np.array(rgb.convert("L").filter(ImageFilter.FIND_EDGES)) > 30).astype(np.uint8) * 255
    return Image.fromarray(e).convert("RGB")

def remove_bg_floodfill(img_rgb, thresh=60.0, feather=0.8, max_iter=4096):
    """Corner-seeded flood fill -> transparent background. Auto-detects the bg
    colour from the four corners, floods inward, STOPS at the first sharp colour
    transition (e.g. white->black outline; also white->green, magenta->black).
    Interior same-colour regions sealed by the outline stay opaque. Colour-agnostic."""
    rgb = np.asarray(img_rgb.convert("RGB")).astype(np.float32)
    H, W, _ = rgb.shape; cs = 6
    corners = np.vstack([rgb[:cs, :cs].reshape(-1, 3), rgb[:cs, -cs:].reshape(-1, 3),
                         rgb[-cs:, :cs].reshape(-1, 3), rgb[-cs:, -cs:].reshape(-1, 3)])
    bg = np.median(corners, axis=0)
    isbg = np.sqrt(((rgb - bg) ** 2).sum(2)) < thresh
    region = np.zeros((H, W), bool)
    region[0, :] |= isbg[0, :]; region[-1, :] |= isbg[-1, :]
    region[:, 0] |= isbg[:, 0]; region[:, -1] |= isbg[:, -1]
    for _ in range(max_iter):
        g = region.copy()
        g[1:, :] |= region[:-1, :]; g[:-1, :] |= region[1:, :]
        g[:, 1:] |= region[:, :-1]; g[:, :-1] |= region[:, 1:]
        g &= isbg
        if int(g.sum()) == int(region.sum()): break
        region = g
    alpha = Image.fromarray(np.where(region, 0, 255).astype(np.uint8), "L")
    if feather > 0:
        alpha = alpha.filter(ImageFilter.GaussianBlur(feather))
    return Image.fromarray(np.dstack([rgb.astype(np.uint8), np.asarray(alpha)]), "RGBA")

# ---------------------------------------------------------------- workflow
def _tail(pos, neg, ref_name, edge_name, model_ref, seed):
    return {
     "6":{"class_type":"CLIPTextEncode","inputs":{"text":pos,"clip":["4",1]}},
     "7":{"class_type":"CLIPTextEncode","inputs":{"text":neg,"clip":["4",1]}},
     "20":{"class_type":"LoadImage","inputs":{"image":ref_name}},
     "21":{"class_type":"VAEEncode","inputs":{"pixels":["20",0],"vae":["4",2]}},
     "30":{"class_type":"LoadImage","inputs":{"image":edge_name}},
     "31":{"class_type":"ControlNetLoader","inputs":{"control_net_name":CN_MODEL}},
     "32":{"class_type":"ControlNetApplyAdvanced","inputs":{"positive":["6",0],"negative":["7",0],"control_net":["31",0],"image":["30",0],"strength":CN,"start_percent":0.0,"end_percent":CN_END}},
     "3":{"class_type":"KSampler","inputs":{"seed":seed,"steps":STEPS,"cfg":CFG,"sampler_name":SAMPLER,"scheduler":SCHED,"denoise":DN,"model":model_ref,"positive":["32",0],"negative":["32",1],"latent_image":["21",0]}},
     "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["4",2]}},
     "9":{"class_type":"SaveImage","inputs":{"filename_prefix":"artgen","images":["8",0]}}}

def graph_hero(pos, neg, ref_name, edge_name, seed):
    g = {"4":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":MODEL}}}
    g.update(_tail(pos, neg, ref_name, edge_name, ["4",0], seed)); return g

def graph_ip(pos, neg, ref_name, edge_name, hero_name, seed):
    g = {"4":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":MODEL}},
     "40":{"class_type":"IPAdapterUnifiedLoader","inputs":{"model":["4",0],"preset":"PLUS (high strength)"}},
     "41":{"class_type":"LoadImage","inputs":{"image":hero_name}},
     "42":{"class_type":"IPAdapter","inputs":{"model":["40",0],"ipadapter":["40",1],"image":["41",0],"weight":IP_WEIGHT,"weight_type":"style transfer","start_at":0.0,"end_at":1.0}}}
    g.update(_tail(pos, neg, ref_name, edge_name, ["42",0], seed)); return g

# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser(prog="art generate", description="Generate directional creature sprites from a template set.")
    ap.add_argument("--from", dest="from_path", required=True, help="template path under textures/templates (e.g. pawns/wolf)")
    ap.add_argument("--to", dest="to_path", default=None, help="output path under textures/sprites (default: same as --from)")
    ap.add_argument("--positive", default="", help="short creature description")
    ap.add_argument("--negative", default="", help="short 'avoid' description")
    ap.add_argument("--prompt", default=None, help="reuse a saved prompt: an id (templates/<from>/<id>.prompt.txt) or a path")
    ap.add_argument("--llm", action="store_true", help="expand --positive/--negative via Claude (spends API tokens; off by default)")
    ap.add_argument("--seed", type=int, default=None, help="seed; also the output id. random if omitted")
    ap.add_argument("--template-id", default="1", help="template id within the folder (default 1)")
    ap.add_argument("--dirs", default="e,s,n", help="directions to generate (default e,s,n; e is the hero)")
    ap.add_argument("--dir", default=None, help="generate a single direction (overrides --dirs), e.g. --dir s")
    ap.add_argument("--bg-thresh", type=float, default=60.0, help="corner-flood key tolerance (colour distance; default 60)")
    ap.add_argument("--keep-bg", action="store_true", help="skip background removal; save the raw sprite on its bg")
    args = ap.parse_args()

    seed = args.seed if args.seed is not None else random.randint(1, 2**31 - 1)
    dirs = [args.dir.strip()] if args.dir else [d.strip() for d in args.dirs.split(",") if d.strip()]
    if "e" in dirs: dirs = ["e"] + [d for d in dirs if d != "e"]   # hero first
    from_path = args.from_path.strip("/")
    out_path = (args.to_path or from_path).strip("/")

    pos, neg_user = resolve_prompts(args, from_path)
    neg = ", ".join(x for x in [neg_user, GENERIC_NEG] if x)

    # persist the reusable prompt into the templates dir (id-keyed)
    tpl_dir = os.path.join(REPO, "textures", "templates", from_path)
    os.makedirs(tpl_dir, exist_ok=True)
    prompt_out = os.path.join(tpl_dir, f"{seed}.prompt.txt")
    write_prompt_file(prompt_out, pos, neg_user, seed, from_path)

    out_dir = os.path.join(REPO, "textures", "sprites", out_path)
    os.makedirs(out_dir, exist_ok=True)
    print(f"generate: from={from_path} to={out_path} seed={seed} dirs={dirs}"
          + ("" if args.keep_bg else f" bg-key(thresh {args.bg_thresh:g})"))
    print(f"  positive -> {pos}")
    print(f"  negative -> {neg}")
    print(f"  wrote {os.path.relpath(prompt_out, REPO)}")

    hero_name = None
    if "e" not in dirs:   # regenerating only s/n — anchor to the existing east sprite if present
        hero_img = load_hero_from_disk(out_dir, seed)
        if hero_img is not None:
            hero_name = _upload(hero_img, f"artgen_{seed}_hero.png")
            print(f"  IP anchor: existing {seed}.e.0.sprite.png")
        else:
            print(f"generate: no existing east sprite for seed {seed}; {dirs} generate without IP anchor", file=sys.stderr)
    for d in dirs:
        if d not in FACE:
            print(f"generate: skipping unknown direction '{d}'", file=sys.stderr); continue
        tpl = load_template(from_path, args.template_id, d)
        ref_name = _upload(tpl, f"artgen_{seed}_{d}_ref.png")
        edge_name = _upload(edge_map(tpl), f"artgen_{seed}_{d}_edge.png")
        full_pos = f"{pos}, {STYLE.format(face=FACE[d])}"
        if d == "e" or hero_name is None:
            raw = _run(graph_hero(full_pos, neg, ref_name, edge_name, seed))
        else:
            raw = _run(graph_ip(full_pos, neg, ref_name, edge_name, hero_name, seed))
        img = Image.open(io.BytesIO(raw)).convert("RGB")
        if d == "e":
            hero_name = _upload(img, f"artgen_{seed}_hero.png")   # east (on white) becomes the IP anchor
        sprite = img if args.keep_bg else remove_bg_floodfill(img, thresh=args.bg_thresh)
        out = os.path.join(out_dir, f"{seed}.{d}.0.sprite.png")
        sprite.save(out)
        print(f"  wrote {os.path.relpath(out, REPO)}")
    print(f"generate: done (id {seed})")

if __name__ == "__main__":
    main()
