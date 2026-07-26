#!/usr/bin/env python3
"""Build the corpus silhouette bank — control images without authoring any art.

The template's only surviving job is to give ControlNet a silhouette (see the stream README:
at dn=1.0 its latent is destroyed). We already own 459 real, on-model sprites across 132
species x e/s/n in the training corpus — so the corpus IS the template library, at zero
authoring cost and already in the target style.

Writes, per species and direction:
    .staging/silhouette-bank/<Species>/<dir>.png   the sprite flattened on white (the control
                                                   image; edge_map() is applied at use time)

Emitting the *sprite on white* rather than a pre-baked edge map is deliberate: the pipeline's
own `edge_map(rgb, thresh)` must stay the single place edges are derived, so the bank cannot
drift from what generate.py feeds ControlNet, and --edge-thresh keeps working.

  python3 bin/lib/silhouette_bank.py            # build
  python3 bin/lib/silhouette_bank.py --verify   # check parity with generate.edge_map
"""
import argparse, os, sys, glob, json
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
SRC  = os.path.join(REPO, ".staging", "quad-lora-train")
BANK = os.path.join(REPO, ".staging", "silhouette-bank")
DIRW = {"east": "e", "south": "s", "north": "n"}

# family -> representative species folder. The corpus name, so the bank lookup is direct.
# Chosen for a CLEAN, characteristic silhouette of that body plan (the thing being borrowed),
# not for how common the animal is.
FAMILY_REP = {
    "canine":    "Wolf_Timber",
    "feline":    "Tiger",
    "bear":      "Bear",
    "cattle":    "Cow",
    "pig":       "Pig",
    "deer":      "Deer",
    "equine":    "Horse",
    "camel":     "Alpaca",
    "pachyderm": "Elephant",
    "rodent":    "Capybara",
    "primate":   "Gorilla",
}

def build():
    if not os.path.isdir(SRC):
        raise SystemExit(f"silhouette_bank: corpus not found: {SRC}")
    n_sp = n_img = 0
    seen = {}
    for png in sorted(glob.glob(os.path.join(SRC, "*", "*.png"))):
        stem = os.path.basename(png)[:-4]
        if "__" not in stem: continue
        species, _, rest = stem.partition("__")
        d = next((v for k, v in DIRW.items() if rest.endswith("_" + k)), None)
        if d is None: continue
        out_dir = os.path.join(BANK, species); os.makedirs(out_dir, exist_ok=True)
        out = os.path.join(out_dir, f"{d}.png")
        if os.path.exists(out): continue           # first sub-variant wins (e.g. DeerMale vs DeerBaby)
        im = Image.open(png).convert("RGBA")
        bg = Image.new("RGBA", im.size, (255, 255, 255, 255)); bg.alpha_composite(im)
        bg.convert("RGB").resize((512, 512), Image.LANCZOS).save(out)
        seen.setdefault(species, set()).add(d); n_img += 1
    n_sp = len(seen)
    with open(os.path.join(BANK, "families.json"), "w") as f:
        json.dump(FAMILY_REP, f, indent=2, sort_keys=True)
    missing = {s: sorted({"e","s","n"} - ds) for s, ds in seen.items() if len(ds) < 3}
    print(f"bank: {n_sp} species, {n_img} control images -> {os.path.relpath(BANK, REPO)}")
    if missing:
        print(f"  incomplete ({len(missing)}): " + ", ".join(f"{s} missing {''.join(v)}" for s, v in sorted(missing.items())))
    bad = [f for f in FAMILY_REP.values() if f not in seen]
    print(f"  family reps present: {len(FAMILY_REP)-len(bad)}/{len(FAMILY_REP)}" + (f"  MISSING {bad}" if bad else " ✓"))
    return n_sp, n_img

def resolve(spec, d):
    """`corpus:<Species>` / `family:<f>` -> the bank control image (RGB on white), or SystemExit."""
    kind, _, key = spec.partition(":")
    if kind == "family":
        if key not in FAMILY_REP:
            raise SystemExit(f"generate: unknown family '{key}'. valid: {', '.join(sorted(FAMILY_REP))}")
        species = FAMILY_REP[key]
    else:
        species = key
    p = os.path.join(BANK, species, f"{d}.png")
    if not os.path.exists(p):
        raise SystemExit(f"generate: no bank silhouette for {species}/{d} ({p}). "
                         f"run: python3 bin/lib/silhouette_bank.py")
    return Image.open(p).convert("RGB")

def verify():
    """The bank must feed the pipeline's OWN edge_map — prove parity on a sample."""
    import generate as G
    import numpy as np
    ok = True
    for species in ("Wolf_Timber", "Bear", "Tiger"):
        src = sorted(glob.glob(os.path.join(SRC, "*", f"{species}__*_east.png")))
        if not src: print(f"  {species}: no corpus source, skipped"); continue
        im = Image.open(src[0]).convert("RGBA")
        bg = Image.new("RGBA", im.size, (255,255,255,255)); bg.alpha_composite(im)
        direct = G.edge_map(bg.convert("RGB").resize((512,512), Image.LANCZOS), 30)
        viabank = G.edge_map(resolve(f"corpus:{species}", "e"), 30)
        same = np.array_equal(np.asarray(direct), np.asarray(viabank))
        print(f"  {species}: bank edge_map identical to direct edge_map -> {same}")
        ok &= same
    print("PARITY OK" if ok else "PARITY FAILED")
    return ok

if __name__ == "__main__":
    ap = argparse.ArgumentParser(prog="silhouette_bank")
    ap.add_argument("--verify", action="store_true")
    a = ap.parse_args()
    if a.verify: sys.exit(0 if verify() else 1)
    build()

# ---------------------------------------------------------------- shape similarity (auto control)
# FAMILY_REP above is a human's guess about taxonomy; what actually governs whether a control
# silhouette works is SHAPE. With 396 silhouettes in the bank, the right control can be measured
# instead of guessed — see docs/work/2026-07-25-sprite-gen-quality/ I2 (the anteater got Elephant
# because someone typed pachyderm->Elephant).
_PROFILE_CACHE = {}

def _profile(img, grid=32):
    """Coarse occupancy grid + aspect for one silhouette. Deliberately low-res: we are matching
    BODY PLAN (where the mass sits), not texture, and a fine grid would chase irrelevant detail."""
    import numpy as np
    sys.path.insert(0, HERE)
    import lora_eval as L
    m = L._filled(L._mask(img), grid)
    ys, xs = np.where(m)
    if len(xs) < 3: return None
    asp = (xs.max()-xs.min()+1) / float(ys.max()-ys.min()+1)
    return np.asarray(m, dtype=float).ravel(), asp

def _bank_profiles(d):
    """{species: (occupancy, aspect)} for one direction, cached per process."""
    if d in _PROFILE_CACHE: return _PROFILE_CACHE[d]
    out = {}
    for sp in sorted(os.listdir(BANK)):
        p = os.path.join(BANK, sp, f"{d}.png")
        if not os.path.isdir(os.path.join(BANK, sp)) or not os.path.exists(p): continue
        pr = _profile(Image.open(p).convert("RGB"))
        if pr: out[sp] = pr
    _PROFILE_CACHE[d] = out
    return out

def nearest(target_img, d, k=5, exclude=()):
    """Top-k bank species whose silhouette for direction `d` is nearest `target_img`.

    Distance = occupancy IoU (where the mass sits) blended with aspect agreement. Returns
    [(species, score)] best first, score in 0..1."""
    import numpy as np
    tp = _profile(target_img)
    if tp is None: return []
    tocc, tasp = tp
    scored = []
    for sp, (occ, asp) in _bank_profiles(d).items():
        if sp in exclude: continue
        inter = float(np.minimum(tocc, occ).sum()); union = float(np.maximum(tocc, occ).sum())
        iou = inter / union if union else 0.0
        agree = 1.0 - min(1.0, abs(asp - tasp) / max(tasp, 1e-3))
        scored.append((sp, 0.7 * iou + 0.3 * agree))
    scored.sort(key=lambda t: -t[1])
    return scored[:k]

def nearest_to_species(species, d, k=5):
    """Top-k bank entries nearest a bank species' own silhouette (self excluded)."""
    p = os.path.join(BANK, species, f"{d}.png")
    if not os.path.exists(p): raise SystemExit(f"silhouette_bank: no bank entry {species}/{d}")
    return nearest(Image.open(p).convert("RGB"), d, k=k, exclude=(species,))
