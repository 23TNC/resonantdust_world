# Issues — inherited defects this stream attacks

## I1 — `d_aspect` is unsigned, so it cannot distinguish a wobble from a broken convention

Inherited from [I14](../2026-07-25-sprite-gen-quality/issues.md#i14). Measured on the run-4 A/B, east:

| species | ref | e07 | run-4 |
|---|---|---|---|
| wolf | 2.08 | 2.41 | **1.23** |
| bear | 2.30 | 2.68 | **1.52** |
| fox | 2.42 | 2.29 | **1.22** |
| pig | 1.95 | 2.62 | **1.43** |

Mean **absolute** error is a dead heat — e07 34.8%, run-4 34.1%. But e07 errs **long** (5/6 above
reference) and run-4 errs **compact** (6/6 below, several by 30–50%). A wolf at 1.23 against a 2.08
reference is a **sitting** wolf. `abs()` scores those identically, so the metric reported a tie where
one model kept the oblique top-down convention and the other abandoned it.

## I2 — Bounding-box statistics cannot see what is inside the box

Run-4's south sprites are head-only portraits inside a drawn white rectangle, and **10/18 passed the
gate**. A framed bust and a full body can share a bbox aspect, a blob count, a solidity and a
background purity. Every metric in the gate is a summary *of the box*, so none of them can see the
difference.

This is the same class as [I3](../2026-07-25-sprite-gen-quality/issues.md#i3) (the blobby anteater
passing) and is why `iou_ref` is proposed: overlap against real ground truth is the first metric that
looks *inside* the box.

## I3 — The frame artefact: a fix that introduced a learnable constant

Inherited from [I13](../2026-07-25-sprite-gen-quality/issues.md#i13). Pinning `fill = 0.85` gave all
459 training images an identical ~7.5% white margin. That uniformity is learnable, and the model
learned it — reproducing the margin as a drawn rectangle with a portrait composed inside.

The predecessor's P2 measured a real benefit (scale sd 0.148 → 0.003, sharpness 43.1 → 90.1); the
remedy must keep it. Jitter the fill, do not abandon normalisation.

## I4 — Pose drift on east, cause unknown

Run-4's east sprites are systematically more compact than both the reference and e07 — sitting or
curled rather than the low lying profile. Four candidate causes, none yet isolated:

1. the learned margin from [I3](#i3) pushing composition inward
2. rank 48 vs run-3's 32
3. 1024 training vs 768
4. crop-to-bbox changing the apparent pose of the training sprites themselves

Run-4 changed all four at once, so its result cannot attribute the drift. P3 isolates before P4
spends another five hours.
