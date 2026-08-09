# Forks — north/south evaluation

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — Build the vision gate BEFORE inventing a third interior metric {#f1}
_2026-08-08 · resolved at plan time_

**Chosen.** [P1](todo.md) builds the Claude-vision QA gate the design doc specifies, and it runs
before [P3](todo.md)'s interior-metric work.

**Why the ordering, against my own instinct.** I have now proposed two mechanisms for why north/south
fails and the data falsified both — interior edge noise (measured identical to east at 20%) and faces
failing by lopsidedness (measured backwards: good sprites are *more* asymmetric). A third hand-built
metric would be a third theory about a failure mode I have twice mis-read. A vision model needs no
theory: it looks at the interior, which is where every unmeasurable failure lives.

**It is also not my idea.** `sprite-gen-plan.md` has specified this gate all along — *"score, keep
best, retry the frame if none pass. This is what makes batch-and-curate hands-off."* Reaching for
geometry while a documented mechanism sat unbuilt is the same miss as
[direction-consistency D1](../2026-08-08-direction-consistency/deviations.md), where the doc's first
choice was skipped because it was not already installed.

**Rejected — train a small classifier on the labelled set.** 63 images, one species, one direction
mostly. That is enough to falsify a metric and nowhere near enough to fit one.

**The risk, stated:** a vision judge may be too slow or too expensive for 12 generations × 6 species,
and may agree with the user only on the obvious cases. [P1](todo.md) measures cost and disagreement
explicitly rather than discovering them after adoption.

## F2 — Re-collect south's labels instead of reusing them {#f2}
_2026-08-08 · resolved on the resolution finding_

**Chosen.** South's 19 existing labels are treated as a measurement of the **512 pipeline** and are
not fitted to. [P0](todo.md) settles the resolution first, then re-collects.

**Why.** All 19 were generated at 512, where the south subject spans a 202px column and the frontal
face lands in ~100px — and 1024 turns the same seeds from splayed masks into faces. "South is 26%
acceptable" is a fact about a setting we are about to change.

**The cost, accepted and named:** the user's south labelling work is partly spent. That is better
than the alternative, which is fitting a judge to a defect we intend to remove and then wondering why
it does not generalise.

## F3 — Reason codes and a `borderline` bucket are part of the data {#f3}
_2026-08-08 · resolved after the user explained three rejects_

**Chosen.** [P0](todo.md) records *why* an image was rejected, and adds `borderline` as distinct from
`bad`.

**Why it is not paperwork.** The user's three east rejects had three different causes — an unkeyed
background (deterministic, now solved), line art quality, and one extra line in a mane — and **two of
the three they would accept if pressed**. Under a binary label all three count equally against any
judge, so a judge that agrees with the user's own hesitation is scored as wrong. Worse, a fixable
deterministic defect and an aesthetic near-miss get averaged into one number that describes neither.

**Rejected — keep binary labels for simplicity.** Simplicity that destroys the distinction between
"this is broken" and "I could go either way" makes every later accuracy figure unreadable.

## F4 — East is out of scope; it is already servable {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** This stream works north and south. East gets only the deterministic background check it
already earned.

**Why.** East's numbers are usable today: `d_aspect` distance from the corpus wolf separates good
(0.021) from bad (0.289), and with the corner-alpha check the clear misses on 31 images drop to
**zero** — the remaining two disagreements are the user's own borderline calls. Spending this stream
re-tuning east would improve a cell that is not blocking anything.

**What that costs, recorded:** the judge built here will be validated mostly on north/south, so its
east behaviour will be under-measured. [P1](todo.md) scores all three directions even though only two
are the target, so east is a free control rather than a blind spot.

## F5 — Generate at 768: match the LoRA, not the base {#f5}
_2026-08-08 · resolved at P0.1 · **768, not 1024 and not 512**_

**Chosen.** `--gen-size 768` — the resolution `rd_diremph_anima_r20_g07` was trained at.

Three resolutions were in play and only two had been tested. Same three seeds, wolf south,
`corpus:Wolf_Timber` control, everything else fixed:

| | result |
|---|---|
| **512** (the old default) | all three broken — splayed white masks, no coherent face |
| **768** (LoRA's training res) | **all three resolve a proper face** — eyes, muzzle, dark nose, ears — and stay closest to the corpus proportion |
| **1024** (SDXL native) | faces resolve, but the interior drifts busier: spikier fur, more strokes, heads growing against the body |

**Why 768 beats 1024, which was my earlier recommendation.** I proposed 1024 on the reasoning that
SDXL is native there and 512 is half of it. That reasoning was incomplete: the **LoRA** has only ever
seen 768, and it is the LoRA that carries the convention. At 1024 the base's own detail prior has
more room and the sprite gains fur strokes and interior lines the corpus does not have — better than
512's broken face, worse than 768 on the thing we actually want.

**It is also cheaper.** 768 is ~2.25× the compute of 512; 1024 is ~4×.

**Recorded as a correction, not a discovery.** The user asked "did we use the 1024 scaled-up dataset?"
while I was recommending 1024, which is what prompted checking `train_run20.sh` — `--resolution="768,768"`,
latent caches stamped `_0768x0768_sdxl.npz`. The 1024 datasets on the box are from the older
quad-only era and were never used for run-20. Had that question not been asked, the stream would have
adopted 1024 on a half-argument.

**Consequence for [P0](todo.md):** the south labels are re-collected at 768, not 1024.
