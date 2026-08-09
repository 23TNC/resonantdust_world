# A north/south evaluation worth training against — 2026-08-08

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{eval_data,eval_agreement,lora_eval,generate}.py`.
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md).
Successor to [`2026-08-08-eval-calibration`](../2026-08-08-eval-calibration/README.md), which
established that we cannot currently tell a good north/south sprite from a bad one._

## Why this exists

The next training run will produce ~12 generations and someone has to pick one. Today that pick is
made by looking at kohya's samples, which are **bare txt2img** and do not resemble pipeline output
([eval-calibration I1](../2026-08-08-eval-calibration/issues.md#i1)). The obvious fix is to score
pipeline output instead — and the predecessor measured that we cannot:

| | n | majority baseline | `iou_ref` | best geometric |
|---|---|---|---|---|
| east | 31 | 84% | 84% | `d_aspect` 90% |
| **south** | 19 | 74% | **63%** | 63% |
| **north** | 13 | 62% | **38%** | 46% |

**East is servable today. North and south are not**, and `iou_ref` is *anti*-correlated on north —
it prefers the sprites the user rejected. So a training run selected on north/south quality would be
selected on noise.

## The reason, and it is structural

At `cn 0.5` the ControlNet **gives** the sprite its silhouette. Every metric we compute — `iou_ref`,
aspect, fill, solidity, hull — describes that silhouette. So on north and south the metrics measure
the thing the pipeline already guaranteed, and carry no information about the thing that varies.

Measured, on south: the sprites the user **rejected** are *geometrically closer* to the real wolf
than the ones they kept (`d_aspect` good 0.014 vs bad 0.010; `iou_ref` distance good 0.087 vs bad
0.065). The failures are entirely **inside** the silhouette — the face does not resolve — and nothing
we compute looks there.

East escapes this because its head is a large side profile that survives; south's frontal face lands
in ~100px and breaks ([eval-calibration I4](../2026-08-08-eval-calibration/issues.md#i4)).

## What the user actually rejects — three different measurables

Asked why three specific east sprites were rejected, the answers were **not one property**:

| reason given | kind | status |
|---|---|---|
| *"having a white background"* | deterministic | **solved** — corner alpha; found it in 1 of 63, exactly the flagged image, and `bg_uni` misses it because it asks "is the plate clean", not "is there still a plate" |
| *"improper line art"* | interior structure | open |
| *"an extra line in the mane"* / *"the mane is missing"* | interior semantics | open, hardest |

Two of those three the user said they would **accept if pressed**. That matters: some disagreements
are the boundary of their own judgement, not metric failures, and a binary good/bad label cannot tell
the two apart. **Capturing the reason is therefore part of the data, not paperwork** —
[P0](todo.md).

## The mechanism the design doc already specifies and we have not built

[`sprite-gen-plan.md`](../../components/dev/scripts/art/plan/sprite-gen-plan.md) § quality upgrades:

> **Claude-vision QA gate** — automated per candidate: *"Single creature? Correct facing for this
> frame? Clean silhouette (no extra limbs/heads/duplicates)? On-style (flat, not photoreal)?"* →
> score, keep best, retry the frame if none pass. **This is what makes batch-and-curate hands-off.**

A vision model looks at the **interior**, which is exactly where every failure we cannot measure
lives. It has been in the plan the whole time while this project fitted thresholds to geometry —
the same shape of miss as
[direction-consistency D1](../2026-08-08-direction-consistency/deviations.md), where the doc's first
choice of edit-model was dropped because it was not already installed.

## Design stance

- **Score PIPELINE output, never kohya samples.** The whole point. An evaluation that judges
  unconstrained txt2img is measuring a model we do not ship.
- **The user's labels are ground truth, and their REASON is data.** A reject for a white background
  and a reject for a missing mane are different failures and must not be averaged.
- **Held out or it did not happen.** Fitted-in-place thresholds made `iou_ref` look like it beat
  baseline everywhere; leave-one-out showed it never does.
- **A metric that cannot beat guessing the majority class is reported as failing.** With 26 good and
  5 bad, "always say good" scores 84% — which looks like a classifier and is nothing.
- **Two of my proposed mechanisms were already wrong** (interior edge noise; faces failing by
  lopsidedness — the measurement says good sprites are *more* asymmetric). Prefer testing a
  mechanism-free judge over inventing a third theory.

## Future intent this plan must not trim

- **The deliverable is a selection procedure for the next training run**, not a metric for its own
  sake. If no automatic judge survives, the honest output is a fast human sorting loop, and that is a
  success condition, not a failure.
- **The asymmetry signal is unexplained, not dismissed.** It gets 3 of 19 wrong on south — the best
  result anyone has — and my account of *why* is falsified. It needs power, not a story.
- **Whatever lands must extend past wolves.** Every label so far is one species; a judge fitted to
  wolves is not a judge.
- **South's current labels measure the 512 pipeline**, not the direction
  ([eval-calibration F5](../2026-08-08-eval-calibration/forks.md#f5)). They are re-collected here at
  the settled resolution before anything is fitted to them.
