# Completed — the east pipeline

_Dated entries: what landed and **how it was verified**. Newest last._

## P0 — Ground truth and a ruler that can see a wrong animal

- **2026-08-08 · P0.1 · Six species pinned, chosen on MEASURED spread rather than intuition.**
  `bin/lib/eval_east.json`. Measured every candidate's real east sprite first, then picked to span
  the axes a method could fail on separately:

  | species | aspect | lum | sat | what it tests |
  |---|---|---|---|---|
  | Wolf_Timber | 2.08 | 88.9 | 17.6 | the incumbent — and the shape every other species is forced into |
  | AEXP_BlackBear | 2.16 | **66.6** | 20.5 | darkest in the corpus; the species that broke visibly |
  | Fox_Red | **2.42** | 87.8 | **70.4** | most saturated and longest |
  | Deer | **1.50** | 111.4 | 6.9 | most compact, thin legs — a canid template cannot fake it |
  | Elephant | 2.02 | **160.5** | 0.4 | brightest and achromatic; where our too-pale defect is *correct* |
  | Tiger | 1.92 | 102.0 | 43.6 | high-frequency interior markings — `iou_ref` is blind to stripes |

  Aspect spans 1.50–2.42, luminance 66.6–160.5, saturation 0.4–70.4. **Pig, Cow and Horse were
  dropped** as near-duplicates of Elephant/Deer on every axis — they would have added cells without
  adding questions. Verified per species that a silhouette-bank `e.png` **and** a real sprite for
  `iou_ref` both exist; `Snake_Rattle`, `Rhinoceros` and `Turtle` were considered and have neither.

- **2026-08-08 · P0.2 · `bin/lib/east_eval.py` scores against ground truth.** Reports **`iou_ref`
  against the real corpus sprite** (primary), signed aspect, interior luminance/saturation as a
  **difference from the real sprite** rather than an absolute, and the structural gate. Methods are
  declared in the set file, so adding S2 is data, not code. Carries the `_rejected/` path
  re-resolution that bit twice in the predecessor
  ([I7 there](../2026-08-08-direction-consistency/issues.md#i7)) — resolved at open time, not at
  write time.

  One thing worth naming: the real corpus sprites are stored **on white with no alpha**, so
  measuring their interior directly would average in the plate. `ref_as_rgba()` rebuilds an alpha
  from the non-plate pixels so `d_lum`/`d_sat` compare animal against animal.

- **2026-08-08 · P0.3 · Self-check PASSED — the ruler reads its own ground truth correctly.**
  Scoring the real sprites through the harness returns **`iou_ref` = 1.000 for all six**, with
  `d_aspect`, `d_lum` and `d_sat` all exactly 0.0 and `blobs` = 1.

  This item exists because the reference lookup falls back to a directory scan when the exact
  filename misses, and a *wrong-but-plausible* reference would silently mis-score every method
  afterwards while looking entirely healthy. A perfect self-score is the only cheap proof that
  `iou_ref` is comparing each species against itself.

- **2026-08-08 · P0.4 · S0 baseline = `iou_ref` 0.731, and the aspect column proves
  [I1](issues.md#i1) as a measurement rather than a code read.** 18 generations, wolf template,
  6 species × 3 seeds.

  | species | mean `iou_ref` | mean `d_aspect` |
  |---|---|---|
  | tiger | 0.808 | +6.9 |
  | wolf | 0.785 | −16.0 |
  | fox | 0.735 | −15.0 |
  | deer | 0.700 | +3.6 |
  | bear | 0.699 | **−34.9** |
  | elephant | 0.657 | **−28.8** |
  | **all** | **0.731** | |

  **The wolf reaches the highest single score of any species — 0.935 against a best-of-the-rest of
  0.811.** Its mean is dragged to 0.785 only by seed 4102, which failed the structural gate. So the
  template's own species has the highest ceiling, which is what I1 predicted.

  **The proof is in `d_aspect`, not `iou_ref`.** Signed proportion error against the real animal:
  **bear −34.9% and elephant −28.8%** — the two bulkiest animals come out roughly a third *more
  compact* than they should, squeezed toward a canid's lither outline. Deer (+3.6) and tiger (+6.9)
  are nearly right, and they are the two species whose proportions already sit closest to the wolf's.
  **13 of 18 sprites are more compact than the real animal.** That is the wolf template imposing
  itself, measured per species, and it is exactly the "wolf→cat came out leggy" failure the design
  doc described.

  **Seed 4102 is bad across species** — it produced 3 blobs for wolf, bear *and* elephant, the only
  gate failures in the run. Kept in the set rather than swapped out: a method that survives a hostile
  seed is the one worth having, and silently replacing it would flatter every later comparison.

  This is the floor. Every method below must beat 0.731, and must fix `d_aspect` on bear and
  elephant specifically — a method that raises mean `iou_ref` while leaving those two squashed has
  not addressed the defect that opened the stream.

## P1 — The stage ladder, measured

- **2026-08-08 · P1/S1 · One flag takes `iou_ref` from 0.731 to 0.852 and halves the proportion
  error. The multi-stage work is not needed to fix trained species.** `--control corpus:<Species>`,
  everything else identical to S0 — same LoRA, seeds, knobs, prompt. 18 generations.

  | species | S0 | S1 | Δ | S0 mean\|aspect\| | S1 mean\|aspect\| |
  |---|---|---|---|---|---|
  | **wolf** | 0.785 | 0.785 | **±0.000** | 16.0 | 11.7 |
  | bear | 0.699 | 0.868 | **+0.169** | 34.9 | 14.4 |
  | fox | 0.735 | 0.931 | **+0.196** | 15.0 | 1.7 |
  | deer | 0.700 | 0.709 | +0.009 | 21.9 | 17.2 |
  | elephant | 0.657 | 0.860 | **+0.203** | 28.8 | 15.6 |
  | tiger | 0.808 | 0.958 | **+0.150** | 6.9 | 1.0 |
  | **all** | **0.731** | **0.852** | **+0.121** | **20.6** | **10.3** |

  Cells below 0.70: **8/18 → 4/18**. Gate failures: **3/18 → 1/18**.

  **The wolf row is the internal control and it is exactly ±0.000.** The one species whose template
  was already correct gains nothing, and every species whose template was wrong gains in proportion
  to how wrong it was — bear and elephant, the two bulkiest animals, gain most. That is not a
  coincidence to note in passing; it is the prediction of [I1](issues.md#i1) coming back as a
  measurement, with a built-in null result to prove the harness is not simply rewarding change.

  **Verified by eye, and the eye is emphatic** (`.staging/p1-s0-vs-s1.png`, seed 4101): S0's bear,
  fox, deer and elephant are all recognisably the same lithe canid silhouette wearing different
  colours. S1's bear is bulky with short legs, the fox has its brush tail, the elephant has a trunk
  and ear, the tiger is striped, and the deer has antlers and thin separated legs.

  **The one disagreement is deer, and the metric is wrong there** ([I5](issues.md#i5)): the real
  `Deer` sprite is an antler-less spotted doe, so the generated buck's antlers score as pure union
  and drag IoU down while being *more* correct. Deer's number must not be optimised away.

  **What this means for the plan ([F1](forks.md#f1) holding as written):** the cheapest possible fix
  wins on trained species, and that was the point of measuring it first. It **cannot** generalise to
  untrained species — there is no corpus silhouette to read — so [P2](todo.md)'s silhouette-generation
  stage keeps its full justification and now has a hard bar to clear: 0.852.

- **2026-08-08 · P1/S3 · `--control auto` scores 0.556 — below the incumbent — because it declined
  on every cell and fell back to no ControlNet at all ([I6](issues.md#i6)).**

  | | mean `iou_ref` | mean `d_aspect` | gate failures |
  |---|---|---|---|
  | S0 template | 0.731 | −20.6 (abs) | 3/18 |
  | S1 corpus | **0.852** | 10.3 (abs) | 1/18 |
  | S3 auto | 0.556 | ≈ −52 | **11/18** |

  The item's acceptance asked *which species it picked* — the answer is **none, 18 times**, with the
  nearest bank match a **Gorilla or Orangutan** for every one of wolf, bear, fox, deer, elephant and
  tiger. Best match peaked at 0.61 against a 0.65 threshold.

  **Verified rather than inferred.** The first S3 run discarded `generate.py`'s stdout, so the
  decision was invisible and the uniform −52 aspect was unexplained. Re-ran with the control
  decision captured per cell — the harness now records it in `results.json` — which turned a
  puzzling table into a diagnosis. The extra 18 generations were the cost of the acceptance
  criterion being written as "which species it picked" rather than "the score".

  **This is the single most useful result of the phase**, because it is the naive form of the user's
  own proposal, measured: generating the silhouette from an unconstrained pass **does not work**.
  The probe comes out upright and compact — matching primates, and half as long as a side profile
  should be — because the model will not produce our convention without something already holding
  the pose. [P2](todo.md)'s stage 1 cannot be "txt2img and see"; it needs its own constraint, and
  finding that is the phase's real question rather than an implementation detail.

- **2026-08-08 · P1/S4 · The refine pass earns nothing and is dropped.** Added `--refine <denoise>`
  to `generate.py` — a second pass, i2i from the first with the *same* ControlNet edge, so the
  sprite is tidied rather than regenerated. Ran S1 + refine 0.35 over the full set.

  **`iou_ref` 0.850 against S1's 0.852** — a −0.002 difference across 18 cells, for **double the
  GPU cost**. The item's acceptance also required the eye on outline cleanliness, because `iou_ref`
  is a silhouette statistic and cannot see it: `.staging/p1-s4-refine.png` puts S1 and S4 side by
  side at seed 4102 and they are **visually indistinguishable**. Outlines are equally clean in both.

  Dropped, on both criteria. The flag stays in `generate.py` — it is a general capability and costs
  nothing unused — but no method in the ladder uses it. **A stage earns its place by moving the
  number; this one does not**, and keeping it would have doubled the cost of every later phase.

  **Incidental finding worth its own note:** the elephant at seed 4102 has a **green plate that
  survived background keying** in both S1 and S4, which is what its 0.680 actually measures. That is
  a *keying* failure, not a shape failure — the corner flood-fill did not recognise the plate — and
  it is the same cell that failed the gate in S0. Recorded so no later phase reads elephant/4102 as
  evidence about silhouettes.

- **2026-08-08 · P1/S5 · Best-of-N works, and the selector that generalises captures under half of
  it.** Computed from the S1 cells already on disk rather than by generating more — three seeds per
  species *is* a best-of-3 sample, so the question needed no new GPU time.

  | selection | mean `iou_ref` | Δ vs single shot | available for untrained species? |
  |---|---|---|---|
  | single shot (expected value of one attempt) | 0.852 | — | yes |
  | best-of-3 **by `iou_ref`** | **0.949** | +0.097 | **no** — needs the real sprite |
  | best-of-3 **by the structural gate** | 0.892 | +0.040 | **yes** — reference-free |

  The item asked for the comparison to be fair on GPU cost: best-of-3 costs 3×, and the honest
  baseline for one attempt is the *mean* of the three, which is what 0.852 is.

  **The split between the two selectors is the point.** Choosing by `iou_ref` is the strongest
  result in the stream so far — but it is choosing by the answer key, so it cannot be part of a
  method that must serve animals with no corpus sprite ([F2](forks.md#f2)). The **gate** selector —
  prefer `blobs == 1`, then the most uniform plate — uses nothing but the generated image, works for
  any species, and still recovers **+0.040 of the +0.097** available. That is the number [P4](todo.md)
  inherits; the 0.949 is not.
