# Issues — the defects this stream attacks

## I1 — Training data is blurred and unnormalised (the biggest single lever)

Two independent defects in `bin/lib/prep_train.py`, both affecting **nearly every training image**:

**Blur.** The prep resizes with `Image.LANCZOS`. Most corpus sources are 256px (545 files), some
128 (72) and some 64 — upscaled to 768 that is a 3–12× enlargement, and LANCZOS *smooths* the result.
So the LoRA was taught soft boundaries on almost its entire dataset, which is a plausible cause of
the blobby bears, mushy legs and "pointy legs" complaints seen throughout.

The user's framing is the important one: **this art is effectively vector graphics rendered as
bitmaps** — flat regions bounded by hard outlines. That means the missing pixels are *reconstructible
from a correct prior* ("edges are crisp, regions are flat"), unlike a photo where an upscaler would
hallucinate. An edge-preserving / anime-line-art upscaler should therefore recover something close to
the original vector art, not invent detail.

**Scale.** Subject scale is never normalised. The bear nearly fills its frame; the cat occupies a
fraction of it. The LoRA correctly learned that scale is arbitrary, and generated scale drifts as a
result (observed: "bear huge, cat small in the same batch"). Normalising the alpha bbox to a fixed
frame fraction also *raises effective resolution* for small subjects, which compounds with the blur
fix rather than duplicating it.

## I2 — Control selection is a hand-written table, and it caused the anteater failure

`silhouette_bank.FAMILY_REP` maps 11 families to one representative each — all chosen by hand. The
giant anteater was routed to **Elephant** purely because the table says `pachyderm -> Elephant`.

Result: the east view is genuinely good (long tapered snout, hunched back, huge bushy tail, diagonal
shoulder stripe) but **south and north collapse into a cape-like blob** where the tail dominates the
silhouette and the body reads as a fan. The elephant silhouette gave it mass but not structure.

There are **396 silhouettes** in the bank and the selection consults none of them — it consults a
human's guess about taxonomy, when the thing that actually matters is *shape*. A similarity search is
both more likely to be right and removes a hand-tuned knob (README design stance).

## I3 — The gate scores proportions, not shapes: it has both error types

| | case | numbers |
|---|---|---|
| **False positive** | blobby anteater south / north **passed** | `d_aspect` 13.2% / 4.1% |
| **False negative** | good horned oryx **quarantined** | `solidity=0.349` vs a 0.35 threshold — failed by 0.001 |

Both follow from the same root: [F4](../2026-07-25-template-free-generation/forks.md#f4) measures the
**bounding box**, so a correctly-proportioned blob passes and a correctly-shaped animal whose horns
enclose empty bbox area fails.

Candidate fix — **IoU of the output silhouette against the control silhouette it was given**. The
pipeline already *has* the intended shape (it fed it to ControlNet), so it can ask "did the output
follow it?". That is much closer to a semantic check than any bbox statistic, and it costs nothing.

The oryx also shows the threshold itself is arbitrary at the boundary: 0.349 vs 0.35 is not a
quality difference. Whatever replaces it must be calibrated against hand-labelled art, not chosen.

## I4 — Cross-direction coherence is ~6× looser than hand-authored art

Bhattacharyya distance between per-direction RGB histograms (opaque pixels only):

| | mean |
|---|---|
| REAL corpus, same animal | **0.028** |
| generated, `family:` | 0.169 |
| generated, `control none` | 0.193 |
| REAL corpus, different animals | **0.565** |

Identity clearly transfers — 0.169 is far nearer same-animal than different-animal — but a set is
visibly less consistent than drawn art. Untested lever: the IP-adapter weight (`IP_WEIGHT = 0.6`),
never swept.

## I5 — `iou_control` does NOT catch the anteater: [F2](forks.md#f2)'s premise is refuted

Implemented and measured immediately. Anteater on the Elephant control:

| view | verdict | `iou_control` |
|---|---|---|
| east | **good** | 0.703 |
| south | **bad** (blob) | **0.789** |
| north | **bad** (blob) | **0.732** |

The bad views score **higher** than the good one — the opposite of the acceptance criterion.

The reasoning error is now obvious: `iou_control` asks *"did the output follow the shape it was
handed?"*. The anteater's failure is that the shape it was handed was **wrong** ([I2](#i2), Elephant
for an anteater), and the blob followed it faithfully. A bad control obeyed produces a high IoU.

`iou_control` is kept as a **reported metric** — it genuinely measures drift off the control, which
is a real failure mode — but it is **not in the gate**, and its value there is unproven on this set.

## I6 — `hull_solidity` does not discriminate either

Hypothesis: a blob is convex (high mask/hull ratio) while an animal with legs, snout and horns is
concave (low ratio). Measured on the 67-sprite calibration set: **good 0.905 vs bad 0.878**,
separation **0.30 sd** — no useful signal. The anteater specifically: 0.927 (good east) vs 0.906 /
0.920 (bad south/north), i.e. backwards again. Implemented and reported, not gated.

## I7 — For an unseen species the reference is a PROXY, so `d_aspect` cannot judge it

This is the structural limit behind the two residual false positives, and it is not fixable by
tuning thresholds.

The gate scores `d_aspect` against a **real corpus sprite**. For a species in the corpus that sprite
*is* the ground truth. For an **unseen** species there is none, so `--ref` names a stand-in from
another species — the anteater was scored against **Elephant**. Its blobby south then scored
`d_aspect` **13.2%** (north: 4.1%) and passed, because it genuinely does resemble an elephant's
proportions. The gate answered the question it was asked; the question was wrong.

Consequence: **geometry gating is sound for corpus species and weak for unseen ones** — exactly the
case template-free generation exists to serve. Closing it needs a metric with a notion of "what an
anteater looks like" (a learned/semantic scorer, deliberately rejected in [F2](forks.md#f2) as an
unexplainable black box). Until then the human remains the filter for unseen species, which the
README already commits to.

## I8 — Coarse occupancy matching cannot tell species apart (`auto` picked a hedgehog for a wolf)

`--control auto` probes with txt2img, then matches the probe against the bank on a 32×32 occupancy
grid + aspect. For a **wolf** it selected **`AEXP_Hedgehog`** (0.79), ahead of every canine.

At that resolution a lying wolf and a hedgehog are both "a roundish mass, wider than tall" — the
metric sees where mass sits, not what the animal is. It is doing exactly what it was built to do;
body-plan occupancy simply does not encode species.

The damage is smaller than feared: at `cn 0.20` the control supplies only rough mass, so the LoRA
still drew a proper grey wolf — just hunched and compressed versus the template-controlled version.
But "less bad than feared" is not "correct", and a hand-written `canine -> Wolf_Timber` is right
where the measurement is wrong. This is why [F4](forks.md#f4) keeps the family table.

## I9 — The proxy-reference problem, in its sharpest form

Anteater, three control modes, `d_aspect` scored against the **Elephant** proxy:

| control | east | south | north | visual verdict |
|---|---|---|---|---|
| `family:pachyderm` | **0.4%** | 13.2% | 4.1% | **worst** — snout survives only in east, s/n are cape blobs |
| `auto -> Gorilla` | 52.2% | 1.7% | 29.8% | worse still — snout gone entirely, hunched blobs |
| `none` | 52.7% | 21.4% | 7.8% | **best** — recognisable anteater in all three views |

**The metric is anti-correlated with quality here.** The visually best output scores worst; the
visually worst scores best. Not a threshold problem — the reference is an *elephant*, so "looks like
an elephant" is what `d_aspect` rewards, and an anteater that looks like an anteater is penalised for
it. Restates [I7](#i7) with the numbers reversed as starkly as they can be.

Practical rule: **for a species with no corpus twin, do not trust `d_aspect`** — neither for gating
nor for choosing a control. The human is the filter there, as the README commits.

## I10 — Two of my own acceptance criteria were wrong (plan errors, corrected in place)

**"higher outline gradient magnitude" is a bad proxy for a better upscale.** It is maximised by
`NEAREST` (107.4 vs LANCZOS 21.7) — which produces visibly *staircased* curves, worse art than a
soft edge. The metric rewards hard pixel steps, not crisp smooth boundaries. Kept as *one* input
and paired with a visual check; ESRGAN won on both (109.8 **and** smooth curves), so the decision
stands, but the criterion alone would have chosen NEAREST.

**"alpha-bbox area / frame area = 0.80 ± 0.03" is unsatisfiable while preserving aspect.** Area is
`w·h`; a long low wolf (aspect ~2.1) and a tall front view (aspect ~0.4) cannot both hit a fixed
area fraction unless one is *distorted*. Replaced with **longer-side normalisation** — every subject's
longer dimension is 0.85 of the frame — which gives consistent on-screen presence with aspect intact.
Achieved: **0.848 ± 0.0009**.

## I11 — Scale normalisation was silently a no-op: two stacked alpha traps

The first implementation measured **0.70 ± 0.084** instead of a flat 0.85, and my first diagnosis was
wrong too. Two separate traps:

1. `Image.getbbox()` on **RGBA** treats a pixel as non-zero if *any* channel is — so a sprite storing
   colour in its transparent margin reports the whole frame and never crops.
2. Switching to the alpha channel's own `getbbox()` **still** returned the whole frame, because the
   corpus's "transparent" background is not alpha 0 but **alpha ≈ 3**.

Only thresholding the alpha (`> ALPHA_MIN = 16`) before taking the bbox fixed it. Worth noting the
failure mode: the crop silently did nothing, the images looked plausible, and only measuring the
achieved fraction exposed it. A visual check would have passed this bug.

## I12 — The quadruped dataset was built by a script that lived only in scratchpad

`prep_train.py` builds the *full* 200-species animal set, but the LoRA under test (`rd_quadruped_e07`)
was trained on the 132-species **quadruped subset**, built by `build_quad.py` — which existed only in
the session scratchpad and did its own `LANCZOS` resize with **no scale normalisation**. So P2's fixes
initially landed on a dataset P3 does not use, and I started rebuilding the wrong one before catching
it.

`build_quad.py` is now in `bin/lib/` and delegates image prep to `prep_train.normalise()`, so both
sets share one prep path and cannot drift again. General lesson: a script that produces a training
input is *tooling*, not a scratch file — if it is not versioned, its defects are invisible to review.

## I13 — Pinned scale taught the model to draw a FRAME, and it broke south views

Run-4's south sprites are **head-only portraits sitting inside a visible white rectangle**
(`.staging/ab4/visual.png`, column 4) — wolf, bear, tiger, cat, fox and pig all render a face, no
body, framed. East views from the same checkpoint are *better* than the shipping model.

**Cause: the P2 scale normalisation was too strict.** Fixing subject size to exactly `fill = 0.85`
gave every one of the 459 training images an identical ~7.5% white margin. A constant that uniform is
learnable, and the model learned it — reproducing the margin as a drawn rectangle and composing a
portrait inside it. The fix that removed one defect (arbitrary scale) introduced another.

**Remedy:** jitter the fill per image (≈0.78–0.90) so subject scale is *consistent* without the
margin being a fixed value. Keeps the benefit that scale-drift is bounded; removes the constant.

**The gate did not catch it.** Run-4's south still passed 10/18 — a bust inside a frame can land on a
plausible bbox aspect. Reported as "slightly worse overall" by the numbers, while the images show
"better on east, catastrophically broken on south". Another instance of
[I3](#i3)/[I7](../2026-07-25-template-free-generation/issues.md#i7): geometry metrics cannot see
*portrait instead of body*. **The visual check is what found this**, and no threshold change would
have.

## I14 — `d_aspect` is unsigned, so it cannot tell "too long" from "wrong pose"

Correcting my own P3 write-up. I reported run-4 as **winning east**; the user looked at the sprites
and said e07 held the proper geometry. Measuring the *signed* error shows they were right:

| species | ref | e07 | run4 |
|---|---|---|---|
| wolf | 2.08 | 2.41 | **1.23** |
| bear | 2.30 | 2.68 | **1.52** |
| fox | 2.42 | 2.29 | **1.22** |
| pig | 1.95 | 2.62 | **1.43** |

Mean **absolute** error is a dead heat — e07 34.8%, run4 34.1%. But e07 errs **long** (5 of 6 above
reference) while run4 errs **compact** (6 of 6 below, several by 30–50%). A wolf at 1.23 against a
2.08 reference is not a lying wolf with slightly-off proportions; it is a **sitting** wolf. Run-4
rendered east more richly while abandoning the low oblique top-down pose the art style depends on.

`d_aspect` takes `abs(...)`, so "too long" and "too upright" are scored identically — the metric was
blind to the distinction by construction, and reported a tie where a human saw one model keeping the
convention and the other dropping it.

**Implication for the gate:** undershooting aspect (too compact) should be penalised harder than
overshooting, because compact is the direction that means *wrong pose* rather than *slightly wrong
proportion*. Not yet implemented — it needs calibrating against the labelled set like [F3](forks.md#f3).

**Correction to [F5](forks.md#f5):** "run-4 wins east" should read "run-4 renders east more richly but
loses the pose convention". Run-4 has **two** failures, not one — a broken convention on east and
framed portraits on south. The verdict (e07 ships) is unchanged and now better supported.
