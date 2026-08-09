#!/usr/bin/env python3
"""The vision QA gate — work/2026-08-08-ns-evaluation P1.

`sprite-gen-plan.md` has specified this all along and it was never built:

    **Claude-vision QA gate** — automated per candidate: "Single creature? Correct facing for this
    frame? Clean silhouette (no extra limbs/heads/duplicates)? On-style (flat, not photoreal)?" →
    score, keep best, retry the frame if none pass. This is what makes batch-and-curate hands-off.

Why a vision model rather than another statistic: every metric this project computes describes the
SILHOUETTE, and at cn 0.5 the ControlNet hands the sprite its silhouette. On north and south the
rejects are geometrically CLOSER to the real animal than the keepers (ns-evaluation I1). The failures
are interior — the face does not resolve — and a model that looks at the image is the only judge we
have that sees interiors. Two hand-built mechanisms have already been falsified by measurement.

  art eval-gate --score-labels          # run over the labelled set, report agreement
  art eval-gate <file.png>              # one image, print the verdict

Needs ANTHROPIC_API_KEY or bin/keys/anthropic.env.
"""
import argparse, base64, io, json, os, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.environ.get("RD_REPO_ROOT") or os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import generate as G
from PIL import Image

ROOT = os.path.join(REPO, ".staging", "eval_data")
DIRW = {"e": "side profile facing RIGHT (east)",
        "s": "front view facing the VIEWER (south)",
        "n": "back view facing AWAY (north)"}

# The doc's four questions, plus the two failure modes the user actually rejected for that the
# doc's list does not name (ns-evaluation I3): an unkeyed background, and line-art quality.
SYSTEM = """You are a QA gate for 2D game creature sprites in a RimWorld / Prison Architect style:
flat colour regions, bold dark outlines, readable when small, on a transparent plate.

You will be shown ONE sprite, composited on a light checker so transparency is visible.

THE CONVENTION — read this before judging, it is NOT naturalistic animal anatomy:
* The body is a SOLID MASS with an UNBROKEN BOTTOM EDGE. Legs are NOT drawn as separate articulated
  limbs. A creature with no visible legs is CORRECT and must PASS. Visible separated legs, a
  standing/upright stance, or a naturalistic pose are REGRESSIONS and should fail `on_style`.
* East is a creature lying low in side profile, long and wide, its head a large profile.
* South and north are TALL NARROW vertical stacks: the tail as a spike at the TOP, then the body,
  then a small head at the BOTTOM (south) or no face at all (north).

Judge ONLY these, and be strict — this gate exists to reject sprites a human artist would reject:
1. single_creature  — exactly one creature, no duplicates, no sprite-sheet grid, no framing/border
2. correct_facing   — matches the stated direction
3. clean_anatomy    — the FEATURES THAT ARE DRAWN resolve. Absent legs are NOT a failure.
                      **This is the check that matters most on SOUTH, and the structure above does
                      NOT satisfy it.** The vertical stack is imposed by the generator, so every
                      south sprite has it — passing a south sprite because "tail spike top, body,
                      face bottom, matches convention" is exactly the mistake. Judge the FACE:
                      • SOUTH must have two clearly readable eyes AND a distinct muzzle or nose.
                        A pale splayed shape, a skull-like mask, a smear, or features that merge
                        into the chest is a FAIL even when the overall stack is perfect.
                      • NORTH must show NO face (it is a back view) but the head must still read as
                        a distinct rounded mass with ears, not dissolve into the body.
                      • EAST must have a readable profile head with one visible eye and a muzzle.
4. on_style         — flat regions, bold outline, not photoreal, not over-detailed, and NOT
                      naturalistic: separated legs or an upright stance fail here
5. clean_lineart    — outlines are deliberate: no stray, doubled or missing strokes
6. keyed_background — the background is transparent, NOT a solid plate

Return ONLY minified JSON, no prose:
{"single_creature":bool,"correct_facing":bool,"clean_anatomy":bool,"on_style":bool,
 "clean_lineart":bool,"keyed_background":bool,"verdict":"good"|"borderline"|"bad",
 "reason":"background"|"lineart"|"anatomy"|"pose"|"colour"|"other"|null,"why":"<=15 words"}

verdict "bad" if any of 1,2,3,6 fails. "borderline" if only 4 or 5 fails, or the flaw is minor
enough that a working artist might ship it. "good" if all pass."""


def checkered(path, size=512):
    """Composite on a checker so the model can SEE transparency — on white, an unkeyed white plate
    is invisible, which is precisely one of the failures being judged."""
    im = Image.open(path).convert("RGBA")
    bg = Image.new("RGBA", im.size, (255, 255, 255, 255))
    d = 32
    for y in range(0, im.size[1], d):
        for x in range(0, im.size[0], d):
            if (x // d + y // d) % 2:
                bg.paste((205, 205, 215, 255), (x, y, min(x + d, im.size[0]), min(y + d, im.size[1])))
    bg.alpha_composite(im)
    out = bg.convert("RGB")
    if out.size[0] > size: out = out.resize((size, size), Image.LANCZOS)
    b = io.BytesIO(); out.save(b, format="PNG")
    return base64.b64encode(b.getvalue()).decode()


def ask(key, path, direction, model="claude-sonnet-5", retries=3):
    body = {"model": model, "max_tokens": 300, "system": SYSTEM,
            "messages": [{"role": "user", "content": [
                {"type": "image", "source": {"type": "base64", "media_type": "image/png",
                                             "data": checkered(path)}},
                {"type": "text", "text": f"Stated direction: {DIRW.get(direction, direction)}. Judge it."}]}]}
    req = urllib.request.Request("https://api.anthropic.com/v1/messages",
        data=json.dumps(body).encode(),
        headers={"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"})
    for attempt in range(retries):
        try:
            r = json.load(urllib.request.urlopen(req, timeout=90))
            txt = "".join(b.get("text", "") for b in r.get("content", [])).strip()
            txt = txt[txt.find("{"): txt.rfind("}") + 1]
            return json.loads(txt)
        except Exception as e:
            if attempt == retries - 1:
                print(f"  gate failed on {os.path.basename(path)}: {e}", file=sys.stderr); return None
            time.sleep(2 * (attempt + 1))


def main():
    ap = argparse.ArgumentParser(prog="art eval-gate", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file", nargs="?")
    ap.add_argument("--score-labels", action="store_true", help="run over every labelled image and report agreement")
    ap.add_argument("--model", default="claude-sonnet-5")
    ap.add_argument("--limit", type=int, default=0, help="cap images (for a cheap smoke run)")
    ap.add_argument("--out", default=".staging/eval_gate.json")
    a = ap.parse_args()

    key = G._anthropic_key()
    if not key: sys.exit("eval-gate: no ANTHROPIC_API_KEY and no bin/keys/anthropic.env")

    if a.file:
        d = os.path.basename(a.file).split("_")[1] if "_" in os.path.basename(a.file) else "e"
        print(json.dumps(ask(key, a.file, d, a.model), indent=2)); return

    if not a.score_labels: sys.exit("eval-gate: pass a file or --score-labels")

    items = []
    for b in sorted(os.listdir(ROOT)) if os.path.isdir(ROOT) else []:
        p = os.path.join(ROOT, b)
        if not os.path.isdir(p) or "-" not in b: continue
        label, _, d = b.partition("-")
        if label not in ("good", "bad", "borderline") or d not in DIRW: continue
        for f in sorted(os.listdir(p)):
            if f.endswith(".png"): items.append((label, d, os.path.join(p, f), f))
    if a.limit: items = items[:a.limit]
    print(f"eval-gate: {len(items)} labelled image(s), model={a.model}")

    t0 = time.time(); rows = []
    for i, (label, d, path, f) in enumerate(items):
        v = ask(key, path, d, a.model)
        rows.append(dict(file=f, dir=d, human=label, gate=v))
        if (i + 1) % 10 == 0: print(f"  {i+1}/{len(items)}  ({(time.time()-t0)/(i+1):.1f}s/img)")
    el = time.time() - t0
    outp = os.path.join(REPO, a.out); os.makedirs(os.path.dirname(outp), exist_ok=True)
    json.dump(rows, open(outp, "w"), indent=2)

    # Agreement. `borderline` is scored as agreeing with EITHER human verdict — a judge that hedges
    # where the labeller hedged is not wrong (F3).
    def agree(h, g):
        if g is None: return False
        gv = g.get("verdict")
        if h == "borderline" or gv == "borderline": return True
        return h == gv
    print(f"\n  {'dir':6s} {'n':>4s} {'baseline':>9s} {'gate':>7s}")
    for d in ("e", "s", "n"):
        sub = [r for r in rows if r["dir"] == d]
        if not sub: continue
        n = len(sub)
        maj = max(sum(1 for r in sub if r["human"] == x) for x in ("good", "bad", "borderline")) / n
        acc = sum(1 for r in sub if agree(r["human"], r["gate"])) / n
        print(f"  {d:6s} {n:4d} {maj:9.0%} {acc:7.0%}" + ("   <-- beats baseline" if acc > maj else ""))
    ok = [r for r in rows if r["gate"]]
    print(f"\n  overall {sum(1 for r in rows if agree(r['human'], r['gate']))}/{len(rows)}"
          f"   ·  {el/max(1,len(rows)):.1f}s per image  ·  {len(rows)-len(ok)} gate error(s)")
    print(f"  -> {os.path.relpath(outp, REPO)}")


if __name__ == "__main__":
    main()
