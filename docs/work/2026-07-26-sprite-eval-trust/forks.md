# Forks — decisions taken, with what was rejected

## F1 — Fix the evaluation before retraining (again) · RESOLVED 2026-07-26

**P0/P1 (eval) precede P4 (retrain).**

The predecessor argued this ordering in [F1 there](../2026-07-25-sprite-gen-quality/forks.md#f1) and
was proved right the hard way: run-4's real failures — framed busts and a broken pose convention —
were invisible to the gate for hours, and surfaced only when the images were laid out by hand. A
retrain scored by a ruler we already know is blind produces a number nobody should act on.

Rejected: **fix the prep first** (it is the known bug, and tempting). But the prep fix has to be
*evaluated*, and evaluating it with the current gate would repeat exactly the mistake this stream
exists to correct.

Rejected: **fix both in parallel**. The P4 A/B must be scored by a **frozen** gate; tuning the
instrument while the experiment runs makes the result unreproducible.

## F2 — `iou_ref` against the reference sprite, not more box statistics · RESOLVED 2026-07-26

**Add IoU against the real corpus sprite** for the species+direction under test.

Rejected: **more bounding-box statistics** (perimeter, moments, Hu invariants). Same class that has
now failed four times — a summary of the box cannot see a portrait inside it ([I2](issues.md#i2)).

Rejected: **reviving `iou_control`**. Already implemented and refuted
([I5 there](../2026-07-25-sprite-gen-quality/issues.md#i5)): it compares against the *control image*,
so a wrong control faithfully obeyed scores **high** — the blobby anteater beat its own good east.
Reference-IoU differs precisely in comparing against **ground truth** instead of instruction.

Rejected for now: **a learned/CLIP scorer**. Strictly more powerful and the only thing that could
serve unseen species, but it is a black box at the exact moment we are trying to *restore* trust in
the measurement. Revisit once `iou_ref` is calibrated and understood.

**Known limit, accepted:** `iou_ref` is undefined for species absent from the corpus. That is where
the human filter stays, permanently, per the README stance.

## F3 — `iou_ref` is a COMPARISON metric, not a gate check · RESOLVED 2026-07-26

Calibrated on `.staging/gate-cal/labels.csv` (67 sprites), errors = false-pos + false-neg:

| gate | FP | FN | total |
|---|---|---|---|
| **CURRENT — `blobs + bg_uni + d_aspect<=50`** | 2 | 1 | **3** ✅ |
| + `iou_ref >= 0.55` | 2 | 3 | 5 |
| + `iou_ref >= 0.60` | 2 | 3 | 5 |
| asymmetric signed `-35 / +60` | 2 | 4 | 6 |
| asym + `iou_ref >= 0.55` | 2 | 5 | 7 |
| asym `-30/+70` + `iou_ref` | 2 | 5 | 7 |

**The gate is unchanged.** Every variant scored worse, all of it in false negatives — `iou_ref`
rejects good sprites whose shape legitimately differs from the one reference sprite for their
species (pose and build vary between individuals of the same animal).

**But `iou_ref` is not useless — it is answering a different question.** Two distinct jobs were
being conflated:

| question | right tool | evidence |
|---|---|---|
| *is this one sprite usable?* | the existing gate | 3 errors on 67 labelled sprites |
| *is model A better than model B?* | **`iou_ref`** | **6/6 species** on both run-4 failures |

`iou_ref` called run-4 worse on **every** species in **both** directions — including east, where
`d_aspect` reported a dead tie (34.8% vs 34.1%). That is exactly the judgement the numbers failed to
make yesterday. It just cannot be a per-sprite pass/fail, because a single reference cannot define
the acceptable range of an individual animal.

So: **gate stays as-is; `iou_ref` and `d_aspect_signed` become the A/B ranking metrics** and are
reported by every comparison. This mirrors [F3 in the predecessor](../2026-07-25-sprite-gen-quality/forks.md#f3),
where `d_fill` had the second-best marginal separation yet tripled gate error — good discrimination
does not imply gate value.
