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
  4-5x LANCZOS, and unlike NEAREST (107.4) it does not staircase the curves. Falls back to LANCZOS
  automatically if ComfyUI is unreachable, so the prep never hard-fails on a box outage.

  SCALE NORMALISATION (--fill).  Source subjects fill wildly different fractions of their frame
  (a bear nearly fills it, a cat occupies a corner), so the LoRA learned that scale is arbitrary
  and generated scale drifts. Each sprite is now cropped to its alpha bbox and rescaled so its
  LONGER side is a fixed fraction of the frame, then centred. Aspect is preserved — see the note
  in normalise() on why equalising bbox AREA is impossible without distorting animals.

  python3 bin/lib/prep_train.py                      # esrgan + normalise (default)
  python3 bin/lib/prep_train.py --upscale lanczos    # the old behaviour, for A/B
"""
import argparse, io, json, os, glob, shutil, time, uuid
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
def normalise(im, size, fill, mode, use_esrgan):
    """Crop to the subject, upscale it, and centre it at a consistent scale on a white plate.

    Scale is normalised on the LONGER SIDE, not bbox area. Equalising area would force every animal
    to the same width x height product, which for a long low wolf (aspect ~2.1) versus a tall front
    view (aspect ~0.4) can only be achieved by DISTORTING one of them — the plan's original
    "bbox area / frame area = 0.80" criterion is unsatisfiable while preserving aspect (issues I10).
    Longer-side normalisation gives every sprite the same on-screen presence with aspect intact."""
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
            print(f"    esrgan failed ({e}); LANCZOS for this one", flush=True)
    rgb = rgb.resize((nw, nh), Image.LANCZOS)         # final exact fit (down-sample from ESRGAN)

    out = Image.new("RGB", (size, size), (255, 255, 255))
    out.paste(rgb, ((size - nw) // 2, (size - nh) // 2))
    return out

def main():
    ap = argparse.ArgumentParser(prog="prep_train")
    ap.add_argument("--upscale", choices=["esrgan", "lanczos"], default="esrgan")
    ap.add_argument("--fill", type=float, default=0.85, help="subject's longer side as a fraction of the frame")
    ap.add_argument("--size", type=int, default=SIZE)
    ap.add_argument("--dst", default=DST)
    ap.add_argument("--limit", type=int, default=0, help="stop after N images (smoke test)")
    args = ap.parse_args()

    use_esrgan = args.upscale == "esrgan"
    if use_esrgan:
        try:
            urllib.request.urlopen(COMFY + "/system_stats", timeout=8)
        except Exception as e:
            print(f"prep: ComfyUI unreachable ({e}); falling back to LANCZOS"); use_esrgan = False

    dst = args.dst if os.path.isabs(args.dst) else os.path.join(REPO, args.dst)
    if os.path.isdir(dst): shutil.rmtree(dst)
    cls = os.path.join(dst, "1_animal"); os.makedirs(cls)
    n_img = n_txt = 0
    t0 = time.time()
    for png in sorted(glob.glob(os.path.join(SRC, "*", "*.png"))):
        folder = os.path.basename(os.path.dirname(png))
        stem = os.path.basename(png)[:-4]
        name = f"{folder}__{stem}"
        normalise(Image.open(png), args.size, args.fill, args.upscale, use_esrgan).save(
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
