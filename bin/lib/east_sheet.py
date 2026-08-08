#!/usr/bin/env python3
"""Contact sheet for east methods — work/2026-08-08-east-pipeline.

Rows are methods (REAL first if asked for), columns are species. One seed, so the sheet shows what
a single run produces rather than a best-of.

  python3 bin/lib/east_sheet.py REAL S0 S1 --seed 4101
"""
import argparse, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import lora_eval as L, east_eval as E
from PIL import Image, ImageDraw


def cell_image(method, subj, seed):
    if method == "REAL":
        r = L.reference_image(subj["corpus"], subj["corpus"], "east")
        return r.convert("RGB") if r is not None else None
    p = E.resolve_written(os.path.join(E.leaf_dir(method, subj["id"]), str(seed), "sprite.e.0.png"))
    if not p: return None
    im = Image.open(p).convert("RGBA")
    bg = Image.new("RGBA", im.size, (255, 255, 255, 255)); bg.alpha_composite(im)
    return bg.convert("RGB")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("methods", nargs="+")
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--out", default=".staging/east-sheet.png")
    a = ap.parse_args()

    cfg = E.load_set()
    seed = a.seed or cfg["seeds"][0]
    subs = cfg["subjects"]
    scores = {}
    for m in a.methods:
        p = os.path.join(E.OUT_ROOT, m, "results.json")
        if os.path.exists(p):
            for r in json.load(open(p))["rows"]:
                if r.get("seed") in (seed, None):
                    scores[(m, r["subject"])] = r.get("iou_ref")

    S, PAD, LBL = 300, 5, 20
    W = 96 + PAD + len(subs) * (S + PAD)
    H = LBL + PAD + len(a.methods) * (S + PAD)
    im = Image.new("RGB", (W, H), (24, 24, 28)); d = ImageDraw.Draw(im)
    for x, s in enumerate(subs):
        d.text((96 + PAD + x * (S + PAD) + 4, 5), s["id"].upper(), fill=(230, 230, 230))
    for y, m in enumerate(a.methods):
        top = LBL + PAD + y * (S + PAD)
        d.text((6, top + S // 2 - 14), m, fill=(150, 255, 180) if m == "REAL" else (150, 210, 255))
        for x, s in enumerate(subs):
            c = cell_image(m, s, seed)
            box = (96 + PAD + x * (S + PAD), top)
            if c is None:
                d.rectangle([box[0], box[1], box[0] + S, box[1] + S], fill=(60, 30, 30)); continue
            im.paste(c.resize((S, S), Image.LANCZOS), box)
            v = scores.get((m, s["id"]))
            if v is not None:
                d.rectangle([box[0], box[1] + S - 16, box[0] + 62, box[1] + S], fill=(20, 20, 24))
                d.text((box[0] + 4, box[1] + S - 14), f"{v:.3f}", fill=(255, 235, 150))
    out = os.path.join(REPO, a.out)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    im.save(out)
    print(f"  seed {seed} -> {os.path.relpath(out, REPO)}  {im.size}")


if __name__ == "__main__":
    main()
