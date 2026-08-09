#!/usr/bin/env python3
"""Generate sprites into `.staging/eval_data/new/` for hand-sorting into good/ and bad/.

Builds the labelled set an automatic evaluator can be trained or calibrated against. Every image is
produced by the REAL pipeline (a control image, ControlNet, the LoRA) rather than bare txt2img —
which is the whole point: judging the LoRA on unconstrained output is what
work/2026-08-08-east-pipeline found we had been doing by accident.

  art eval-data                              # 6 seeds x e,s,n through S2a = 18 images
  art eval-data --count 20
  art eval-data --dirs e                     # east only
  art eval-data --species bear --method S1
  art eval-data --cn 0.35 --lora-strength 0.85     # widen beyond what the seed can give you
  art eval-data --gen-size 0                       # generate at the control's native 512 (the old behaviour)

GENERATION RESOLUTION defaults to 768 — the resolution the LoRA was TRAINED at (ns-evaluation F5).
The control image is the i2i latent, so it sets what the base actually draws at. Measured on
wolf-south over 3 seeds: **512** gives a splayed white mask (12/12 rejected by hand), **768** gives a
proper face and stays closest to the corpus proportion, **1024** also gives a face but drifts busier
— spikier fur, more interior strokes — because 1024 is native for the BASE while 768 is the only
resolution the LORA has seen. ~2.25x the compute of 512.

Each seed is ONE pipeline call covering every requested direction, so south and north anchor on the
east hero the way the shipping pipeline does.

Files are named `<species>_<dir>_<method>_<lora>_<seed>.png`, so provenance survives being moved
into good/ or bad/ by hand. **Only `new/` is created or written.** good/ and bad/ are yours; the
script reads them to avoid re-offering a seed you already judged, and never creates or touches them.
NOTHING is deleted or overwritten: a seed present in ANY folder under eval_data/ is skipped — the
script scans them all, whatever you name them — so re-running after sorting will not resurrect what
you threw away.

ON SEED VARIATION. Seeds are drawn at random from the full 32-bit range, not counted upward. But be
aware of what the seed can and cannot move: at `cn 0.5` the control pins the silhouette, and the
measured shape IoU between seeds is ~0.65 (against ~0.33 with the looser authored template). Colour
and shading vary freely; POSE does not. If you want genuinely different shapes, lower --cn or raise
--lora-strength; the seed alone will not do it.
"""
import argparse, os, random, subprocess, sys, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
ROOT = os.path.join(REPO, ".staging", "eval_data")
# `new` is the only folder this tool WRITES. Every other folder under eval_data/ is yours — the
# script reads them all to avoid re-offering a seed you already judged, and never creates, moves or
# deletes anything in them. Scanned dynamically rather than from a fixed list, because the sorting
# taxonomy is yours to invent: a hardcoded ("new","good","bad") missed folders named 0-e/1-e/reject-e
# and would have cheerfully regenerated everything already sorted.
NEW = "new"

# Defaults track work/2026-08-08-east-pipeline's measured best: S2a = the family representative's
# silhouette as the ControlNet source, which beat the authored wolf template 0.829 vs 0.731.
METHODS = {
    "S0":  ("template", None),
    "S1":  ("corpus:{corpus}", None),
    "S2a": ("family:{family}", None),
    "S2b": ("family:{family}", 0.20),      # + silhouette stage; measured WORSE, kept for contrast
}
SPECIES = {
    #  id         corpus sprite      family       prompt
    "wolf":     ("Wolf_Timber",    "canine",    "grey timber wolf"),
    "bear":     ("AEXP_BlackBear", "bear",      "black bear"),
    "fox":      ("Fox_Red",        "canine",    "red fox"),
    "deer":     ("Deer",           "deer",      "deer"),
    "elephant": ("Elephant",       "pachyderm", "elephant"),
    "tiger":    ("Tiger",          "feline",    "tiger"),
}


def taken_seeds(species, method):
    """Seeds already used for this species+method in ANY folder under eval_data/ — including
    whatever you have named your reject bucket, so an image you threw away is never silently
    regenerated and re-offered. Direction is deliberately ignored: one seed produces the whole e/s/n
    set, so the seed is spent once you have judged any of them."""
    seen = set()
    for b in sorted(os.listdir(ROOT)) if os.path.isdir(ROOT) else []:
        d = os.path.join(ROOT, b)
        if not os.path.isdir(d): continue
        for f in os.listdir(d):
            if not f.endswith(".png"): continue
            parts = f[:-4].split("_")
            if len(parts) < 4: continue
            if parts[0] == species and method in parts:
                try: seen.add(int(parts[-1]))
                except ValueError: pass
    return seen


def main():
    ap = argparse.ArgumentParser(prog="art eval-data", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--count", type=int, default=6, help="how many to generate (default 6)")
    ap.add_argument("--species", default="wolf", choices=sorted(SPECIES))
    ap.add_argument("--dirs", default="e,s,n",
                    help="directions per seed (default e,s,n). Generated in ONE pipeline call, so south and north anchor on the east hero exactly as the shipping pipeline does — generating them separately would not.")
    ap.add_argument("--method", default="S2a", choices=sorted(METHODS),
                    help="control source (default S2a: the family representative's silhouette)")
    ap.add_argument("--lora", default="rd_diremph_anima_r20_g07.safetensors")
    ap.add_argument("--lora-strength", type=float, default=0.7)
    ap.add_argument("--cn", type=float, default=0.5, help="ControlNet strength; LOWER to let the shape vary between seeds")
    ap.add_argument("--cn-end", type=float, default=0.9)
    ap.add_argument("--size", type=int, default=512, help="saved sprite size")
    ap.add_argument("--gen-size", type=int, default=768,
                    help="GENERATION resolution (default 768 — the LoRA's training resolution). The control image becomes the i2i latent, so this is what the base actually draws at. The bank is 512 and SDXL is native at 1024 — measured on wolf-south: 512 mangles the frontal face, 768 fixes it and stays closest to the corpus, 1024 fixes it but drifts busier. Pass 0 to keep the control's own size.")
    ap.add_argument("--frame-fill", type=float, default=0.0,
                    help="crop the control to its subject and re-square it (try 0.06). Measured NOT to help on south, and it drags the source plate's border into frame; off by default.")
    ap.add_argument("--seed-min", type=int, default=1)
    ap.add_argument("--seed-max", type=int, default=2**31 - 1)
    ap.add_argument("--seeds", default=None, help="comma-separated explicit seeds instead of random ones")
    ap.add_argument("--dry-run", action="store_true", help="print the seeds and commands, generate nothing")
    a = ap.parse_args()

    os.makedirs(os.path.join(ROOT, NEW), exist_ok=True)   # ONLY new/; good and bad are yours

    corpus, family, positive = SPECIES[a.species]
    control_tpl, stage1 = METHODS[a.method]
    control = control_tpl.format(corpus=corpus, family=family)
    lora_tag = a.lora.replace(".safetensors", "").replace("rd_diremph_anima_", "").replace("rd_", "")

    dirs = [d.strip() for d in a.dirs.split(",") if d.strip()]
    bad = [d for d in dirs if d not in ("e", "s", "n")]
    if bad: sys.exit(f"eval-data: unknown direction(s) {bad}")
    used = taken_seeds(a.species, a.method)
    if a.seeds:
        seeds = [int(s) for s in a.seeds.split(",") if int(s) not in used]
    else:
        rng = random.Random()
        seeds, guard = [], 0
        while len(seeds) < a.count and guard < a.count * 200:
            s = rng.randint(a.seed_min, a.seed_max); guard += 1
            if s not in used and s not in seeds: seeds.append(s)

    print(f"eval-data: {a.species} [{','.join(dirs)}] via {a.method} (control {control}) "
          f"lora={lora_tag} str={a.lora_strength} cn={a.cn}/{a.cn_end} "
          f"gen={a.gen_size or 'control-native'}" + (f" frame-fill={a.frame_fill}" if a.frame_fill else ""))
    print(f"  {len(used)} seed(s) already sorted or pending; generating {len(seeds)} seed(s) "
          f"x {len(dirs)} direction(s) = {len(seeds)*len(dirs)} image(s)")
    if a.dry_run:
        print("  seeds:", ", ".join(str(s) for s in seeds)); return

    to = f"_evaldata/{a.method}/{a.species}"
    made, attempted = 0, 0
    for s in seeds:
        # ONE call for the whole direction set. generate.py renders east first and anchors south and
        # north on it through the IP-Adapter; issuing three separate --dir calls would still find the
        # hero on disk, but only if east happened to run first and succeed. Asking for them together
        # is what the shipping pipeline does, so the eval data matches what gets judged.
        cmd = [sys.executable, os.path.join(HERE, "generate.py"),
               "--from", "pawn/animal/wolf", "--to", to,
               "--positive", positive, "--dirs", ",".join(dirs), "--seed", str(s),
               "--lora", a.lora, "--lora-strength", str(a.lora_strength),
               "--cn", str(a.cn), "--cn-end", str(a.cn_end), "--size", str(a.size),
               "--control", control, "--body-plan", "rd_quadruped", "--no-metrics"]
        if stage1:          cmd += ["--stage1-cn", str(stage1)]
        if a.gen_size:      cmd += ["--gen-size", str(a.gen_size)]
        if a.frame_fill > 0: cmd += ["--frame-fill", str(a.frame_fill)]
        r = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                           env={**os.environ, "RD_REPO_ROOT": REPO})
        if r.returncode != 0:
            print(f"  seed {s}: FAILED\n{r.stderr.strip()[-400:]}", file=sys.stderr); continue
        for d in dirs:
            attempted += 1
            # Resolve where it landed: the gate quarantines a whole leaf into _rejected/ AFTER
            # printing the path, so the reported location is not safe to trust. A rejected sprite is
            # still WANTED here — it is exactly the kind of image `bad/` is for.
            leaf = os.path.join(REPO, "textures", to, str(s))
            src = os.path.join(leaf, f"sprite.{d}.0.png")
            if not os.path.exists(src):
                alt = os.path.join(os.path.dirname(leaf), "_rejected", str(s), f"sprite.{d}.0.png")
                src = alt if os.path.exists(alt) else None
            if not src:
                print(f"  seed {s} {d}: no sprite on disk", file=sys.stderr); continue
            gtag = f"g{a.gen_size}_" if a.gen_size else ""
            dst = os.path.join(ROOT, NEW, f"{a.species}_{d}_{a.method}_{lora_tag}_{gtag}{s}.png")
            shutil.copy2(src, dst); made += 1
            print(f"  -> {os.path.relpath(dst, REPO)}")
    print(f"eval-data: {made}/{attempted} written to {os.path.relpath(os.path.join(ROOT, NEW), REPO)}")


if __name__ == "__main__":
    main()
