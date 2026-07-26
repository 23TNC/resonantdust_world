#!/usr/bin/env python3
"""P0 baseline — score a fixed species x direction matrix in one control mode.

Answers the question the template-free stream is built on: how good is txt2img+LoRA with NO
control art, measured against the SAME sprites generated through the templated pipeline?
Both modes write the same CSV shape so they can be diffed directly.

  --mode none      txt2img + LoRA, no template read, no ControlNet
  --mode template  the real generate.py pipeline driven by --template-kind's art

Every row carries the raw metrics plus the corpus reference for that species+direction, so the
pass-gate can be set afterwards from data rather than guessed.

  python3 bin/lib/tf_baseline.py --mode none --out .staging/tf-baseline
"""
import argparse, csv, io, os, sys
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import lora_eval as L

REPO = L.REPO

# name, family, corpus folder, corpus stem, prompt
SPECIES = [
    ("wolf",   "canine",  "Wolf_Timber", "Wolf_Timber", "grey timber wolf, yellow eyes"),
    ("tiger",  "feline",  "Tiger",       "Tiger",       "orange tiger with black stripes"),
    ("bear",   "bear",    "Bear",        "Bear",        "large brown bear, thick shaggy fur"),
    ("cat",    "feline",  "Cat",         "Cat",         "domestic cat, short fur"),
    ("jaguar", "feline",  "AEXP_Jaguar", "Jaguar",      "spotted jaguar, golden coat, black rosettes"),
    ("fox",    "canine",  "Fox_Red",     "Fox_Red",     "red fox, white chest, bushy tail"),
    ("cougar", "feline",  "Cougar",      "Cougar",      "tan cougar mountain lion"),
    ("pig",    "pig",     "Pig",         "Pig",         "pink domestic pig"),
    ("horse",  "equine",  "Horse",       "Horse1",      "brown horse with a black mane"),
    ("zebra",  "equine",  "AEXP_Zebra",  "Zebra",       "zebra with black and white stripes"),
]
STYLE = ("rd_style, rd_animal, rd_quadruped, {face}, single creature, full body, white background")

def main():
    ap = argparse.ArgumentParser(prog="tf_baseline")
    ap.add_argument("--mode", choices=["none", "template"], required=True)
    ap.add_argument("--lora", default="rd_quadruped_e07.safetensors")
    ap.add_argument("--lora-strength", type=float, default=0.85)
    ap.add_argument("--template-kind", default="pawn/animal/wolf", help="--mode template: whose art drives ControlNet")
    ap.add_argument("--seed", type=int, default=9100)
    ap.add_argument("--seeds", type=int, default=1,
                    help="rolls per cell; >1 turns each species+direction into a RATE rather than a "
                         "single roll (the predecessor's single-seed baseline reported a failure rate "
                         "as if it were a species verdict)")
    ap.add_argument("--size", type=int, default=768)
    ap.add_argument("--cfg", type=float, default=6.0)
    ap.add_argument("--out", default=".staging/tf-baseline")
    args = ap.parse_args()

    import generate as G
    G.LORA, G.LORA_STRENGTH, G.STYLE = args.lora, args.lora_strength, STYLE

    out = os.path.join(REPO, args.out, args.mode); os.makedirs(out, exist_ok=True)
    tpl_cache = {}
    rows = []
    for name, fam, folder, stem, prompt in SPECIES:
        for d in ("e", "s", "n"):
          for k in range(max(1, args.seeds)):
            seed = args.seed + k
            pos = f"{prompt}, {STYLE.format(face=G.FACE[d])}"
            if args.mode == "none":
                raw = L._run(L.graph(pos, args.lora, args.lora_strength, args.cfg, seed, size=args.size))
            else:
                if d not in tpl_cache:                       # upload the template art once per dir
                    t = G.load_template(args.template_kind.strip("/"), d, "0", "0")
                    tpl_cache[d] = (G._upload(t, f"base_{d}_ref.png"),
                                    G._upload(G.edge_map(t, 30), f"base_{d}_edge.png"))
                ref_name, edge_name = tpl_cache[d]
                raw = G._run(G.graph_hero(pos, G.GENERIC_NEG, ref_name, edge_name, seed))
            im = Image.open(io.BytesIO(raw)).convert("RGB")
            im.save(os.path.join(out, f"{name}_{d}" + (f"_s{seed}" if args.seeds > 1 else "") + ".png"))

            m = L.measure(im)
            r = L.reference(folder, stem, L.DIRS[d][2])
            gate_ok = (m["blobs"] == 1 and m["bg_uni"] >= 0.75 and
                       (not r or 100*abs(m["aspect"]-r["aspect"])/max(r["aspect"],1e-3) <= 50.0))
            row = dict(species=name, family=fam, dir=d, mode=args.mode, seed=seed, gate=int(gate_ok),
                       blobs=m["blobs"], bg=round(m["bg"], 3), fill=round(m["fill"], 4),
                       aspect=round(m["aspect"], 3), solidity=round(m["solidity"], 3),
                       ref_fill=round(r["fill"], 4) if r else "",
                       ref_aspect=round(r["aspect"], 3) if r else "",
                       d_aspect_pct=round(100*abs(m["aspect"]-r["aspect"])/max(r["aspect"],1e-3), 1) if r else "",
                       d_fill_pct=round(100*abs(m["fill"]-r["fill"])/max(r["fill"],1e-3), 1) if r else "",
                       score=round(L.score(m, r), 1))
            rows.append(row)
            print(f"  {name:<7}{d}  blobs={row['blobs']} bg={row['bg']:.2f} asp={row['aspect']:.2f}"
                  f" (ref {row['ref_aspect']}) d_asp={row['d_aspect_pct']}% score={row['score']}")

    csv_path = os.path.join(REPO, args.out, f"scores-{args.mode}.csv")
    with open(csv_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
    sc = [r["score"] for r in rows]
    print(f"\nmode={args.mode}  rows={len(rows)}  mean score={np.mean(sc):.1f}  median={np.median(sc):.1f}")
    if args.seeds > 1:                      # per-cell PASS RATE, the point of multi-seed
        print("per-cell gate pass rate:")
        for name, _f, _fo, _st, _p in SPECIES:
            cells = []
            for d in ("e", "s", "n"):
                sub = [r for r in rows if r["species"] == name and r["dir"] == d]
                cells.append(f"{d} {sum(x['gate'] for x in sub)}/{len(sub)}")
            print(f"   {name:<8} " + "  ".join(cells))
        print(f"overall gate pass rate: {sum(r['gate'] for r in rows)}/{len(rows)}"
              f" = {100*sum(r['gate'] for r in rows)/len(rows):.0f}%")
    print(f"wrote {os.path.relpath(csv_path, REPO)}")

if __name__ == "__main__":
    main()
