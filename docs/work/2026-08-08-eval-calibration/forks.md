# Forks — eval calibration

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — The human labels are ground truth here, not the corpus {#f1}
_2026-08-08 · resolved when the labelled set arrived_

**Chosen.** For this stream, "good" means the user put it in `good-*`. The corpus sprite is not the
target.

**Why the distinction matters.** The east-pipeline stream used the corpus as ground truth and was
right to: it was asking *"is this the right animal"*, and `iou_ref` answers that. This stream asks
*"is this sprite acceptable"*, and those come apart — measurably. A south sprite with a perfect
silhouette and no face scores **higher** on `iou_ref` than a good one ([I2](issues.md#i2)). Only the
user's sorting encodes the property being tested.

**Consequence, stated:** a metric can be excellent at one job and useless at the other, and calling
either of them "the ruler" without saying which job is how this project ended up ranking methods
with a statistic that cannot rank images.

## F2 — Every accuracy is leave-one-out {#f2}
_2026-08-08 · resolved after the first table flattered itself_

**Chosen.** Fit the threshold without the image being scored, then score it. Report that number.

**Why.** The in-place fit gave `iou_ref` 87% on east, 79% on south and 69% on north — all "beating
baseline". Held out, the same metric gives **84% / 63% / 38%**, which is at or below baseline
everywhere. The first table was not wrong arithmetic; it was a threshold chosen with knowledge of
the answer, on 13–31 points, which is a machine for manufacturing confidence.

**Kept anyway:** the in-place numbers are still printed, labelled as the upper bound they are. The
gap between the two columns is itself the honest measure of how little data there is.

## F3 — A metric that cannot beat the majority class is reported as failing {#f3}
_2026-08-08 · resolved at plan time_

**Chosen.** Every table carries the majority-class baseline, and any metric at or below it is marked
as no better than guessing.

**Why it needs to be structural rather than a habit.** The east set is 26 good against 5 bad, so
"always say good" scores **84%** — which sounds like a working classifier and is nothing. Without the
baseline printed next to it, `iou_ref`'s 84% reads as a success. This is the same shape as the
project's earlier `iou_control` problem: a number that looks like validation while measuring
obedience to the wrong thing.

## F4 — Grow the set before building on `sat` {#f4}
_2026-08-08 · resolved at the moment `sat` scored 95%_

**Chosen.** Do not build a selector on the best-scoring metric yet. [P1](todo.md) grows south and
north to n≥40 first.

**Why, against the temptation.** `sat` at 95% leave-one-out on south is the strongest result in the
stream and I think it is **spurious** ([I3](issues.md#i3)): n=19, no mechanism connecting saturation
to "does this wolf have a face", and it **inverts sign against east**, where good sprites are *more*
saturated than bad ones. A discriminator that changes direction between cells of the same set is
describing the sample.

**Rejected — ship a `sat` threshold now and refine later.** A selector wired in on 19 images would
be trusted long after anyone remembered why it was provisional, and its failures would look like
model regressions rather than like a bad ruler.

## F5 — South's labels measure the 512 PIPELINE, not the method {#f5}
_2026-08-08 · resolved on finding [I4](issues.md#i4)_

**Chosen.** Treat the current south labels (5 good / 14 bad) as a measurement of the 512-resolution
pipeline, and plan to re-label after [P2](todo.md) settles the generation resolution.

**Why.** Those 19 images were all generated at 512, where the frontal face lands in ~100px and comes
out as a splayed mask. At 1024 the same seeds produce faces. So "south is 26% acceptable" is a fact
about a resolution setting, not about south — and carrying it forward as a property of the direction
would bake a fixable defect into the plan as a constraint.

**The cost, accepted:** the south labelling work is partly spent. Recorded rather than hidden,
because the alternative — quietly reusing labels collected under different conditions — is how a set
becomes untrustworthy without anyone noticing.
