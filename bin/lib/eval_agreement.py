#!/usr/bin/env python3
"""Score the hand-labelled set and report whether our automatic metrics agree with the human.

The labelled buckets under `.staging/eval_data/` (good-e, bad-s, …) are the ground truth here — not
the corpus. The question is not "is this sprite good" but **"can any metric we already compute tell
good from bad the way you do"**, which is what decides whether generation selection can ever be
automated.

  art eval-agreement                 # all directions
  art eval-agreement --dir s

Reports, per direction and per metric: the mean in each bucket, the separation, and the best
single-threshold accuracy. A metric that cannot beat "always guess the majority class" is not a
ruler, and is reported as such rather than dressed up with a correlation.
"""
import argparse, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import generate as G
import lora_eval as L
from PIL import Image

ROOT = os.path.join(REPO, ".staging", "eval_data")
DIRW = {"e": "east", "s": "south", "n": "north"}
# species id -> the corpus folder holding its real sprite, for iou_ref / d_aspect
CORPUS = {"wolf": "Wolf_Timber", "bear": "AEXP_BlackBear", "fox": "Fox_Red",
          "deer": "Deer", "elephant": "Elephant", "tiger": "Tiger"}
NAME = re.compile(r"^(?P<species>[a-z]+)_(?P<dir>[esn])_")


def score(path):
    m = NAME.match(os.path.basename(path))
    if not m: return None
    species, d = m.group("species"), m.group("dir")
    im = Image.open(path).convert("RGBA")
    w = Image.new("RGBA", im.size, (255, 255, 255, 255)); w.alpha_composite(im)
    flat = w.convert("RGB")
    mm = L.measure(flat)
    out = dict(species=species, dir=d, blobs=mm["blobs"], bg_uni=mm["bg_uni"],
               solidity=mm["solidity"], hull=mm["hull_solidity"], fill=mm["fill"], aspect=mm["aspect"])
    corp = CORPUS.get(species)
    if corp:
        ref = L.reference_image(corp, corp, DIRW[d])
        rm = L.reference(corp, corp, DIRW[d])
        if ref is not None: out["iou_ref"] = L.iou_ref(flat, ref)
        if rm: out["d_aspect"] = abs(L.d_aspect_signed(mm, rm))
    i = G.interior_metrics(im)
    if i: out.update(lum=i["lum"], sat=i["sat"], cov=i["cov"])
    return out


def best_threshold(good, bad):
    """Best accuracy any single threshold on this metric can reach, and which side 'good' is on.

    Brute force over every midpoint — the sets are tens of images, so there is no reason to
    approximate. Returns (accuracy, threshold, direction) where direction is '>' if good is high."""
    vals = sorted(set(good + bad))
    if len(vals) < 2: return None
    n = len(good) + len(bad)
    best = (0.0, None, None)
    for i in range(len(vals) - 1):
        t = (vals[i] + vals[i + 1]) / 2
        hi = (sum(1 for v in good if v > t) + sum(1 for v in bad if v <= t)) / n
        lo = (sum(1 for v in good if v <= t) + sum(1 for v in bad if v > t)) / n
        if hi > best[0]: best = (hi, t, ">")
        if lo > best[0]: best = (lo, t, "<=")
    return best


def main():
    ap = argparse.ArgumentParser(prog="art eval-agreement", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dir", dest="only", default=None, choices=["e", "s", "n"])
    a = ap.parse_args()

    rows = {}
    for bucket in sorted(os.listdir(ROOT)) if os.path.isdir(ROOT) else []:
        b = os.path.join(ROOT, bucket)
        if not os.path.isdir(b) or "-" not in bucket: continue
        label, _, d = bucket.partition("-")
        if label not in ("good", "bad") or d not in DIRW: continue
        if a.only and d != a.only: continue
        for f in sorted(os.listdir(b)):
            if not f.endswith(".png"): continue
            s = score(os.path.join(b, f))
            if s: rows.setdefault(d, []).append((label, s))

    if not rows: sys.exit("eval-agreement: no labelled images found under .staging/eval_data/*-[esn]/")

    METRICS = ["iou_ref", "d_aspect", "blobs", "bg_uni", "solidity", "hull", "fill", "aspect", "lum", "sat", "cov"]
    for d in ("e", "s", "n"):
        if d not in rows: continue
        good = [r for lab, r in rows[d] if lab == "good"]
        bad = [r for lab, r in rows[d] if lab == "bad"]
        n = len(good) + len(bad)
        base = max(len(good), len(bad)) / n        # always-guess-majority
        print(f"\n  {DIRW[d].upper()}  good={len(good)} bad={len(bad)}   "
              f"majority-class baseline = {base:.0%}")
        print(f"    {'metric':9s} {'good':>9s} {'bad':>9s} {'best acc':>9s}  {'rule':>22s}")
        scored = []
        for k in METRICS:
            gv = [r[k] for r in good if k in r]; bv = [r[k] for r in bad if k in r]
            if len(gv) < 2 or len(bv) < 2: continue
            gm, bm = sum(gv)/len(gv), sum(bv)/len(bv)
            bt = best_threshold(gv, bv)
            if not bt: continue
            acc, t, side = bt
            scored.append((acc, k, gm, bm, t, side))
        for acc, k, gm, bm, t, side in sorted(scored, reverse=True):
            lift = acc - base
            mark = "  <-- beats baseline" if lift > 0.001 else ""
            print(f"    {k:9s} {gm:9.3f} {bm:9.3f} {acc:9.0%}  {('good ' + side + ' ' + format(t, '.3f')):>22s}{mark}")


if __name__ == "__main__":
    main()
