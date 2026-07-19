#!/usr/bin/env python3
"""bin/art generate — creature sprite generator.

Drives the ComfyUI box (SDXL i2i + ControlNet edge, east hero + IP-Adapter
anchored south/north) to paint directional sprites from a template set.

Naming — folder-per-variant layout (docs/texture-paths.md), each map a <map>.png in a
<id>.<dir>.<layer>/<variant>/ leaf:
  templates in  textures/<from>/<template-id>.<dir>.<layer>/<template-variant>/template.png
  sprites  out  textures/<to-or-from>/<template-id>.<dir>.<layer>/<seed>/sprite.png  (SEED = variant; transparent RGBA)
  prompt   out  textures/<from>/<seed>.prompt.txt                                    (reusable, --prompt; loose sidecar)
The template supplies <id>.<dir>.<layer>; the SEED becomes the output <variant>, so many
seeds share one <id>.<dir>.<layer> — i.e. multiple sprite variants/control-maps per object.

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
sys.path.insert(0, HERE)
import texpath
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")

# ---- proven recipe (see the sprite-gen memory / docs/sprite-gen-plan.md) ----
MODEL   = "sdxl/cyberrealisticXL_v80.safetensors"
CN_MODEL= "sdxl/diffusion_pytorch_model.safetensors"
STEPS, CFG, SAMPLER, SCHED = 26, 6.0, "dpmpp_2m", "karras"
DN, CN, CN_END = 0.70, 0.50, 0.90
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

POS_SYS = ("You refine a description for a 2D cartoon game sprite (RimWorld / Prison Architect "
           "style: bold simple shapes, soft dimensional shading, clear colour regions, readable when small). PRESERVE "
           "every feature the user stated (species, colours, markings, eyes) — never drop or contradict them — "
           "then ADD complementary visual detail that suits this style: distinct colour regions, simple bold "
           "markings, a clear silhouette. Do NOT add pose, camera angle, background, or render-style keywords "
           "(those are appended separately by the pipeline). Output ONLY a comma-separated appearance fragment, "
           "no preamble, no quotes.")
NEG_SYS = ("You refine a terse 'avoid' list for a flat 2D cartoon game sprite generator. PRESERVE the user's "
           "stated items and add a few closely-related things worth avoiding for this flat cartoon style. Output "
           "ONLY a comma-separated list, no preamble, no quotes.")

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
        p = os.path.join(REPO, "textures", from_path, f"{ref}.prompt.txt")
    if not os.path.exists(p):
        raise SystemExit(f"generate: --prompt file not found: {p}")
    return p

def read_prompt_file(path):
    pos, neg, cur = [], [], None
    for line in open(path):
        s = line.rstrip("\n")
        st = s.strip()
        if st == "#" or st.startswith("# "): continue   # comment = '#' + space (so '#FF00FF' hex is content)
        low = st.lower()
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
    # Base prompt: explicit --positive/--negative, else the --prompt file (explicit
    # args override the file). Then --llm (if set) expands whatever the base is —
    # so --prompt just PRE-FILLS pos/neg and the LLM takes over from there.
    pos, neg = args.positive.strip(), args.negative.strip()
    if args.prompt:
        pth = _resolve_prompt_ref(args.prompt, from_path)
        fpos, fneg = read_prompt_file(pth)
        pos, neg = (pos or fpos), (neg or fneg)
        print(f"generate: loaded prompt {os.path.relpath(pth, REPO)}"
              + (" (+ --llm expand)" if args.llm else " (verbatim, no LLM)"))
    if not args.llm:
        return pos, neg
    key = _anthropic_key()
    if not key:
        print("generate: --llm set but no ANTHROPIC_API_KEY (bin/keys/anthropic.env); using verbatim", file=sys.stderr)
        return pos, neg
    print("generate: expanding prompts via Claude (--llm)")
    return _expand(key, pos, "pos") or pos, _expand(key, neg, "neg") or neg

# ---------------------------------------------------------------- naming / templates / bg
# Leaf layout (docs/components/dev/textures/design/texture-layout/): each directional map is
# <map>.<dir>.<part>.png inside a single <variant>/ leaf (no <id>/<subkind>). These return the
# leaf-relative path to a member. `part` is the old `layer` segment (renamed, moved into the file).
def template_name(tid, d, part, tvar):   # the pose reference (map=template); tid dropped
    return os.path.join(texpath.variant_leaf(tvar), texpath.map_name("template", d, part))
def sprite_name(tid, d, part, seed):     # generated sprite: SEED is the variant (map=sprite)
    return os.path.join(texpath.variant_leaf(seed), texpath.map_name("sprite", d, part))

def load_template(from_path, tid, d, layer, tvar):
    p = os.path.join(REPO, "textures", from_path, template_name(tid, d, layer, tvar))
    if not os.path.exists(p):
        raise SystemExit(f"generate: template not found: {p}")
    t = Image.open(p).convert("RGBA").resize((512, 512), Image.LANCZOS)
    f = Image.new("RGBA", (512, 512), (255, 255, 255, 255)); f.alpha_composite(t)
    return f.convert("RGB")

def load_hero_from_disk(out_dir, tid, layer, seed):
    """An already-generated east sprite (<id>.e.<layer>/<seed>/sprite.png), flattened
    onto white, for use as the IP anchor when east isn't regenerated this run."""
    p = os.path.join(out_dir, sprite_name(tid, "e", layer, seed))
    if not os.path.exists(p):
        return None
    im = Image.open(p).convert("RGBA")
    w = Image.new("RGBA", im.size, (255, 255, 255, 255)); w.alpha_composite(im)
    return w.convert("RGB")

def edge_map(rgb, thresh=30):
    """Binary edge map fed to ControlNet. Higher thresh keeps only the strong outer
    silhouette and drops interior/AA noise — a cleaner control signal that lets you
    hold high CN without the model baking every speck of fur texture into a hard line."""
    e = (np.array(rgb.convert("L").filter(ImageFilter.FIND_EDGES)) > thresh).astype(np.uint8) * 255
    return Image.fromarray(e).convert("RGB")

def remove_bg_floodfill(img_rgb, thresh=60.0, choke=2, feather=0.8, max_iter=4096):
    """Corner-seeded flood fill -> transparent background. Auto-detects the bg
    colour from the four corners, floods inward, STOPS at the first sharp colour
    transition (e.g. white->black outline; also white->green, magenta->black).
    Interior same-colour regions sealed by the outline stay opaque. Colour-agnostic.

    choke: erode the alpha inward by N px, seating the cut against the black outline
    so the anti-aliased fringe the flood leaves behind (the light 'halo' between bg
    and outline) is removed. Mirrors the magenta matte choke; colour-agnostic, so it
    also clears a green/magenta fringe. 0 = no choke."""
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
    for _ in range(int(choke)):                          # choke: kill the AA fringe / halo
        alpha = alpha.filter(ImageFilter.MinFilter(3))
    if feather > 0:
        alpha = alpha.filter(ImageFilter.GaussianBlur(feather))
    return Image.fromarray(np.dstack([rgb.astype(np.uint8), np.asarray(alpha)]), "RGBA")

def make_symmetric(img, axis):
    """Force mirror symmetry by averaging the sprite with its own flip. axis='h'
    mirrors left<->right (horizontal symmetry, a vertical mirror line — the useful
    one for front/back N/S views); axis='v' mirrors top<->bottom. Blends in
    premultiplied alpha so transparent (bg-coloured) RGB can't bleed into the
    averaged edges and re-create a halo."""
    flip = Image.FLIP_LEFT_RIGHT if axis == "h" else Image.FLIP_TOP_BOTTOM
    mirror = img.transpose(flip)
    if img.mode != "RGBA":
        a = np.asarray(img.convert("RGB")).astype(np.float32)
        b = np.asarray(mirror.convert("RGB")).astype(np.float32)
        return Image.fromarray(((a + b) / 2.0).astype(np.uint8), "RGB")
    a = np.asarray(img).astype(np.float32); b = np.asarray(mirror).astype(np.float32)
    pre = (a[..., :3] * (a[..., 3:4] / 255.0) + b[..., :3] * (b[..., 3:4] / 255.0)) / 2.0
    oa = (a[..., 3:4] + b[..., 3:4]) / 2.0               # averaged alpha
    rgb = np.where(oa > 1e-3, pre / np.clip(oa / 255.0, 1e-3, None), 0.0)
    return Image.fromarray(np.dstack([np.clip(rgb, 0, 255), oa[..., 0]]).astype(np.uint8), "RGBA")

def resize_sprite(img, size):
    """Scale the final sprite to size×size. For RGBA, resize with PREMULTIPLIED
    alpha so the transparent pixels' (bg-coloured) RGB can't bleed into the edges
    and re-create the halo — then unpremultiply. Generation stays at 512 (quality);
    only the saved file shrinks."""
    if size <= 0 or (img.width == size and img.height == size):
        return img
    if img.mode != "RGBA":
        return img.resize((size, size), Image.LANCZOS)
    a = np.asarray(img).astype(np.float32); al = a[..., 3:4] / 255.0
    pre = Image.fromarray(np.dstack([a[..., :3] * al, a[..., 3]]).astype(np.uint8), "RGBA")
    pre = pre.resize((size, size), Image.LANCZOS)
    r = np.asarray(pre).astype(np.float32); oa = r[..., 3:4] / 255.0
    rgb = np.where(oa > 1e-3, r[..., :3] / np.clip(oa, 1e-3, None), 0.0)
    return Image.fromarray(np.dstack([np.clip(rgb, 0, 255), r[..., 3]]).astype(np.uint8), "RGBA")

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
    global DN, CN, CN_END, CFG   # --dn/--cn/--cn-end/--cfg override the module defaults
    ap = argparse.ArgumentParser(prog="art generate", description="Generate directional creature sprites from a template set.")
    ap.add_argument("--from", dest="from_path", required=True, help="kind path under textures/ holding the template leaves, any depth (e.g. pawn/animal/wolf/default)")
    ap.add_argument("--to", dest="to_path", default=None, help="output kind path under textures/ (default: same as --from)")
    ap.add_argument("--positive", default="", help="short creature description")
    ap.add_argument("--negative", default="", help="short 'avoid' description")
    ap.add_argument("--prompt", default=None, help="reuse a saved prompt: an id (textures/<from>/<id>.prompt.txt) or a path")
    ap.add_argument("--llm", action="store_true", help="expand --positive/--negative via Claude (spends API tokens; off by default)")
    ap.add_argument("--seed", type=int, default=None, help="seed; also the output id. random if omitted")
    ap.add_argument("--template-id", default="1", help="template <id> within the folder (default 1)")
    ap.add_argument("--layer", default="0", help="template/sprite <layer> field (default 0)")
    ap.add_argument("--template-variant", default="0", help="template <variant> field to read (default 0)")
    ap.add_argument("--dirs", default="e,s,n", help="directions to generate (default e,s,n; e is the hero)")
    ap.add_argument("--dir", default=None, help="generate a single direction (overrides --dirs), e.g. --dir s")
    ap.add_argument("--size", type=int, default=512, help="output sprite size in px (default 512; generation stays at 512, only the saved file scales)")
    ap.add_argument("--bg-thresh", type=float, default=60.0, help="corner-flood key tolerance (colour distance; default 60)")
    ap.add_argument("--choke", type=int, default=2, help="erode alpha inward N px to kill the AA fringe/halo at the outline (default 2; 1 can still bleed bg)")
    ap.add_argument("--keep-bg", action="store_true", help="skip background removal; save the raw sprite on its bg")
    ap.add_argument("--hsym", default="", help="directions (subset of 'sne') to force horizontal (left-right) mirror symmetry, e.g. --hsym ns for front/back views")
    ap.add_argument("--vsym", default="", help="directions (subset of 'sne') to force vertical (top-bottom) mirror symmetry")
    ap.add_argument("--dn", type=float, default=DN, help=f"i2i denoise strength (default {DN}; higher = more repaint freedom / less template fidelity)")
    ap.add_argument("--cn", type=float, default=CN, help=f"ControlNet edge strength (default {CN}; higher = harder pull toward the template silhouette)")
    ap.add_argument("--cn-end", type=float, default=CN_END, help=f"ControlNet end_percent: fraction of steps it stays active (default {CN_END}; higher holds the silhouette deeper into the paint-in phase)")
    ap.add_argument("--edge-thresh", type=int, default=30, help="FIND_EDGES cutoff for the ControlNet edge map (default 30; raise to 60-90 to keep only the strong silhouette and drop interior noise, so high CN doesn't blow up the lines)")
    ap.add_argument("--cfg", type=float, default=CFG, help=f"classifier-free guidance scale (default {CFG:g}; lower loosens prompt/prior adherence — the model chases 'wolf' less hard, so it adds legs less; higher pushes harder toward the prompt and control)")
    args = ap.parse_args()

    DN, CN, CN_END, CFG = args.dn, args.cn, args.cn_end, args.cfg

    hsym = {c for c in args.hsym.lower() if not c.isspace() and c != ","}
    vsym = {c for c in args.vsym.lower() if not c.isspace() and c != ","}
    bad = (hsym | vsym) - set(FACE)
    if bad:
        print(f"generate: --hsym/--vsym ignores unknown direction(s) {''.join(sorted(bad))} (valid: {''.join(FACE)})", file=sys.stderr)

    seed = args.seed if args.seed is not None else random.randint(1, 2**31 - 1)
    tid, layer, tvar = args.template_id, args.layer, args.template_variant   # template <id>.<dir>.<layer>.<variant>
    dirs = [args.dir.strip()] if args.dir else [d.strip() for d in args.dirs.split(",") if d.strip()]
    if "e" in dirs: dirs = ["e"] + [d for d in dirs if d != "e"]   # hero first
    from_path = args.from_path.strip("/")
    out_path = (args.to_path or from_path).strip("/")

    pos, neg_user = resolve_prompts(args, from_path)
    neg = ", ".join(x for x in [neg_user, GENERIC_NEG] if x)

    # persist the reusable prompt as a loose seed-keyed sidecar at the from-level
    # (shared across the e/s/n leaves, so it's not a per-variant leaf member)
    tpl_dir = os.path.join(REPO, "textures", from_path)
    os.makedirs(tpl_dir, exist_ok=True)
    prompt_out = os.path.join(tpl_dir, f"{seed}.prompt.txt")
    write_prompt_file(prompt_out, pos, neg_user, seed, from_path)

    out_dir = os.path.join(REPO, "textures", out_path)
    os.makedirs(out_dir, exist_ok=True)
    print(f"generate: from={from_path} to={out_path} seed={seed} dirs={dirs} size={args.size}"
          + ("" if args.keep_bg else f" bg-key(thresh {args.bg_thresh:g})"))
    print(f"  recipe -> dn={DN:g} cn={CN:g} cn_end={CN_END:g} edge_thresh={args.edge_thresh} steps={STEPS} cfg={CFG:g}")
    print(f"  bg-key -> " + ("keep-bg (no removal)" if args.keep_bg else f"bg_thresh={args.bg_thresh:g} choke={args.choke}"))
    if hsym: print(f"  hsym (left-right) -> {''.join(sorted(hsym))}")
    if vsym: print(f"  vsym (top-bottom) -> {''.join(sorted(vsym))}")
    print(f"  positive -> {pos}")
    print(f"  negative -> {neg}")
    print(f"  wrote {os.path.relpath(prompt_out, REPO)}")

    hero_name = None
    if "e" not in dirs:   # regenerating only s/n — anchor to the existing east sprite if present
        hero_img = load_hero_from_disk(out_dir, tid, layer, seed)
        if hero_img is not None:
            hero_name = _upload(hero_img, f"artgen_{seed}_hero.png")
            print(f"  IP anchor: existing {sprite_name(tid, 'e', layer, seed)}")
        else:
            print(f"generate: no existing east sprite ({sprite_name(tid, 'e', layer, seed)}); {dirs} generate without IP anchor", file=sys.stderr)
    for d in dirs:
        if d not in FACE:
            print(f"generate: skipping unknown direction '{d}'", file=sys.stderr); continue
        tpl = load_template(from_path, tid, d, layer, tvar)
        ref_name = _upload(tpl, f"artgen_{seed}_{d}_ref.png")
        edge_name = _upload(edge_map(tpl, args.edge_thresh), f"artgen_{seed}_{d}_edge.png")
        full_pos = f"{pos}, {STYLE.format(face=FACE[d])}"
        if d == "e" or hero_name is None:
            raw = _run(graph_hero(full_pos, neg, ref_name, edge_name, seed))
        else:
            raw = _run(graph_ip(full_pos, neg, ref_name, edge_name, hero_name, seed))
        img = Image.open(io.BytesIO(raw)).convert("RGB")
        if d == "e":
            hero_name = _upload(img, f"artgen_{seed}_hero.png")   # east (on white) becomes the IP anchor
        sprite = img if args.keep_bg else remove_bg_floodfill(img, thresh=args.bg_thresh, choke=args.choke)
        if d in hsym: sprite = make_symmetric(sprite, "h")             # force symmetry after the cut
        if d in vsym: sprite = make_symmetric(sprite, "v")
        sprite = resize_sprite(sprite, args.size)                       # scale AFTER the cut (premultiplied)
        out = os.path.join(out_dir, sprite_name(tid, d, layer, seed))   # SEED = variant
        os.makedirs(os.path.dirname(out), exist_ok=True)                # ensure the variant leaf
        sprite.save(out)
        print(f"  wrote {os.path.relpath(out, REPO)}")
    print(f"generate: done (id {tid} variant {seed})")

if __name__ == "__main__":
    main()
