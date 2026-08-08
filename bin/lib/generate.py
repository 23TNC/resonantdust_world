#!/usr/bin/env python3
"""bin/art generate — creature sprite generator.

Drives the ComfyUI box (SDXL i2i + ControlNet edge, east hero + IP-Adapter
anchored south/north) to paint directional sprites from a template set.

Naming — object-model taxonomy leaf (docs/components/dev/textures/design/texture-layout/):
<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>. --from/--to IS a kind path
(type/subtype/kind, any depth, e.g. pawn/animal/wolf). The TEMPLATE is held by the kind —
one pose set shared by every variant — and each run paints a new numbered variant beside it:
  template in   textures/<kind>/template.<dir>.<part>.png       (kind-level; the e/s/n poses)
  sprites  out  textures/<kind>/<seed>/sprite.<dir>.<part>.png  (SEED = the variant folder, all dirs together; transparent RGBA)
  prompt   out  textures/<kind>/<seed>.prompt.txt               (reusable, --prompt; loose sidecar)
One kind-level template feeds many seed variants; each variant folder holds its own e/s/n sprites.

Background is removed by a corner flood-fill (colour auto-detected from the four
corners, stops at the sharp outline transition). Prompts are used verbatim unless
--llm is passed, which expands the terse --positive/--negative via Claude (needs
ANTHROPIC_API_KEY or bin/keys/anthropic.env). Generic negatives are always added.

Env: COMFYUI_URL (default http://172.16.10.10:8188), ANTHROPIC_API_KEY.
"""
import argparse, json, os, sys, time, uuid, random, io, shutil, urllib.request, urllib.parse
from PIL import Image, ImageFilter
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")

# ---- proven recipe (see the sprite-gen memory / docs/sprite-gen-plan.md) ----
# BASE CHECKPOINT. Was cyberrealisticXL_v80 until 2026-08-02 — a PHOTOREALISM finetune, chosen only
# because it was the sole SDXL checkpoint on the box in March, while the target art is flat regions
# bounded by hard black outlines (work/2026-08-02-lora-beat-e07 I5). Measured no-LoRA probe: this
# base puts 4/4 east cells wider than tall (mean aspect 1.89) where Illustrious-XL managed 0/4
# (1.02) — i.e. it draws BODIES in side profile, not busts. Illustrious is not ruled out (user,
# 2026-08-02: "I am not ruling out future attempts"); switching is this constant plus eval_set.json.
MODEL   = "sdxl/animagine-xl-4.0.safetensors"
CN_MODEL= "sdxl/diffusion_pytorch_model.safetensors"
STEPS, CFG, SAMPLER, SCHED = 26, 6.0, "dpmpp_2m", "karras"
# Measured on the quadruped LoRA via `lora_eval.py --pipeline` (4x4 dn/cn grid + cn_end
# probes, wolf e/s/n, 3 seeds each): with a style LoRA carrying the look, ControlNet only
# has to hold the SILHOUETTE, so denoise can go to 1.0 (template pixels fully repainted)
# and cn can drop far. dn1.0/cn0.20/cn_end0.30 scores ~3.4x the seed-to-seed variety of the
# old 0.70/0.50/0.90 with 9/9 sprites still geometrically valid. Without a LoRA these are
# too loose — pass --dn/--cn/--cn-end to restore the tighter legacy recipe.
DN, CN, CN_END = 1.00, 0.20, 0.30
IP_WEIGHT = 0.6
# How the IP-Adapter blends the hero into south/north. `style transfer` was the original and it is
# the wrong lever for our purpose (work/2026-08-08-direction-consistency I3): it exists to carry
# STYLE while discarding composition and content, and "this is the same animal, same markings, same
# value" is content. Selectable so P2.2 can sweep it rather than argue about it.
IP_WEIGHT_TYPE = "style transfer"
AUTO_MIN_MATCH = 0.65        # --control auto declines below this (see resolve_auto)
DIRS = ["e", "s", "n"]                       # e generated first = the IP hero
FACE = {
  "e": "lying down facing right, side profile",
  "s": "facing down toward the viewer, front view, head chest and face visible",
  "n": "facing up away from the viewer, back view, seen from behind, back of head body and tail toward the camera",
}
# The style boilerplate appended to --positive. TWO FORMS, because the right one depends on how the
# LoRA in use was CAPTIONED (work/2026-08-08-direction-consistency I1):
#
#   prose - English sentences. Correct for e07 and anything else trained on natural-language
#           captions. This was the only form until 2026-08-08.
#   tags  - the tag list the style-only corpus was captioned with, direction tripled at the head.
#           Correct for the rd_style family (run-16 onward), which has NEVER SEEN the prose words.
#
# Measured on r20g07, same seed and knobs, prompt text the only difference: the prose form produced
# cyan rim-light and peach hindquarter artifacts in all three directions and a hollow unfilled tail
# on south, with saturation incoherent ACROSS directions of one animal (48.0 / 20.4 / 28.3). The tag
# form produced a clean wolf in all three (4.4 / 6.4 / 4.1). The artifacts are the base model's
# priors filling in for words the LoRA has no response to.
STYLE_PROSE = ("oblique top-down view game creature sprite, {face}, flat cel shading, bold dark outline, "
               "hand-painted 2D game art, cartoon, on a plain solid white background")
# Direction phrases exactly as the corpus was captioned (style-not-species F1 addendum whitelist).
STYLE_TAG_PHRASE = {
  "e": "side profile, side view, facing right",
  "s": "front view, facing the viewer, facing forward",
  "n": "back view, facing away, seen from behind",
}
STYLE_TAG_DIR = {"e": "rd_east", "s": "rd_south", "n": "rd_north"}
# The body plan is a CAPTION token the LoRA discriminates on (six plans, 27-498 examples each), not
# decoration. Overridable per-subject; quadruped is the corpus's dominant plan at 498/701.
STYLE_TAG_BODY_PLAN = "rd_quadruped"

def style_for(d, form, body_plan=None, override=None):
    """The style boilerplate for direction `d`.

    `override` (--style) wins outright and is passed through .format(face=...) so the existing
    '{face}' escape keeps working for callers that use it."""
    if override is not None:
        return override.format(face=FACE[d]) if "{face}" in override else override
    if form == "prose":
        return STYLE_PROSE.format(face=FACE[d])
    t = STYLE_TAG_DIR[d]
    return (f"{t}, {t}, {t}, rd_style, rd_animal, {body_plan or STYLE_TAG_BODY_PLAN}, "
            f"{STYLE_TAG_PHRASE[d]}, single creature, full body")

STYLE = STYLE_PROSE          # retained for callers/tests that import it; --style still overrides
# Default form. `tags` because every LoRA this project trains from run-16 onward is tag-captioned
# (the style-not-species thesis produces nothing else). `--style-form prose` restores the old
# behaviour for e07 and any future natural-language model.
STYLE_FORM = "tags"
STYLE_OVERRIDE = None
# LoRAs known to be PROSE-captioned. Used only to warn on an obvious mismatch — a wrong style form
# is silent in the output and shows up as artifacts nobody attributes to the prompt (I1).
PROSE_LORAS = ("rd_quadruped_e07",)
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
# <map>.<dir>.<part>.png. Generated sprites sit in a per-variant <seed>/ leaf; the TEMPLATE is a
# kind-level asset (no variant folder), one pose set shared by every variant of the kind.
# `part` is the old `layer` segment (renamed, moved into the file).
def template_name(d, part):              # the pose reference (map=template), held by the kind
    return texpath.map_name("template", d, part)
def sprite_name(d, part, seed):          # generated sprite: SEED is the variant leaf (map=sprite)
    return os.path.join(texpath.variant_leaf(seed), texpath.map_name("sprite", d, part))

def _on_white(im, size=512):
    """Flatten RGBA onto the white plate the pipeline (and the LoRA's training data) expects."""
    t = im.convert("RGBA").resize((size, size), Image.LANCZOS)
    f = Image.new("RGBA", (size, size), (255, 255, 255, 255)); f.alpha_composite(t)
    return f.convert("RGB")

def load_template(from_path, d, part, tvar, required=True):
    """The hand-authored control image, or None.

    `required=False` is the template-free path: a missing template stops being fatal, so a kind
    with no art can still generate (the LoRA carries both the style and the body plan — see
    docs/work/2026-07-25-template-free-generation/). Callers that get None must build a graph
    with no ControlNet."""
    # Canonical: template held at the kind level (textures/<kind>/template.<dir>.<part>.png).
    # Fallback: an old set still inside a <variant>/ leaf (textures/<kind>/<tvar>/template...).
    kind_dir = os.path.join(REPO, "textures", from_path)
    p = os.path.join(kind_dir, template_name(d, part))
    if not os.path.exists(p):
        alt = os.path.join(kind_dir, texpath.variant_leaf(tvar), template_name(d, part))
        if not os.path.exists(alt):
            if not required:
                return None
            raise SystemExit(f"generate: template not found: {p}")
        p = alt
    return _on_white(Image.open(p))

def resolve_control(mode, from_path, d, part, tvar):
    """The control image for this direction, per --control. Returns a 512 RGB-on-white image,
    or None for the template-free path.

      template  hand-authored art (the default; explicit art always outranks an inferred
                silhouette, forks.md F2). Missing art is still fatal here — asking for a
                template and silently getting none would hide a typo'd --from.
      none      no control at all: txt2img + LoRA.
      corpus:<Species> / family:<f>   corpus-derived silhouettes — P2, not yet wired.
    """
    if mode == "none":
        return None
    if mode.startswith(("corpus:", "family:")):
        import silhouette_bank
        return silhouette_bank.resolve(mode, d)
    return load_template(from_path, d, part, tvar, required=True)

def resolve_auto(pos_base, neg, seed, probe_dir="e"):
    """`--control auto` — pick the control species by MEASUREMENT instead of a family table.

    Two passes: generate one template-free PROBE (no ControlNet), then find the bank silhouette
    whose body plan is nearest that probe, and use that species for every direction.

    Probing on EAST is deliberate: it is the only direction that passed 10/10 in both control modes
    (predecessor I4), so it is the most trustworthy read of what the species actually looks like.
    Resolving ONE species for all three directions — rather than a nearest-match per direction —
    costs one extra generation instead of three AND keeps the e/s/n set coherent, since a set built
    from three different reference animals would inherit three different body plans."""
    import silhouette_bank
    full = f"{pos_base}, {style_for(probe_dir, STYLE_FORM, None, STYLE_OVERRIDE)}"
    raw = _run(graph_hero(full, neg, None, None, seed))          # txt2img, no control
    probe = Image.open(io.BytesIO(raw)).convert("RGB")
    hits = silhouette_bank.nearest(probe, probe_dir, k=5)
    if not hits:
        return None, []
    # DECLINE when nothing in the bank really fits. Measured: species that genuinely have a
    # body-plan twin in the corpus score 0.705-0.967 against it (Elephant/Gorilla are the floor),
    # while a giant anteater — whose body plan exists nowhere in a quadruped corpus — tops out at
    # 0.532. Forcing its best match (Gorilla) was measurably WORSE than using no control at all:
    # the snout vanished entirely. So below the threshold, no control beats a bad one.
    if hits[0][1] < AUTO_MIN_MATCH:
        print(f"  control -> auto DECLINED (best match {hits[0][0]} {hits[0][1]:.2f} < "
              f"{AUTO_MIN_MATCH:.2f}); no bank body plan fits, using no control")
        return None, hits
    return hits[0][0], hits

# ---------------------------------------------------------------- candidate screening (P3)
# The generalized generator fails often; that is fine as long as failure is DETECTED. The gate
# is forks.md F4, calibrated against hand-judged sprites — blobs==1, keyable plate, aspect
# within 50% of the real corpus sprite, solidity>=0.35.
# Calibrated against .staging/gate-cal/labels.csv (67 hand-labelled sprites), not chosen.
# solidity was REMOVED: on the calibration set it separated good from bad at only 0.19 sd
# while causing a false negative (a good horned oryx rejected at 0.349 vs 0.35) — it charges
# real cost for almost no signal. d_fill was TRIED and rejected: `fill` looks like a strong
# discriminator in isolation (0.83 sd) but as a hard gate it rejects many good sprites, taking
# total errors from 3 to 8-11. Errors on the calibration set: 3 (was 4).
GATE = dict(bg_uni=0.75, d_aspect=50.0)

def score_sprite(sprite, ref_spec, d):
    """(metrics, passed) for one RGBA sprite. `ref_spec` is 'Folder[:stem]' naming the real
    corpus sprite that defines correct proportions; without it the aspect check is skipped
    (structure-only screening) rather than silently inventing a reference."""
    import lora_eval as L
    w = Image.new("RGBA", sprite.size, (255, 255, 255, 255)); w.alpha_composite(sprite.convert("RGBA"))
    m = L.measure(w.convert("RGB"))
    d_aspect = None
    if ref_spec:
        folder, _, stem = ref_spec.partition(":")
        r = L.reference(folder, stem or folder, {"e": "east", "s": "south", "n": "north"}[d])
        if r: d_aspect = 100.0 * abs(m["aspect"] - r["aspect"]) / max(r["aspect"], 1e-3)
    ok = (m["blobs"] == 1 and m["bg_uni"] >= GATE["bg_uni"]
          and (d_aspect is None or d_aspect <= GATE["d_aspect"]))
    m["d_aspect"] = round(d_aspect, 1) if d_aspect is not None else ""
    return m, ok

# ------------------------------------------------------------------ interior metrics
# Cross-direction CONSISTENCY, measured on the sprite INTERIOR only.
#
# work/2026-08-08-direction-consistency: at cn >= 0.5 the silhouette comes from the authored
# ControlNet template, so any consistency measured on SHAPE is measuring the templates and says
# nothing about the pipeline (I4 there). Only the interior — value, saturation, palette — is
# evidence. Hence: opaque pixels only, and no shape term.
#
# The ceiling is measured off the real corpus, not chosen: three views of one animal sit within
# 9.0 luminance (AEXP_Coyote 3.0, Wolf_Timber 7.7, Direwolf 7.7, Fox_Red 9.0).
LUM_SPREAD_CEILING = 9.0
# Corpus animals measure 62-121 mean luminance over their non-plate pixels. A set that is
# internally consistent AND uniformly too pale scores perfectly on spread while being wrong, so
# the absolute band is reported beside it (F4 there).
LUM_BAND = (62.0, 121.0)

def interior_metrics(sprite):
    """Luminance / saturation / coverage over a sprite's OPAQUE pixels.

    Returns None for a fully transparent sprite rather than a zero row — an empty result must not
    average into a spread as if it were a measurement."""
    a = np.array(sprite.convert("RGBA"))
    op = a[..., 3] > 128
    if not op.any(): return None
    px = a[..., :3][op].astype(np.float64)
    lum = 0.2126 * px[:, 0] + 0.7152 * px[:, 1] + 0.0722 * px[:, 2]
    return dict(lum=round(float(lum.mean()), 1),
                sat=round(float((px.max(1) - px.min(1)).mean()), 1),
                cov=round(100.0 * float(op.sum()) / op.size, 1))

def report_consistency(rows):
    """Print per-direction interior metrics and the cross-direction luminance SPREAD.

    The spread is per CANDIDATE SEED — a leaf is the unit that ships as a set, so mixing seeds
    would report a number no single sprite set actually has. Needs >= 2 directions to mean
    anything; with one direction there is nothing to be consistent with, and that is said rather
    than printed as 0.0."""
    have = [r for r in rows if r.get("lum") is not None]
    if not have: return
    print("  interior metrics (opaque pixels only — silhouette comes from the template, F3/I4):")
    for seed in sorted({r["seed"] for r in have}):
        sr = [r for r in have if r["seed"] == seed]
        for r in sorted(sr, key=lambda r: "esn".index(r["dir"]) if r["dir"] in "esn" else 9):
            band = "" if LUM_BAND[0] <= r["lum"] <= LUM_BAND[1] else "  <- outside corpus band 62-121"
            print(f"    seed {seed}  {r['dir']}: lum={r['lum']:6.1f} sat={r['sat']:5.1f} cov={r['cov']:5.1f}%{band}")
        if len(sr) < 2:
            print(f"    seed {seed}  spread: n/a (one direction — nothing to be consistent with)")
            continue
        lums = [r["lum"] for r in sr]
        spread = max(lums) - min(lums)
        verdict = "OK" if spread <= LUM_SPREAD_CEILING else f"{spread / LUM_SPREAD_CEILING:.1f}x over"
        print(f"    seed {seed}  spread={spread:.1f}  (corpus ceiling {LUM_SPREAD_CEILING:.1f} — {verdict})")

def report_candidates(rows, out_dir, out_path, dirs, args):
    """Write scores.csv, quarantine rejected variant leaves, and name the best seed per direction.

    A candidate is a SEED = one variant leaf holding e/s/n. Rejection is per LEAF, not per sprite
    (forks.md F5): a variant with a good east and a broken south is not usable as a set, and the
    renderer expects a leaf's directions to belong together."""
    if not rows: return
    import csv as _csv
    try:
        sys.path.insert(0, HERE); import lora_eval as _L
        cells = [(f"{r['dir']} {'ok' if r['valid'] else 'REJECT'}", f"seed {r['seed']}",
                  os.path.join(REPO, r["path"])) for r in rows]
        sp = _L.contact_sheet(cells, os.path.join(out_dir, "sheet.png"))
        print(f"  sheet -> {os.path.relpath(sp, REPO)}   (review before trusting the scores)")
    except Exception as e:
        print(f"generate: contact sheet skipped ({e})", file=sys.stderr)
    if args.ref or args.candidates > 1:
        with open(os.path.join(out_dir, "scores.csv"), "w", newline="") as f:
            w = _csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
    seeds = sorted({r["seed"] for r in rows})
    bad = [s for s in seeds if not all(r["valid"] for r in rows if r["seed"] == s)]
    for s in bad:                                   # quarantine the whole leaf
        leaf = os.path.join(out_dir, texpath.variant_leaf(s))
        rej = os.path.join(out_dir, "_rejected", texpath.variant_leaf(s))
        if os.path.isdir(leaf):
            os.makedirs(os.path.dirname(rej), exist_ok=True)
            if os.path.isdir(rej): shutil.rmtree(rej)
            shutil.move(leaf, rej)
    kept = [s for s in seeds if s not in bad]
    if args.candidates > 1 or args.ref:
        print(f"generate: {len(kept)}/{len(seeds)} candidate(s) passed the gate"
              + (f"; quarantined {sorted(bad)} -> _rejected/" if bad else ""))
        for d in dirs:                              # best surviving seed per direction
            cand = [r for r in rows if r["dir"] == d and r["seed"] in kept and r["d_aspect"] != ""]
            if cand:
                b = min(cand, key=lambda r: r["d_aspect"])
                print(f"  best {d}: seed {b['seed']}  d_aspect={b['d_aspect']}%")
    print(f"generate: done (kind {out_path}, variants {kept if kept else 'none kept'})")

def load_hero_from_disk(out_dir, part, seed):
    """An already-generated east sprite (<seed>/sprite.e.<part>.png), flattened onto
    white, for use as the IP anchor when east isn't regenerated this run."""
    p = os.path.join(out_dir, sprite_name("e", part, seed))
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
# LORA/LORA_STRENGTH are set from --lora/--lora-strength. When a LoRA is set, node 50
# (LoraLoader) sits between the checkpoint and everything downstream, so BOTH the model
# and the CLIP text encoders see the trained weights (a style LoRA trains the TE too).
LORA, LORA_STRENGTH = None, 1.0

def _base(g):
    """Add the checkpoint (+ optional LoRA) loaders to graph `g`; return (model_ref, clip_ref)."""
    g["4"] = {"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":MODEL}}
    if not LORA:
        return ["4",0], ["4",1]
    g["50"] = {"class_type":"LoraLoader","inputs":{"model":["4",0],"clip":["4",1],
               "lora_name":LORA,"strength_model":LORA_STRENGTH,"strength_clip":LORA_STRENGTH}}
    return ["50",0], ["50",1]

def _tail(pos, neg, ref_name, edge_name, model_ref, seed, clip_ref=("4",1), size=512):
    """The shared graph tail. `ref_name`/`edge_name` may be None (template-free): the i2i
    latent falls back to an EmptyLatentImage and the ControlNet nodes (30/31/32) are omitted
    entirely, so the sampler reads the raw text conditioning."""
    clip_ref = list(clip_ref)
    g = {
     "6":{"class_type":"CLIPTextEncode","inputs":{"text":pos,"clip":clip_ref}},
     "7":{"class_type":"CLIPTextEncode","inputs":{"text":neg,"clip":clip_ref}},
     "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["4",2]}},
     "9":{"class_type":"SaveImage","inputs":{"filename_prefix":"artgen","images":["8",0]}}}
    if ref_name is not None:                       # i2i from the control art
        g["20"] = {"class_type":"LoadImage","inputs":{"image":ref_name}}
        g["21"] = {"class_type":"VAEEncode","inputs":{"pixels":["20",0],"vae":["4",2]}}
        latent, denoise = ["21",0], DN
    else:                                          # template-free: nothing to denoise FROM
        g["21"] = {"class_type":"EmptyLatentImage","inputs":{"width":size,"height":size,"batch_size":1}}
        latent, denoise = ["21",0], 1.0
    if edge_name is not None:
        g["30"] = {"class_type":"LoadImage","inputs":{"image":edge_name}}
        g["31"] = {"class_type":"ControlNetLoader","inputs":{"control_net_name":CN_MODEL}}
        g["32"] = {"class_type":"ControlNetApplyAdvanced","inputs":{"positive":["6",0],"negative":["7",0],"control_net":["31",0],"image":["30",0],"strength":CN,"start_percent":0.0,"end_percent":CN_END}}
        pos_ref, neg_ref = ["32",0], ["32",1]
    else:
        pos_ref, neg_ref = ["6",0], ["7",0]
    g["3"] = {"class_type":"KSampler","inputs":{"seed":seed,"steps":STEPS,"cfg":CFG,"sampler_name":SAMPLER,"scheduler":SCHED,"denoise":denoise,"model":model_ref,"positive":pos_ref,"negative":neg_ref,"latent_image":latent}}
    return g

def graph_hero(pos, neg, ref_name, edge_name, seed):
    g = {}
    model_ref, clip_ref = _base(g)
    g.update(_tail(pos, neg, ref_name, edge_name, model_ref, seed, clip_ref)); return g

def graph_ip(pos, neg, ref_name, edge_name, hero_name, seed):
    g = {}
    base_model, clip_ref = _base(g)
    g.update({
     "40":{"class_type":"IPAdapterUnifiedLoader","inputs":{"model":base_model,"preset":"PLUS (high strength)"}},
     "41":{"class_type":"LoadImage","inputs":{"image":hero_name}},
     # IPAdapterAdvanced, not IPAdapter: the simple node hardcodes a short weight_type list, and the
     # composition-carrying types this stream needs (I3) live on the advanced one.
     "42":{"class_type":"IPAdapterAdvanced","inputs":{"model":["40",0],"ipadapter":["40",1],"image":["41",0],
           "weight":IP_WEIGHT,"weight_type":IP_WEIGHT_TYPE,"combine_embeds":"concat",
           "start_at":0.0,"end_at":1.0,"embeds_scaling":"V only"}}})
    g.update(_tail(pos, neg, ref_name, edge_name, ["42",0], seed, clip_ref)); return g

# ---------------------------------------------------------------- main
def main():
    global DN, CN, CN_END, CFG, LORA, LORA_STRENGTH, STYLE_FORM, STYLE_OVERRIDE, IP_WEIGHT, IP_WEIGHT_TYPE   # CLI overrides of the module defaults
    ap = argparse.ArgumentParser(prog="art generate", description="Generate directional creature sprites from a template set.")
    ap.add_argument("--from", dest="from_path", required=True, help="kind path under textures/ holding the kind-level template (type/subtype/kind, e.g. pawn/animal/wolf)")
    ap.add_argument("--to", dest="to_path", default=None, help="output kind path under textures/ (default: same as --from)")
    ap.add_argument("--positive", default="", help="short creature description")
    ap.add_argument("--negative", default="", help="short 'avoid' description")
    ap.add_argument("--prompt", default=None, help="reuse a saved prompt: an id (textures/<from>/<id>.prompt.txt) or a path")
    ap.add_argument("--llm", action="store_true", help="expand --positive/--negative via Claude (spends API tokens; off by default)")
    ap.add_argument("--seed", type=int, default=None, help="seed; also the output variant folder. random if omitted")
    ap.add_argument("--part", default="0", help="sprite <part> field — body=0, head=1, … (default 0)")
    ap.add_argument("--candidates", type=int, default=1,
                    help="generate N seeded candidates (seed, seed+1, ...); each is one variant leaf, auto-screened")
    ap.add_argument("--ref", default=None,
                    help="corpus sprite defining correct proportions for the gate, e.g. Bear or AEXP_Jaguar:Jaguar")
    ap.add_argument("--control", default="template",
                    help="control-image source: template (default) | auto (probe, then nearest bank body plan) | corpus:<Species> | family:<f> | none (txt2img+LoRA, no ControlNet)")
    ap.add_argument("--template-variant", default="0", help="legacy fallback: variant folder to read the template from when none sits at the kind level (default 0)")
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
    ap.add_argument("--lora", default=None, help="style LoRA to apply, as ComfyUI sees it under models/loras (e.g. rd_quadruped_e07.safetensors); applied to BOTH the model and the text encoders")
    ap.add_argument("--lora-strength", type=float, default=1.0, help="LoRA strength for model+clip (default 1.0; try 0.6-0.9 if it overpowers the template)")
    ap.add_argument("--style", default=None, help="override the style boilerplate appended to --positive (use the LoRA's trained tags, e.g. 'rd_style, rd_animal, rd_quadruped, {face}')")
    ap.add_argument("--ip-weight", type=float, default=IP_WEIGHT,
                    help=f"IP-Adapter weight for the south/north hero anchor (default {IP_WEIGHT})")
    ap.add_argument("--ip-weight-type", default=IP_WEIGHT_TYPE,
                    help=f"how the adapter blends the hero (default '{IP_WEIGHT_TYPE}'). 'style transfer' carries style and DISCARDS content by design, which is the wrong lever for cross-view identity (I3); composition-carrying types are the point of the sweep.")
    ap.add_argument("--east-self-anchor", action="store_true",
                    help="re-render east through the IP-Adapter graph anchored on its own first pass, so all three directions come off the SAME graph (I6: east is the outlier in 11/12 sets precisely because it alone skips the adapter). Costs one extra generation per set.")
    ap.add_argument("--style-form", choices=["tags", "prose"], default=STYLE_FORM,
                    help=f"which style boilerplate to append (default {STYLE_FORM}). 'tags' = the caption form the rd_style LoRA family was trained on; 'prose' = the English boilerplate correct for e07 and other natural-language-captioned models. Ignored when --style is given.")
    ap.add_argument("--body-plan", default=STYLE_TAG_BODY_PLAN,
                    help=f"body-plan caption token for --style-form tags (default {STYLE_TAG_BODY_PLAN}); the corpus discriminates six, so a biped rendered as rd_quadruped asks for the wrong silhouette")
    ap.add_argument("--metrics", dest="metrics", action="store_true", default=True,
                    help="report per-direction interior luminance/saturation/coverage and the cross-direction luminance SPREAD (default on — no sheet should be judged by eye alone)")
    ap.add_argument("--no-metrics", dest="metrics", action="store_false",
                    help="suppress the interior-metrics report")
    args = ap.parse_args()

    d0 = {"dn": DN, "cn": CN, "cn-end": CN_END}   # module defaults, for the --control none notice
    DN, CN, CN_END, CFG = args.dn, args.cn, args.cn_end, args.cfg
    LORA, LORA_STRENGTH = args.lora, args.lora_strength
    STYLE_FORM = args.style_form
    STYLE_OVERRIDE = args.style
    IP_WEIGHT, IP_WEIGHT_TYPE = args.ip_weight, args.ip_weight_type
    print(f"  ip-adapter -> weight={IP_WEIGHT} type='{IP_WEIGHT_TYPE}'")
    # A wrong style form is INVISIBLE in the output — it shows up as artifacts nobody attributes to
    # the prompt (I1). So the form is always echoed, and an obvious mismatch is called out.
    if STYLE_OVERRIDE:
        print(f"  style -> --style override (form {STYLE_FORM} ignored)")
    else:
        print(f"  style -> {STYLE_FORM}" + (f" ({args.body_plan})" if STYLE_FORM == "tags" else ""))
        if LORA and STYLE_FORM == "tags" and any(p in LORA for p in PROSE_LORAS):
            print(f"generate: {LORA} is PROSE-captioned but --style-form is 'tags'; "
                  f"pass --style-form prose or it will be prompted in a language it never saw", file=sys.stderr)

    hsym = {c for c in args.hsym.lower() if not c.isspace() and c != ","}
    vsym = {c for c in args.vsym.lower() if not c.isspace() and c != ","}
    bad = (hsym | vsym) - set(FACE)
    if bad:
        print(f"generate: --hsym/--vsym ignores unknown direction(s) {''.join(sorted(bad))} (valid: {''.join(FACE)})", file=sys.stderr)

    seed = args.seed if args.seed is not None else random.randint(1, 2**31 - 1)
    part, tvar = args.part, args.template_variant   # template.<dir>.<part>; tvar = legacy fallback variant folder
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
    if args.control == "none":
        # No control image => no i2i latent and no ControlNet, so all three dials are inert.
        # Say so rather than printing a recipe the run does not actually honour.
        noted = [f"--{k}" for k, v in (("dn", args.dn), ("cn", args.cn), ("cn-end", args.cn_end))
                 if v != d0[k]]
        print("  control -> none (txt2img + LoRA; no ControlNet, no i2i latent)")
        if noted:
            print(f"  NOTE: {', '.join(noted)} ignored under --control none "
                  f"(nothing to denoise from, nothing to constrain); sampling at full strength",
                  file=sys.stderr)
    else:
        print(f"  recipe -> dn={DN:g} cn={CN:g} cn_end={CN_END:g} edge_thresh={args.edge_thresh} steps={STEPS} cfg={CFG:g}")
    print(f"  bg-key -> " + ("keep-bg (no removal)" if args.keep_bg else f"bg_thresh={args.bg_thresh:g} choke={args.choke}"))
    if hsym: print(f"  hsym (left-right) -> {''.join(sorted(hsym))}")
    if vsym: print(f"  vsym (top-bottom) -> {''.join(sorted(vsym))}")
    print(f"  positive -> {pos}")
    print(f"  negative -> {neg}")
    print(f"  wrote {os.path.relpath(prompt_out, REPO)}")

    if args.control == "auto":
        sp, hits = resolve_auto(pos, neg, seed)
        if sp:
            args.control = f"corpus:{sp}"
            print(f"  control -> auto picked {sp} "
                  f"(next: {', '.join(f'{h[0]} {h[1]:.2f}' for h in hits[1:4])})")
            # The gate reference must AGREE with the control: scoring d_aspect against a species
            # whose silhouette we did not use measures the wrong thing (it rejected a good auto
            # east purely for not resembling the Elephant it was never shaped by). Explicit --ref
            # still wins, but say so when it disagrees.
            if not args.ref:
                args.ref = sp
                print(f"  gate ref -> {sp} (follows the auto-picked control)")
            elif args.ref.split(":")[0] != sp:
                print(f"generate: --ref {args.ref} differs from the auto-picked control {sp}; "
                      f"the gate will score against {args.ref}", file=sys.stderr)
        else:
            args.control = "none"

    rows = []                                   # one per generated sprite, for scores.csv
    for k in range(max(1, args.candidates)):
        cseed = seed + k
        if args.candidates > 1: print(f"  -- candidate {k+1}/{args.candidates} (seed {cseed})")
        hero_name = None
        if "e" not in dirs:   # regenerating only s/n — anchor to the existing east sprite if present
            hero_img = load_hero_from_disk(out_dir, part, cseed)
            if hero_img is not None:
                hero_name = _upload(hero_img, f"artgen_{cseed}_hero.png")
                print(f"  IP anchor: existing {sprite_name('e', part, cseed)}")
            else:
                print(f"generate: no existing east sprite ({sprite_name('e', part, cseed)}); {dirs} generate without IP anchor", file=sys.stderr)
        for d in dirs:
            if d not in FACE:
                print(f"generate: skipping unknown direction '{d}'", file=sys.stderr); continue
            tpl = resolve_control(args.control, from_path, d, part, tvar)
            if tpl is not None:
                ref_name = _upload(tpl, f"artgen_{cseed}_{d}_ref.png")
                edge_name = _upload(edge_map(tpl, args.edge_thresh), f"artgen_{cseed}_{d}_edge.png")
            else:
                ref_name = edge_name = None      # template-free: no i2i latent, no ControlNet
            full_pos = f"{pos}, {style_for(d, STYLE_FORM, args.body_plan, STYLE_OVERRIDE)}"
            # Echo what is actually SENT. The style form is invisible in the output and its failure
            # mode is artifacts nobody attributes to the prompt (I1) — so the prompt is not a thing
            # to be inferred from flags.
            print(f"  prompt[{d}] -> {full_pos}")
            if d == "e" or hero_name is None:
                raw = _run(graph_hero(full_pos, neg, ref_name, edge_name, cseed))
            else:
                raw = _run(graph_ip(full_pos, neg, ref_name, edge_name, hero_name, cseed))
            img = Image.open(io.BytesIO(raw)).convert("RGB")
            if d == "e":
                hero_name = _upload(img, f"artgen_{cseed}_hero.png")   # east (on white) becomes the IP anchor
                if args.east_self_anchor:
                    # I6: east is the extreme in 11/12 sets because it is the ONE direction that
                    # skips the IP-Adapter — graph_hero for east, graph_ip for south and north. The
                    # pipeline was comparing two graphs and calling the gap inconsistency. Re-render
                    # east through graph_ip anchored on its own first pass, so all three views come
                    # off the same graph. Costs one extra generation per set.
                    raw = _run(graph_ip(full_pos, neg, ref_name, edge_name, hero_name, cseed))
                    img = Image.open(io.BytesIO(raw)).convert("RGB")
                    # The SHIPPED east must also be what s/n anchor on; re-anchoring on the
                    # discarded first pass would put the mismatch straight back.
                    hero_name = _upload(img, f"artgen_{cseed}_hero.png")
                    print(f"  east: second pass through graph_ip (self-anchored)")
            sprite = img if args.keep_bg else remove_bg_floodfill(img, thresh=args.bg_thresh, choke=args.choke)
            if d in hsym: sprite = make_symmetric(sprite, "h")             # force symmetry after the cut
            if d in vsym: sprite = make_symmetric(sprite, "v")
            sprite = resize_sprite(sprite, args.size)                       # scale AFTER the cut (premultiplied)
            out = os.path.join(out_dir, sprite_name(d, part, cseed))        # SEED = variant leaf
            os.makedirs(os.path.dirname(out), exist_ok=True)                # ensure the variant leaf
            sprite.save(out)
            m, ok = score_sprite(sprite, args.ref, d)
            im = interior_metrics(sprite) or dict(lum=None, sat=None, cov=None)
            rows.append(dict(seed=cseed, dir=d, path=os.path.relpath(out, REPO), valid=int(ok),
                             **{k2: round(v, 3) if isinstance(v, float) else v for k2, v in m.items()},
                             **im))
            print(f"  wrote {os.path.relpath(out, REPO)}" + (f"   [{'ok' if ok else 'REJECT'}]" if args.ref or args.candidates > 1 else ""))
    if args.metrics: report_consistency(rows)
    report_candidates(rows, out_dir, out_path, dirs, args)

if __name__ == "__main__":
    main()
