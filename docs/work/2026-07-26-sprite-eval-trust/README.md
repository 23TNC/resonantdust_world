# Sprite eval trust — make the measurement agree with the eye, then fix what run-4 broke — 2026-07-26

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{lora_eval,prep_train,build_quad,generate}.py`
+ the `rd_quadruped` LoRA. Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Successor to
[`2026-07-25-sprite-gen-quality`](../2026-07-25-sprite-gen-quality/README.md) — its **P4 flywheel**
and **P5 coherence** remain open there and are **not** in scope here. Shipping LoRA is still
`rd_quadruped_e07`._

## Why this stream exists

Run-4 spent 4 h 39 m of GPU and did not ship. It failed twice, and **the evaluation caught neither**:

| | what happened | what the metric said |
|---|---|---|
| **south** | head-only portraits inside a drawn white rectangle | passed **10/18** |
| **east** | pose drifted from lying to sitting | a **tie** (34.8% vs 34.1%) |

Both were found by *looking at the images*. That is the third and fourth time in two days that an
ad-hoc visual check overturned the numbers — the anteater case ([I9 there](../2026-07-25-sprite-gen-quality/issues.md#i9))
had the metric rating the visually **best** output **worst**.

## The thesis: the eval is the bottleneck, not the model

A measurement that disagrees with the eye is worse than no measurement, because it is *trusted*. Two
concrete blind spots, both structural rather than mis-tuned:

1. **`d_aspect` is unsigned.** `abs(generated − reference)` scores "too long" and "too upright"
   identically. But long is a *proportion wobble* and compact is a *broken pose convention* — the
   sign was the entire signal, and the metric threw it away ([I14 there](../2026-07-25-sprite-gen-quality/issues.md#i14)).
2. **Everything is bounding-box statistics.** A framed bust and a full body can share a bbox aspect.
   No summary of the box can see *what is inside it*.

So this stream fixes the ruler before touching the model again — the same ordering its predecessor
argued in [F1](../2026-07-25-sprite-gen-quality/forks.md#f1), which proved right the hard way:
the gate's blindness is *why* run-4's real failure went unnoticed until the images were laid out.

## The candidate fix, and why it is different from the one already refuted

**IoU of the generated silhouette against the REFERENCE corpus sprite** for that species+direction.

This is **not** `iou_control`, which was implemented and refuted in the predecessor
([I5 there](../2026-07-25-sprite-gen-quality/issues.md#i5)). That one compared the output to the
**control image** it was handed, and failed because a *wrong* control faithfully obeyed scores high —
the blobby anteater beat its own good east view.

Reference-IoU compares against **real ground truth**: the actual Wolf_Timber sprite for wolf-east. A
sitting wolf overlaps a lying wolf poorly; a framed bust overlaps a full body poorly. It should
therefore catch **both** run-4 failures, which no bbox statistic can. Its limit is honest and known:
it is only defined for **corpus species** — but that is exactly where A/B comparisons live.

## Design stance

- **The visual check becomes part of the harness, not a favour.** Every eval and A/B auto-emits a
  contact sheet. It found the truth three times; it should not depend on someone remembering to write
  a montage script.
- **Calibrate, never choose.** Any gate change is scored against `.staging/gate-cal/labels.csv`
  (67 hand-labelled sprites) and reports false-positive/false-negative counts, as
  [F3 there](../2026-07-25-sprite-gen-quality/forks.md#f3) did — where a metric with the *second-best*
  marginal separation still tripled total error when gated on.
- **Diagnose before retraining.** Run-4's pose drift has at least four candidate causes (the learned
  margin, rank 48, 1024, crop-to-bbox). Another 5-hour run that changes several at once cannot
  attribute the result. Isolate first.
- **The human stays the final filter.** Reference-IoU will not save unseen species, where there is no
  ground truth to compare against. That limit is permanent and belongs in the workflow, not in a
  threshold.

## Future intent this plan must not trim

The eval work is deliberately **body-plan agnostic**: the planned biped, legless, arthropod and
humanoid LoRAs will each arrive with their own corpus and must reuse this gate and contact-sheet
harness unchanged. Do not special-case quadrupeds. Likewise, the prep fix must keep the *benefit* the
predecessor's P2 measured — scale drift bounded to sd 0.003, outlines 2.1× sharper — while removing
the learnable constant; the fix is to **jitter** the fill, not to abandon normalisation.
