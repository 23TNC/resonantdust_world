#!/usr/bin/env python3
"""Score east sprites against GROUND TRUTH — work/2026-08-08-east-pipeline.

The set lives in `eval_east.json` (override with RD_EAST_SET). Every species in it has a real
corpus sprite, so the primary metric is `iou_ref`: the generated silhouette against **the real
animal**, never against the control image the generator was handed. That distinction is the whole
point — a wolf-shaped bear obeys its control perfectly and `iou_control` rewards it (I3).

  python3 bin/lib/east_eval.py --method S0                  # wolf template (the control)
  python3 bin/lib/east_eval.py --method S1                  # each species' own corpus silhouette
  python3 bin/lib/east_eval.py --method S3                  # probe -> nearest bank body plan
  python3 bin/lib/east_eval.py --method REAL                # score the real sprites (self-check)
  python3 bin/lib/east_eval.py --method S1 --measure-only   # re-measure from disk
  python3 bin/lib/east_eval.py --compare S0 S1              # table + sheet, no generation

It does NOT pick a winner. F4: iou_ref leads, the eye ratifies, and iou_ref is a SILHOUETTE
statistic — a correctly shaped tiger with no stripes scores perfectly.
"""
import argparse, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import generate as G
import lora_eval as L
from PIL import Image

SET = os.environ.get("RD_EAST_SET", os.path.join(HERE, "eval_east.json"))
OUT_ROOT = os.path.join(REPO, ".staging", "east")


def load_set():
    with open(SET) as f:
        return json.load(f)


def leaf_dir(method, subj_id):
    return os.path.join(REPO, "textures", "_east", method, subj_id)


def resolve_written(p):
    """Where a sprite actually ended up — the gate quarantines a leaf into `_rejected/` AFTER the
    write path is printed, so a path is not safe to trust until the moment it is opened
    (direction-consistency I7, which bit twice)."""
    if os.path.exists(p): return p
    leaf = os.path.dirname(p)
    q = os.path.join(os.path.dirname(leaf), "_rejected", os.path.basename(leaf), os.path.basename(p))
    return q if os.path.exists(q) else None


def control_arg(method, cfg, subj):
    m = cfg["methods"][method]["control"]
    return m.replace("<species>", subj["corpus"]) if "<species>" in m else m


def generate(cfg, method, subj, seed):
    to = f"_east/{method}/{subj['id']}"
    cmd = [sys.executable, os.path.join(HERE, "generate.py"),
           "--from", cfg["template_from"], "--to", to,
           "--positive", subj["positive"], "--dir", cfg["dir"], "--seed", str(seed),
           "--lora", cfg["lora"], "--lora-strength", str(cfg["lora_strength"]),
           "--cn", str(cfg["cn"]), "--cn-end", str(cfg["cn_end"]),
           "--size", str(cfg["size"]), "--body-plan", subj["body_plan"],
           "--control", control_arg(method, cfg, subj), "--no-metrics"]
    r = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                       env={**os.environ, "RD_REPO_ROOT": REPO})
    if r.returncode != 0:
        print(f"    {subj['id']} {seed}: FAILED\n{r.stderr.strip()[-500:]}", file=sys.stderr)
        return None
    picked = None
    for line in r.stdout.splitlines():
        if "auto ->" in line or "control ->" in line: picked = line.strip()
    return picked


def score(img, subj):
    """iou_ref + signed aspect + interior colour, all against the REAL sprite."""
    ref = L.reference_image(subj["corpus"], subj["corpus"], "east")
    if ref is None: return None
    w = Image.new("RGBA", img.size, (255, 255, 255, 255)); w.alpha_composite(img.convert("RGBA"))
    flat = w.convert("RGB")
    m = L.measure(flat)
    rm = L.reference(subj["corpus"], subj["corpus"], "east")
    out = dict(iou_ref=round(L.iou_ref(flat, ref), 3),
               d_aspect=round(L.d_aspect_signed(m, rm), 1) if rm else None,
               blobs=m["blobs"], bg_uni=round(m["bg_uni"], 3))
    im = G.interior_metrics(img)
    if im: out.update(im)
    # The real sprite's own interior, so colour error is a DIFFERENCE and not an absolute to squint at.
    rim = G.interior_metrics(ref_as_rgba(ref))
    if rim and im:
        out["d_lum"] = round(im["lum"] - rim["lum"], 1)
        out["d_sat"] = round(im["sat"] - rim["sat"], 1)
    return out


def ref_as_rgba(ref_rgb):
    """The real sprite carries no alpha (it is composited on white for iou). Rebuild an alpha from
    the non-plate pixels so interior_metrics measures the ANIMAL, not the plate."""
    import numpy as np
    a = np.asarray(ref_rgb).astype(np.uint8)
    lum = 0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]
    alpha = ((lum < 235).astype(np.uint8)) * 255
    return Image.fromarray(np.dstack([a, alpha]), "RGBA")


def run(cfg, method, subjects, seeds, measure_only):
    rows = []
    for subj in subjects:
        for seed in seeds:
            if method == "REAL":
                ref = L.reference_image(subj["corpus"], subj["corpus"], "east")
                s = score(ref_as_rgba(ref), subj) if ref is not None else None
                rows.append(dict(subject=subj["id"], seed=None, **(s or {})))
                break                                   # one row; the real sprite has no seed
            p = os.path.join(leaf_dir(method, subj["id"]), str(seed), f"sprite.e.0.png")
            if not measure_only:
                generate(cfg, method, subj, seed)
            p = resolve_written(p)
            if p is None:
                print(f"    {subj['id']} {seed}: no sprite on disk", file=sys.stderr)
                rows.append(dict(subject=subj["id"], seed=seed)); continue
            rows.append(dict(subject=subj["id"], seed=seed, **(score(Image.open(p), subj) or {})))
    return rows


def table(rows, label):
    ok = [r for r in rows if r.get("iou_ref") is not None]
    print(f"\n  {label}: {len(ok)}/{len(rows)} scored")
    print(f"  {'subject':10s} {'seed':>5s} {'iou_ref':>8s} {'d_aspect':>9s} {'d_lum':>7s} {'d_sat':>7s} {'blobs':>6s}")
    for r in rows:
        if r.get("iou_ref") is None:
            print(f"  {r['subject']:10s} {str(r.get('seed') or '-'):>5s} {'-':>8s}"); continue
        print(f"  {r['subject']:10s} {str(r.get('seed') or '-'):>5s} {r['iou_ref']:8.3f} "
              f"{(r.get('d_aspect') if r.get('d_aspect') is not None else 0):+9.1f} "
              f"{(r.get('d_lum') or 0):+7.1f} {(r.get('d_sat') or 0):+7.1f} {r.get('blobs',0):6d}")
    if ok:
        mi = sum(r["iou_ref"] for r in ok) / len(ok)
        print(f"  {'MEAN':10s} {'':>5s} {mi:8.3f}")
    return ok


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--method", default=None, help="a key in eval_east.json 'methods', or REAL to score the ground-truth sprites")
    ap.add_argument("--compare", nargs="+", default=None, help="compare already-measured methods; no generation")
    ap.add_argument("--subjects", default=None, help="comma-separated subject ids (default: all)")
    ap.add_argument("--seeds", default=None)
    ap.add_argument("--measure-only", action="store_true")
    a = ap.parse_args()

    cfg = load_set()
    subs = cfg["subjects"]
    if a.subjects:
        want = [s.strip() for s in a.subjects.split(",")]
        subs = [s for s in subs if s["id"] in want]
    seeds = [int(s) for s in a.seeds.split(",")] if a.seeds else cfg["seeds"]

    if a.compare:
        data = {}
        for m in a.compare:
            p = os.path.join(OUT_ROOT, m, "results.json")
            if not os.path.exists(p): sys.exit(f"east_eval: no results for '{m}' — run --method {m} first")
            data[m] = json.load(open(p))["rows"]
        keys = sorted({(r["subject"], r.get("seed")) for rs in data.values() for r in rs},
                      key=lambda k: (k[0], k[1] or 0))
        w = max(len(m) for m in a.compare) + 2
        print(f"  {'subject':10s} {'seed':>5s} " + "".join(f"{m:>{w}s}" for m in a.compare) + "   (iou_ref)")
        for subj, seed in keys:
            cells = []
            for m in a.compare:
                r = next((r for r in data[m] if r["subject"] == subj and r.get("seed") == seed), None)
                cells.append(f"{r['iou_ref']:.3f}" if r and r.get("iou_ref") is not None else "-")
            print(f"  {subj:10s} {str(seed or '-'):>5s} " + "".join(f"{c:>{w}s}" for c in cells))
        print()
        for m in a.compare:
            ok = [r for r in data[m] if r.get("iou_ref") is not None]
            if ok: print(f"  {m:>{w}s}: mean iou_ref={sum(r['iou_ref'] for r in ok)/len(ok):.3f}  n={len(ok)}")
        return

    if not a.method: sys.exit("east_eval: pass --method or --compare")
    if a.method != "REAL" and a.method not in cfg["methods"]:
        sys.exit(f"east_eval: unknown method '{a.method}' (have {sorted(cfg['methods'])} + REAL)")

    if a.method != "REAL":
        print(f"east: method={a.method} control={cfg['methods'][a.method]['control']} "
              f"lora={cfg['lora']} cn={cfg['cn']}/{cfg['cn_end']} str={cfg['lora_strength']}")
    rows = run(cfg, a.method, subs, seeds, a.measure_only)
    table(rows, a.method)
    d = os.path.join(OUT_ROOT, a.method); os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "results.json"), "w") as f:
        json.dump(dict(method=a.method, config=cfg, rows=rows), f, indent=2)
    print(f"  results -> {os.path.relpath(os.path.join(d, 'results.json'), REPO)}")


if __name__ == "__main__":
    main()
