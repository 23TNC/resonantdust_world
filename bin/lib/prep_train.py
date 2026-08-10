#!/usr/bin/env python3
"""Bake the culled/tagged animal-lora set into a kohya-ready training dir.

Output is a FLAT kohya `N_classname` folder (DST/1_animal/) so any kohya version reads it without
recursion or a .toml. Files are species-namespaced to avoid collisions (both AEXP_Hyena/ and loose
Hyena/ hold a 'Hyena_east.png').

Two quality fixes over the first version (docs/work/2026-07-25-sprite-gen-quality/ P2):

  UPSCALE (--upscale esrgan, default).  The old prep resized with LANCZOS, which at these scale
  factors erases the outline: a 64px source at 768 measured mean outline gradient 21.7 with
  literally ZERO strong-edge pixels. Since this art is flat regions bounded by hard outlines —
  vector graphics rendered as bitmaps — the missing pixels are RECONSTRUCTIBLE from that prior
  rather than invented. Measured on the box's ESRGAN: 109.8 (64px Cat) and 120.7 (128px Bear),
  4-5x LANCZOS, and unlike NEAREST (107.4) it does not staircase the curves.

  It used to fall back to LANCZOS automatically when ComfyUI was unreachable, "so the prep never
  hard-fails on a box outage". That is the wrong trade for a TRAINING SET: the upscaler is a
  property of the corpus, so a silent fallback means the same command produces a materially
  different dataset depending on whether a container happened to be up, and nothing downstream can
  tell. Worse, the PER-IMAGE fallback could mix both within one set. It now FAILS LOUD
  (exit 2) and LANCZOS is opt-in via `--upscale lanczos` (whole set) or `--allow-degraded`
  (tolerate per-image misses). See work/2026-08-02-lora-beat-e07 I2.

  SCALE NORMALISATION (--fill).  Source subjects fill wildly different fractions of their frame
  (a bear nearly fills it, a cat occupies a corner), so the LoRA learned that scale is arbitrary
  and generated scale drifts. Each sprite is now cropped to its alpha bbox and rescaled so its
  LONGER side is a fixed fraction of the frame, then centred. Aspect is preserved — see the note
  in normalise() on why equalising bbox AREA is impossible without distorting animals.

  python3 bin/lib/prep_train.py                      # esrgan + normalise (default)
  python3 bin/lib/prep_train.py --upscale lanczos    # the old behaviour, for A/B
"""
import argparse, hashlib, io, json, os, glob, shutil, time, uuid
import urllib.request, urllib.parse
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, ".staging", "animal-lora")
DST = os.path.join(REPO, ".staging", "animal-lora-train")
SIZE = 768
COMFY = os.environ.get("COMFYUI_URL", "http://172.16.10.10:8188").rstrip("/")
ESRGAN_MODEL = "hunyuan/4x_foolhardy_Remacri(1).pth"
ALPHA_MIN = 16               # corpus "transparent" pixels sit at alpha ~3, not 0

# ---------------------------------------------------------------- esrgan via the ComfyUI box
def _upload(pil, name):
    buf = io.BytesIO(); pil.convert("RGB").save(buf, "PNG"); b = "----rd"; body = io.BytesIO()
    def w(s): body.write(s.encode() if isinstance(s, str) else s)
    w(f'--{b}\r\nContent-Disposition: form-data; name="image"; filename="{name}"\r\nContent-Type: image/png\r\n\r\n'); w(buf.getvalue())
    w(f'\r\n--{b}\r\nContent-Disposition: form-data; name="overwrite"\r\n\r\ntrue\r\n--{b}--\r\n')
    r = urllib.request.Request(COMFY + "/upload/image", data=body.getvalue(),
        headers={"Content-Type": f"multipart/form-data; boundary={b}"})
    return json.load(urllib.request.urlopen(r, timeout=60))["name"]

def esrgan(pil, timeout=180):
    """One 4x pass through the box's upscale model. RGB in, RGB out."""
    name = _upload(pil, f"prep_{uuid.uuid4().hex[:8]}.png")
    g = {"1": {"class_type": "LoadImage", "inputs": {"image": name}},
         "2": {"class_type": "UpscaleModelLoader", "inputs": {"model_name": ESRGAN_MODEL}},
         "3": {"class_type": "ImageUpscaleWithModel", "inputs": {"upscale_model": ["2", 0], "image": ["1", 0]}},
         "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": "prep", "images": ["3", 0]}}}
    req = urllib.request.Request(COMFY + "/prompt",
        data=json.dumps({"prompt": g, "client_id": uuid.uuid4().hex}).encode(),
        headers={"Content-Type": "application/json"})
    pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
    t0 = time.time()
    while time.time() - t0 < timeout:
        h = json.load(urllib.request.urlopen(f"{COMFY}/history/{pid}", timeout=20))
        if pid in h:
            im = h[pid]["outputs"]["9"]["images"][0]
            q = urllib.parse.urlencode({"filename": im["filename"], "subfolder": im.get("subfolder", ""), "type": im["type"]})
            return Image.open(io.BytesIO(urllib.request.urlopen(f"{COMFY}/view?{q}", timeout=60).read())).convert("RGB")
        time.sleep(1.0)
    raise RuntimeError("esrgan timeout")

# ---------------------------------------------------------------- prep
def jittered_fill(fill, jitter, key):
    """Per-image fill fraction, deterministic in `key` (the source path).

    Pinning fill to one value gave all 459 training images an identical ~7.5% white margin, and the
    model LEARNED that constant — run-4 drew the margin as a rectangle and composed a portrait inside
    it (issues I3). Jitter keeps scale BOUNDED (the P2 benefit: drift was sd 0.148 before
    normalisation) while removing the constant there is to learn.

    Deterministic rather than random so a rebuild is reproducible — an irreproducible dataset makes
    every A/B afterwards unfalsifiable."""
    if jitter <= 0: return fill
    h = int(hashlib.sha256(key.encode()).hexdigest()[:8], 16) / 0xFFFFFFFF   # [0,1)
    return fill + (h * 2.0 - 1.0) * jitter

def _subject_frac(img):
    """Longer side of the non-white bbox, as a fraction of the frame."""
    import numpy as _np
    # THRESHOLD 235, not 250. ESRGAN leaves plate noise at 238-249, so a <250 cut catches stray
    # corner pixels, reports the bbox as the FULL FRAME, and makes this function return 1.0 for
    # every image — which silently disabled the collapse guard below (it can never see got < want).
    # Measured on a known-good beaver east: <250 gives 1.000, <235 gives 0.896 against want 0.898.
    # That is why the 2026-08-02 build shipped 52% corrupt with the assertion "in place".
    g = _np.asarray(img.convert("L")); m = (g < 235)
    ys, xs = _np.where(m)
    if not len(ys): return 0.0
    return max(ys.max()-ys.min()+1, xs.max()-xs.min()+1) / float(g.shape[0])


def _assert_subject_preserved(src, out, allow_degraded, tol=0.55):
    """The NATURAL path must not change how much of the frame the subject occupies — that is the
    entire point of it. Measured 2026-08-02: a 701-image build came out 52% corrupt, perfectly
    bimodal (366 images with the subject at ~2% of frame, 335 correct, NOTHING in between), while
    the same inputs upscale correctly on a fresh run. So the upscale path fails intermittently
    under a long run and does so INVISIBLY — the images look like clean white plates.

    This is the third silent-corruption bug in this pipeline (the alpha-bbox no-op, the LANCZOS
    fallback, now this). The lesson each time is the same: assert the post-condition rather than
    trust the step. A mismatch aborts unless --allow-degraded."""
    a = src.convert("RGBA")
    plate = Image.new("RGBA", a.size, (255, 255, 255, 255)); plate.alpha_composite(a)
    want = _subject_frac(plate.convert("RGB"))
    got = _subject_frac(out)
    if want > 0 and got < want * tol:
        msg = (f"prep: subject collapsed during upscale — source fills {want:.3f} of its frame, "
               f"output fills {got:.3f} ({got/want:.2f}x). The upscale silently returned unscaled "
               f"content; the image LOOKS like a clean plate and would poison the training set.")
        if not allow_degraded:
            raise SystemExit(msg + "\n       Re-run (it is intermittent), or pass --allow-degraded.")
        print("    " + msg + " [--allow-degraded]", flush=True)


def normalise(im, size, fill, mode, use_esrgan, allow_degraded=False):
    """Crop to the subject, upscale it, and centre it at a consistent scale on a white plate.

    Scale is normalised on the LONGER SIDE, not bbox area. Equalising area would force every animal
    to the same width x height product, which for a long low wolf (aspect ~2.1) versus a tall front
    view (aspect ~0.4) can only be achieved by DISTORTING one of them — the plan's original
    "bbox area / frame area = 0.80" criterion is unsatisfiable while preserving aspect (issues I10).
    Longer-side normalisation gives every sprite the same on-screen presence with aspect intact."""
    # NATURAL SCALE (`--fill 0`) — reproduce v1's condition, which is the ONE dataset property the
    # shipping model had and every loser lacked. v1 was unnormalised: the subject occupied whatever
    # fraction of its frame the source gave it (measured mean 0.755, sd 0.1478). Normalisation
    # crushed that to sd 0.0032 and run-5's jitter only reached 0.027 — 5.4x short of the shipping
    # condition, so "scale variance doesn't matter" was never tested where it mattered (I1).
    # This keeps the source framing verbatim while still gaining the ESRGAN outline, which is the
    # combination no run has had.
    if fill <= 0:
        a = im.convert("RGBA")
        plate = Image.new("RGBA", a.size, (255, 255, 255, 255)); plate.alpha_composite(a)
        rgb = plate.convert("RGB")
        if use_esrgan and size > max(rgb.size):
            try:
                while max(rgb.size) < size:
                    rgb = esrgan(rgb)
            except Exception as e:
                if not allow_degraded:
                    raise SystemExit(f"prep: ESRGAN failed on this image ({e}). "
                                     f"Refusing to mix upscalers; see --allow-degraded / --upscale lanczos.")
                print(f"    esrgan failed ({e}); LANCZOS for this one [--allow-degraded]", flush=True)
        out = rgb.resize((size, size), Image.LANCZOS)
        # The upscale fails INTERMITTENTLY on the same input (I2), so a collapse is worth retrying
        # before aborting a 701-image build. Bounded, and it still aborts if the retries do not take.
        for attempt in range(3):
            try:
                _assert_subject_preserved(im, out, allow_degraded); break
            except SystemExit:
                if attempt == 2: raise
                print(f"    subject collapsed; retrying upscale ({attempt+1}/2)", flush=True)
                r = plate.convert("RGB")
                while max(r.size) < size: r = esrgan(r)
                out = r.resize((size, size), Image.LANCZOS)
        return out

    a = im.convert("RGBA")
    # THRESHOLD the alpha before taking the bbox. Two traps here, both measured:
    #   * Image.getbbox() on RGBA calls a pixel non-zero if ANY channel is, so a colour-in-the-
    #     transparent-margin sprite reports the whole frame.
    #   * the corpus's "transparent" background is not alpha 0 but alpha ~3, so even the alpha
    #     channel's own getbbox() still returns the whole frame.
    # Either one silently defeats scale normalisation (measured: longer-side fraction 0.70 +/- 0.08
    # where a flat 0.85 was intended). ALPHA_MIN is the cut between background haze and real pixels.
    bb = a.getchannel("A").point(lambda v: 255 if v > ALPHA_MIN else 0).getbbox()
    if bb: a = a.crop(bb)
    w, h = a.size
    target = max(1, int(round(size * fill)))          # longer side lands here
    sc = target / float(max(w, h))
    nw, nh = max(1, int(round(w * sc))), max(1, int(round(h * sc)))

    plate = Image.new("RGBA", a.size, (255, 255, 255, 255)); plate.alpha_composite(a)
    rgb = plate.convert("RGB")
    if use_esrgan and sc > 1.0:                       # only upscale when actually enlarging
        try:
            while rgb.size[0] < nw and rgb.size[1] < nh:
                rgb = esrgan(rgb)
        except Exception as e:
            # A per-image miss is the NASTIER half of the old silent fallback: it yields a set that
            # is part ESRGAN and part LANCZOS, with nothing on disk recording which. Fail fast so
            # the corpus is never half-and-half; `--allow-degraded` opts back into the old mixing.
            if not allow_degraded:
                raise SystemExit(
                    f"prep: ESRGAN failed on this image ({e}).\n"
                    f"       Refusing to mix ESRGAN and LANCZOS in one training set — the upscaler is a\n"
                    f"       property of the corpus, and a mixed set is unrecoverable after the fact.\n"
                    f"       Fix the box, or pass --upscale lanczos (whole set) / --allow-degraded (tolerate)."
                )
            print(f"    esrgan failed ({e}); LANCZOS for this one [--allow-degraded]", flush=True)
    rgb = rgb.resize((nw, nh), Image.LANCZOS)         # final exact fit (down-sample from ESRGAN)

    out = Image.new("RGB", (size, size), (255, 255, 255))
    out.paste(rgb, ((size - nw) // 2, (size - nh) // 2))
    return out

def main():
    ap = argparse.ArgumentParser(prog="prep_train")
    ap.add_argument("--upscale", choices=["esrgan", "lanczos"], default="esrgan")
    ap.add_argument("--fill", type=float, default=0.85,
                    help="subject's longer side as a fraction of the frame. 0 = NATURAL: keep the "
                         "source framing verbatim (v1's condition, measured sd 0.148 — see I1).")
    ap.add_argument("--jitter", type=float, default=0.05,
                    help="+/- range around --fill, deterministic per source file; 0 pins it (which "
                         "taught run-4 to draw a margin, issues I3)")
    ap.add_argument("--size", type=int, default=SIZE)
    ap.add_argument("--dst", default=DST)
    ap.add_argument("--limit", type=int, default=0, help="stop after N images (smoke test)")
    ap.add_argument("--allow-degraded", action="store_true",
                    help="permit the LANCZOS fallback when ESRGAN is unavailable or misses an image. "
                         "OFF by default: a silently-degraded or half-and-half training set is "
                         "unrecoverable after the fact (I2).")
    args = ap.parse_args()

    use_esrgan = args.upscale == "esrgan"
    if use_esrgan:
        # PRE-FLIGHT, and it is a HARD GATE (I2). The upscaler is a property of the corpus, so
        # "ComfyUI happened to be down" must never silently become "we trained on a different
        # dataset". Opting out is explicit and recorded in the command line.
        try:
            urllib.request.urlopen(COMFY + "/system_stats", timeout=8)
        except Exception as e:
            if not args.allow_degraded:
                raise SystemExit(
                    f"prep: ComfyUI unreachable at {COMFY} ({e}).\n"
                    f"       REFUSING to silently build a LANCZOS training set — outline sharpness\n"
                    f"       measures 43.1 that way versus 90.1 with ESRGAN, and nothing downstream\n"
                    f"       can tell which one it got.\n"
                    f"       Start ComfyUI, or choose explicitly:\n"
                    f"         --upscale lanczos    build the whole set with LANCZOS, on purpose\n"
                    f"         --allow-degraded     fall back per-image where ESRGAN misses"
                )
            print(f"prep: ComfyUI unreachable ({e}); LANCZOS for the whole set [--allow-degraded]")
            use_esrgan = False

    dst = args.dst if os.path.isabs(args.dst) else os.path.join(REPO, args.dst)
    if os.path.isdir(dst): shutil.rmtree(dst)
    cls = os.path.join(dst, "1_animal"); os.makedirs(cls)
    n_img = n_txt = 0
    t0 = time.time()
    for png in sorted(glob.glob(os.path.join(SRC, "*", "*.png"))):
        folder = os.path.basename(os.path.dirname(png))
        stem = os.path.basename(png)[:-4]
        name = f"{folder}__{stem}"
        f = jittered_fill(args.fill, args.jitter, os.path.basename(png))
        normalise(Image.open(png), args.size, f, args.upscale, use_esrgan,
                  args.allow_degraded).save(
            os.path.join(cls, name + ".png"))
        n_img += 1
        txt = png[:-4] + ".txt"
        if os.path.exists(txt):
            shutil.copy2(txt, os.path.join(cls, name + ".txt")); n_txt += 1
        if n_img % 25 == 0:
            print(f"  {n_img} ... ({time.time()-t0:.0f}s)", flush=True)
        if args.limit and n_img >= args.limit: break

    print(f"train set -> {cls}")
    print(f"  upscale={'esrgan' if use_esrgan else 'lanczos'}  fill={args.fill}  size={args.size}")
    print(f"  images: {n_img}   captions: {n_txt}   ({time.time()-t0:.0f}s)")
    if not args.limit: assert n_img == n_txt, f"MISMATCH: {n_img} images vs {n_txt} captions"

if __name__ == "__main__":
    main()
