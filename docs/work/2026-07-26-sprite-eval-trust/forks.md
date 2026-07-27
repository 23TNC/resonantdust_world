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
