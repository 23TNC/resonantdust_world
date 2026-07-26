# Forks — decisions taken, with what was rejected

## F1 — Phase order: fix the gate before touching the training data · RESOLVED 2026-07-25

**Gate first (P0), data second (P2).**

The tempting order is data-first: it is the biggest lever ([I1](issues.md#i1)), and the fixes are
already understood. Rejected because **the gate is the instrument every later comparison is scored
with**, and it currently both passes blobs and rejects antlers ([I3](issues.md#i3)). Retrain first
and the A/B result — "new LoRA scores better" — would be measured with a ruler we already know is
wrong in both directions. Fixing the ruler is one CPU-only phase and makes every subsequent number
trustworthy.

Also rejected: *interleaving* (fix the gate while the retrain runs). Tempting for wall-clock, but the
P3 A/B must be scored by a **frozen** gate — tuning the instrument while the experiment runs is how
you get an unreproducible result.

## F2 — Screening metric: silhouette IoU vs more bbox statistics · RESOLVED 2026-07-25

**Add `iou_control`** — IoU of the generated silhouette against the control silhouette that was fed
to ControlNet.

Rejected: **more bounding-box statistics** (perimeter ratio, moments, Hu invariants). They are all
the same *class* of measurement that already failed — global summaries that a blob can satisfy.

Rejected for now: **a learned/semantic scorer** (CLIP similarity to "a wolf", or a small classifier).
Strictly more powerful and worth revisiting, but it needs a labelled corpus we do not have, and it
would make the gate a black box at exactly the moment we need to trust it. IoU-against-intent is
cheap, explainable, and uses information the pipeline already holds.

**The gate never becomes the arbiter of taste** (README design stance): its job is to cut the pile
down to something worth a human glance. The anteater passing on proportions is the standing proof
that a number cannot replace the eye.
