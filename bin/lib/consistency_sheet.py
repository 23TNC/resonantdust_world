#!/usr/bin/env python3
"""Compare consistency labels — one table and one contact sheet.

work/2026-08-08-direction-consistency. Every phase produces a label
(`baseline`, `prose`, `east-anchor`, `qwen`, …); this puts them side by side on the same subjects
and seeds so a change is judged against its predecessor rather than against memory.

  python3 bin/lib/consistency_sheet.py baseline prose
  python3 bin/lib/consistency_sheet.py baseline prose --subject wolf --seed 9101

F3 of the predecessor holds: the eye ratifies, the metric is the tiebreak. This prints BOTH and
never picks a winner.
"""
import argparse, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
from PIL import Image, ImageDraw
import consistency_eval as CE


def load(label, out):
    p = os.path.join(REPO, out, label, "results.json")
    if not os.path.exists(p): sys.exit(f"consistency_sheet: no results for '{label}' at {p}")
    return json.load(open(p))


def table(labels, data, ceiling):
    keys = sorted({(r["subject"], r["seed"]) for d in data.values() for r in d["results"]})
    w = max(len(l) for l in labels) + 2
    print(f"  {'subject':10s} {'seed':>5s} " + "".join(f"{l:>{w}s}" for l in labels))
    for subj, seed in keys:
        cells = []
        for l in labels:
            r = next((r for r in data[l]["results"] if r["subject"] == subj and r["seed"] == seed), None)
            cells.append(f"{r['spread']:.1f}" if r and r["spread"] is not None else "-")
        print(f"  {subj:10s} {seed:5d} " + "".join(f"{c:>{w}s}" for c in cells))
    print()
    for l in labels:
        sp = [r["spread"] for r in data[l]["results"] if r["spread"] is not None]
        if not sp: continue
        lums = [v["lum"] for r in data[l]["results"] for v in r["dirs"].values()]
        lo, hi = data[l]["config"]["lum_band"]
        inband = sum(1 for x in lums if lo <= x <= hi)
        print(f"  {l:>{w}s}: mean={sum(sp)/len(sp):5.1f}  worst={max(sp):5.1f}  "
              f"{sum(1 for s in sp if s <= ceiling)}/{len(sp)} clear {ceiling}  "
              f"| lum mean={sum(lums)/len(lums):5.1f}  in-band {inband}/{len(lums)}")


def sheet(labels, data, out, subject, seed, path):
    """Rows = labels, columns = directions, for ONE subject+seed."""
    dirs = data[labels[0]]["config"]["dirs"]
    S, PAD, LBL = 320, 6, 22
    W = PAD + len(dirs) * (S + PAD)
    H = LBL + PAD + len(labels) * (S + LBL + PAD)
    im = Image.new("RGB", (W, H), (24, 24, 28)); d = ImageDraw.Draw(im)
    for x, dd in enumerate(dirs):
        d.text((PAD + x * (S + PAD) + 4, 5), {"e": "EAST", "s": "SOUTH", "n": "NORTH"}[dd], fill=(230, 230, 230))
    for y, l in enumerate(labels):
        top = LBL + PAD + y * (S + LBL + PAD)
        r = next((r for r in data[l]["results"] if r["subject"] == subject and r["seed"] == seed), None)
        sp = f"spread={r['spread']:.1f}" if r and r["spread"] is not None else "spread=-"
        d.text((PAD, top - 1), f"{l}   {sp}", fill=(150, 210, 255) if y else (170, 170, 175))
        cfg = data[l]["config"]
        for x, dd in enumerate(dirs):
            p = os.path.join(REPO, "textures", "_consistency", l, subject, str(seed), f"sprite.{dd}.0.png")
            p, _ = CE.resolve_written(p)
            box = (PAD + x * (S + PAD), top + LBL)
            if not p:
                d.rectangle([box[0], box[1], box[0] + S, box[1] + S], fill=(60, 30, 30)); continue
            sp_im = Image.open(p).convert("RGBA")
            bg = Image.new("RGBA", sp_im.size, (255, 255, 255, 255)); bg.alpha_composite(sp_im)
            im.paste(bg.convert("RGB").resize((S, S), Image.LANCZOS), box)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    im.save(path); return path


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("labels", nargs="+", help="labels to compare, in order")
    ap.add_argument("--out", default=".staging/consistency")
    ap.add_argument("--subject", default=None, help="subject for the contact sheet (default: the worst-spread subject of the FIRST label)")
    ap.add_argument("--seed", type=int, default=None, help="seed for the contact sheet")
    ap.add_argument("--sheet", default=None, help="output png (default .staging/consistency-<labels>.png)")
    a = ap.parse_args()

    data = {l: load(l, a.out) for l in a.labels}
    ceiling = data[a.labels[0]]["config"]["lum_spread_ceiling"]
    table(a.labels, data, ceiling)

    # Default the sheet to the FIRST label's worst cell — the one a change most needs to fix.
    subject, seed = a.subject, a.seed
    if subject is None or seed is None:
        worst = max((r for r in data[a.labels[0]]["results"] if r["spread"] is not None),
                    key=lambda r: r["spread"])
        subject = subject or worst["subject"]; seed = seed or worst["seed"]
        print(f"\n  sheet cell -> {subject} {seed} (worst spread in '{a.labels[0]}')")
    path = a.sheet or os.path.join(REPO, ".staging", f"consistency-{'-vs-'.join(a.labels)}.png")
    print(f"  sheet -> {os.path.relpath(sheet(a.labels, data, a.out, subject, seed, path), REPO)}")


if __name__ == "__main__":
    main()
