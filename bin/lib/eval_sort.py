#!/usr/bin/env python3
"""Sort `.staging/eval_data/new/` into labelled buckets, capturing WHY.

work/2026-08-08-ns-evaluation P0. A bare good/bad label loses two things the labelling actually
knows, and both matter when scoring a judge:

  the REASON — the user's three east rejects had three different causes: an unkeyed white
    background (deterministic, and a corner-alpha check now finds it), line-art quality, and one
    stray line in a mane. Averaging those into "bad" hides that one is a fixable pipeline defect and
    two are aesthetics.
  BORDERLINE — two of those three were "if pressed I'd take it as acceptable". Scoring a judge as
    WRONG for agreeing with the labeller's own hesitation is a measurement error, not a model error.

Buckets are `<label>-<dir>` (`good-e`, `bad-s`, `borderline-n`) so the existing readers keep working,
and the reason rides in a sidecar `labels.json` keyed by filename — never in the filename, because a
file gets moved by hand and a name would then have to be rewritten.

  art eval-sort                       # show the next unsorted image's stats and the commands
  art eval-sort --list                # counts per bucket, and how many rejects lack a reason
  art eval-sort <file> good
  art eval-sort <file> bad lineart
  art eval-sort <file> borderline anatomy --note "extra line in the mane"

REASONS
  background  the plate was not keyed out / a solid background survived
  lineart     outline or interior strokes wrong — spurious, missing, or malformed
  anatomy     the creature is wrong — missing or extra features, broken face
  pose        wrong facing, wrong stance, breaks the convention
  colour      palette or value wrong for the species
  other       something else; use --note
"""
import argparse, json, os, shutil, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
ROOT = os.path.join(REPO, ".staging", "eval_data")
LABELS = os.path.join(ROOT, "labels.json")
LABEL_SET = ("good", "borderline", "bad")
REASONS = ("background", "lineart", "anatomy", "pose", "colour", "other")


def load():
    return json.load(open(LABELS)) if os.path.exists(LABELS) else {}


def save(d):
    os.makedirs(ROOT, exist_ok=True)
    json.dump(d, open(LABELS, "w"), indent=2, sort_keys=True)


def find(name):
    """Locate a file by basename anywhere under eval_data/, so you can label it after moving it."""
    for b in sorted(os.listdir(ROOT)) if os.path.isdir(ROOT) else []:
        p = os.path.join(ROOT, b, name)
        if os.path.isfile(p): return p
    return None


def direction(name):
    parts = name.split("_")
    return parts[1] if len(parts) > 1 and parts[1] in ("e", "s", "n") else None


def main():
    ap = argparse.ArgumentParser(prog="art eval-sort", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file", nargs="?", help="filename (basename is enough)")
    ap.add_argument("label", nargs="?", choices=LABEL_SET)
    ap.add_argument("reason", nargs="?", choices=REASONS, help="required for bad/borderline")
    ap.add_argument("--note", default=None, help="free text, for the detail a code cannot carry")
    ap.add_argument("--list", action="store_true", help="counts per bucket + reason coverage")
    a = ap.parse_args()

    if not os.path.isdir(ROOT): sys.exit(f"eval-sort: no {os.path.relpath(ROOT, REPO)}")
    labels = load()

    if a.list or not a.file:
        print(f"  {'bucket':16s} {'n':>4s}")
        unsorted = []
        for b in sorted(os.listdir(ROOT)):
            d = os.path.join(ROOT, b)
            if not os.path.isdir(d): continue
            fs = [f for f in os.listdir(d) if f.endswith(".png")]
            if fs: print(f"  {b:16s} {len(fs):4d}")
            if b == "new": unsorted = sorted(fs)
        # A reject with no reason is a row P3 cannot use — surface it rather than let it rot.
        missing = [f for f, v in labels.items()
                   if v.get("label") in ("bad", "borderline") and not v.get("reason")]
        if missing:
            print(f"\n  {len(missing)} reject(s) with NO reason recorded — P3 cannot use these:")
            for f in missing[:10]: print(f"      {f}")
        if unsorted:
            print(f"\n  next unsorted ({len(unsorted)} in new/): {unsorted[0]}")
            print(f"      art eval-sort {unsorted[0]} good")
            print(f"      art eval-sort {unsorted[0]} bad <reason>")
        return

    if not a.label: sys.exit("eval-sort: need a label (good | borderline | bad)")
    name = os.path.basename(a.file)
    src = find(name)
    if not src: sys.exit(f"eval-sort: {name} not found under {os.path.relpath(ROOT, REPO)}")
    d = direction(name)
    if not d: sys.exit(f"eval-sort: cannot read a direction from {name}")
    if a.label in ("bad", "borderline") and not a.reason:
        sys.exit(f"eval-sort: {a.label} needs a reason ({' | '.join(REASONS)})")

    dst_dir = os.path.join(ROOT, f"{a.label}-{d}")
    os.makedirs(dst_dir, exist_ok=True)
    dst = os.path.join(dst_dir, name)
    if os.path.abspath(src) != os.path.abspath(dst):
        shutil.move(src, dst)
    labels[name] = {k: v for k, v in
                    dict(label=a.label, reason=a.reason, note=a.note, dir=d).items() if v}
    save(labels)
    print(f"  {name} -> {a.label}-{d}" + (f"  ({a.reason})" if a.reason else ""))


if __name__ == "__main__":
    main()
