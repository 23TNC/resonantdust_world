#!/usr/bin/env python3
"""Run the pinned consistency set and report cross-direction luminance spread.

work/2026-08-08-direction-consistency P0. The set lives in `consistency_set.json` (override with
RD_CONSISTENCY_SET) so two invocations measure the same cells; a number produced on ad-hoc cells is
not comparable to the one before it, which is how this project has been misled before.

Usage:
  python3 bin/lib/consistency_eval.py --label baseline
  python3 bin/lib/consistency_eval.py --label tags --style-form tags --subjects wolf,fox

Writes <out>/<label>/results.json and prints one table. It does NOT judge — F3 of the predecessor
holds: the eye ratifies, the metric is the tiebreak. Build the sheet and look at it.
"""
import argparse, io, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import generate as G                                     # interior_metrics + the ceilings
from PIL import Image

SET = os.environ.get("RD_CONSISTENCY_SET", os.path.join(HERE, "consistency_set.json"))

# Style forms are generate.py's business now (`--style-form`); this harness only selects one.
# The duplicate caption builder that used to live here is deleted rather than deprecated - two
# places to change one caption is how the forms drifted apart in the first place.


def load_set():
    with open(SET) as f:
        return json.load(f)


def resolve_written(p):
    """Where a reported sprite path actually ENDED UP.

    `report_candidates` quarantines a whole variant leaf into `_rejected/<leaf>/` when any one of
    its directions fails the structural gate — and it does so AFTER printing the write path. So the
    path `generate.py` reports can be stale by the time we measure it, which silently drops the cell
    to two directions and biases the baseline toward sets that happened to pass the gate. Look in
    the quarantine before giving up, and tell the caller which it was."""
    if os.path.exists(p): return p, False
    leaf = os.path.dirname(p)
    q = os.path.join(os.path.dirname(leaf), "_rejected", os.path.basename(leaf), os.path.basename(p))
    if os.path.exists(q): return q, True
    return None, False


def cell_paths(cfg, subj, seed, out_root):
    """Sprite paths for one cell, read off DISK — used by --measure-only."""
    leaf = os.path.join(REPO, "textures", "_consistency", out_root, subj["id"], str(seed))
    out = {}
    for d in cfg["dirs"]:
        p, _ = resolve_written(os.path.join(leaf, f"sprite.{d}.0.png"))
        if p: out[d] = p
    return out


def run_cell(cfg, subj, seed, style_form, out_root, extra):
    """Generate one subject at one seed, all directions. Returns {dir: sprite_path}."""
    to = f"_consistency/{out_root}/{subj['id']}"
    paths = {}
    for d in cfg["dirs"]:
        cmd = [sys.executable, os.path.join(HERE, "generate.py"),
               "--from", subj["from"], "--to", to,
               "--positive", subj["positive"], "--dir", d, "--seed", str(seed),
               "--lora", cfg["lora"], "--lora-strength", str(cfg["lora_strength"]),
               "--cn", str(cfg["cn"]), "--cn-end", str(cfg["cn_end"]),
               "--size", str(cfg["size"]), "--no-metrics"]
        # Pass the form THROUGH rather than building the caption here. Until 2026-08-08 this
        # constructed --style itself for 'tags' and passed NOTHING for 'prose' — and generate.py's
        # default is 'tags', so the `prose` label silently ran as tags and produced results
        # bit-identical to `baseline` on all 36 cells. A harness that can only express one of the
        # two things it is meant to compare is worse than no harness.
        cmd += ["--style-form", style_form, "--body-plan", subj["body_plan"]]
        cmd += extra
        r = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                           env={**os.environ, "RD_REPO_ROOT": REPO})
        if r.returncode != 0:
            print(f"    {subj['id']} seed {seed} {d}: FAILED\n{r.stderr.strip()[-600:]}", file=sys.stderr)
            continue
        for line in r.stdout.splitlines():
            if line.strip().startswith("wrote ") and f"sprite.{d}." in line:
                raw = os.path.join(REPO, line.split("wrote ", 1)[1].split()[0])
                p, quarantined = resolve_written(raw)
                if p is None:
                    print(f"    {subj['id']} seed {seed} {d}: reported {raw} but it is not on disk", file=sys.stderr)
                    continue
                if quarantined:
                    print(f"    {subj['id']} seed {seed} {d}: gate-rejected (measured from _rejected/)")
                paths[d] = p
    return paths


def measure_cell(paths):
    out = {}
    for d, p in paths.items():
        if not os.path.exists(p): continue
        m = G.interior_metrics(Image.open(p))
        if m: out[d] = m
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--label", required=True, help="name for this configuration; becomes the output folder")
    ap.add_argument("--style-form", choices=["prose", "tags"], default="tags",
                    help="prose = generate.py's own STYLE boilerplate; tags = the LoRA's trained caption form (default)")
    ap.add_argument("--subjects", default=None, help="comma-separated subject ids (default: all in the set)")
    ap.add_argument("--seeds", default=None, help="comma-separated seeds (default: the set's)")
    ap.add_argument("--out", default=".staging/consistency", help="output root for results.json")
    ap.add_argument("--measure-only", action="store_true",
                    help="do not generate; re-measure the sprites already on disk for this label (includes gate-quarantined leaves)")
    ap.add_argument("rest", nargs=argparse.REMAINDER, help="extra flags passed through to generate.py after --")
    a = ap.parse_args()

    cfg = load_set()
    subs = cfg["subjects"]
    if a.subjects:
        want = [s.strip() for s in a.subjects.split(",")]
        subs = [s for s in subs if s["id"] in want]
        missing = set(want) - {s["id"] for s in subs}
        if missing: sys.exit(f"consistency_eval: unknown subject(s) {sorted(missing)}")
    seeds = [int(s) for s in a.seeds.split(",")] if a.seeds else cfg["seeds"]
    extra = [x for x in a.rest if x != "--"]

    ceiling = cfg["lum_spread_ceiling"]; lo, hi = cfg["lum_band"]
    print(f"consistency: label={a.label} style-form={a.style_form} lora={cfg['lora']} "
          f"cn={cfg['cn']}/{cfg['cn_end']} str={cfg['lora_strength']}")
    print(f"  {len(subs)} subject(s) x {len(seeds)} seed(s) x {len(cfg['dirs'])} dir(s) "
          f"= {len(subs)*len(seeds)*len(cfg['dirs'])} generations")

    results, spreads = [], []
    for subj in subs:
        for seed in seeds:
            paths = (cell_paths(cfg, subj, seed, a.label) if a.measure_only
                     else run_cell(cfg, subj, seed, a.style_form, a.label, extra))
            m = measure_cell(paths)
            if len(m) < 2:
                print(f"  {subj['id']:9s} seed {seed}: only {len(m)} direction(s) — no spread", file=sys.stderr)
                results.append(dict(subject=subj["id"], seed=seed, dirs=m, spread=None))
                continue
            lums = [v["lum"] for v in m.values()]
            spread = round(max(lums) - min(lums), 1)
            spreads.append(spread)
            results.append(dict(subject=subj["id"], seed=seed, dirs=m, spread=spread))
            cells = "  ".join(f"{d}={m[d]['lum']:.0f}" for d in cfg["dirs"] if d in m)
            band = "" if all(lo <= v["lum"] <= hi for v in m.values()) else "  [outside corpus band]"
            print(f"  {subj['id']:9s} seed {seed}: {cells}  spread={spread:5.1f} "
                  f"{'OK ' if spread <= ceiling else 'OVER'}{band}")

    outdir = os.path.join(REPO, a.out, a.label)
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, "results.json"), "w") as f:
        json.dump(dict(label=a.label, style_form=a.style_form, config=cfg,
                       seeds=seeds, results=results), f, indent=2)

    if spreads:
        worst = max(spreads); mean = sum(spreads) / len(spreads)
        print(f"\n  SPREAD over {len(spreads)} set(s): mean={mean:.1f} worst={worst:.1f} "
              f"(corpus ceiling {ceiling})")
        print(f"  {sum(1 for s in spreads if s <= ceiling)}/{len(spreads)} sets clear the ceiling")
    print(f"  results -> {os.path.relpath(os.path.join(outdir, 'results.json'), REPO)}")


if __name__ == "__main__":
    main()
