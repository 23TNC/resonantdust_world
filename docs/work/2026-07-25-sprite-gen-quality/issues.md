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
