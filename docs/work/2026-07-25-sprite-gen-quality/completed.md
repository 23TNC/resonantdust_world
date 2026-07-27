# Completed — verification log

_Append-only. Dated entries: what landed and **how it was checked**. Item text lives in
[`todo.md`](todo.md) with its box ticked._

_(nothing landed yet — the stream opened 2026-07-25 with the plan only)_

## 2026-07-26 — P0 complete (the ruler is calibrated, and two assumptions were refuted)

**Calibration set** — `.staging/gate-cal/labels.csv`, **67 sprites** (58 good / 9 bad), each row
carrying a `why`. Labels are this session's verdicts: the user's explicit calls where given ("the
row 2+ bears are perfect", "row 1 wolf south is pretty good", the anteater "doesn't quite look
right"), otherwise a visual judgement made against the montages. **Known weakness: these are not
blind labels**, and the good/bad split is imbalanced 58/9 because most generations genuinely are
fine. Deliberately ambiguous sprites (wombat, meerkat, the hornless oryx south) were **excluded**
rather than forced into a bucket. The tf-baseline sprites were re-examined visually before labelling
— they had only ever been seen as numbers — which confirmed all five template-free south failures
render a *side view* where a front view was asked for.

**`iou_control` and `hull_solidity` implemented, then refuted as gate checks** —
[I5](issues.md#i5), [I6](issues.md#i6). The anteater's bad views score *higher* `iou_control` (0.789
/ 0.732) than its good east (0.703), because IoU asks "did it follow the control?" and the control
was the wrong shape to begin with. `hull_solidity` separates good from bad at only 0.30 sd. Both are
kept as reported metrics — they measure real things — but neither is gated.

**Gate re-tuned on evidence ([F3](forks.md#f3)): errors 4 → 3.** `solidity` removed (0.19 sd of
signal, and it caused the oryx false negative — verified now passing). `d_fill` tried and rejected:
despite the second-best marginal separation (0.83 sd) it doubles-to-triples total error as a hard
gate. `blobs` retained as a zero-cost regression alarm.

**The residual 2 false positives are structural, not a tuning failure ([I7](issues.md#i7)):** for an
*unseen* species the gate's reference is a proxy from another species, so the anteater was judged
against an Elephant and its blob legitimately resembles elephant proportions. Geometry gating is
sound for corpus species and weak for exactly the unseen case template-free generation exists to
serve. Recorded rather than tuned away.

### Two caveats on the P0 boxes

**The `iou_control` item's acceptance FAILED, and the box is ticked anyway.** Its criterion was
"the blobby anteater south scores lower `iou_control` than its own good east" — the measurement came
back the opposite way (0.789 vs 0.703). The *work* is done and the metric ships as a reported value;
what is retired is the plan's **premise** that it would catch this case. That is a plan error, logged
as [I5](issues.md#i5). Ticking the box without this note would have misrepresented a refutation as a
success.

**Removing `solidity` has a real cost, accepted knowingly.** The item asked to *replace* it with a
horn-tolerant variant; the calibration data said the honest move was to **delete** it, since both it
(0.19 sd) and the hull-based replacement (0.30 sd) carry almost no signal. The consequence: a
single-component but genuinely *wispy* sprite is no longer caught — `blobs` only catches a shape
that fragments into separate pieces. No such sprite exists in the 67-sprite calibration set, so the
risk is untested rather than disproven. If wispy output shows up later, the fix is a targeted check,
not restoring a threshold that rejected good art.

## 2026-07-26 — P0 item 5: multi-seed baselines

`tf_baseline.py --seeds N` now rolls each species+direction N times, records the seed and the gate
verdict per row, and prints a **per-cell pass rate**. Verified with `--mode none --seeds 3`: **90
rows** (30 cells × 3), overall gate pass rate **74/90 = 82%**.

The aggregate barely moved from the single-seed run (83%), but the per-cell view is new information
the old baseline could not express — it separates *consistent* failure from *stochastic* failure:

| pattern | cells |
|---|---|
| consistently fails | **cat south 0/3** |
| mostly fails | bear south 1/3, pig south 1/3 |
| coin-flip | fox south 2/3, horse south 2/3, zebra south 2/3, several norths 2/3 |
| always passes | every east except cat (2/3); wolf, tiger all three |

This matters for where effort goes: a 0/3 cell is a capability gap (no amount of sampling fixes it),
while a 2/3 cell is exactly what `--candidates` already solves for free. The single-seed baseline
reported both as the same thing.

## 2026-07-26 — P1 complete (auto built and measured; the plan's premise refuted)

**Shape-similarity search** added to `silhouette_bank.py`: 32×32 occupancy IoU blended 0.7/0.3 with
aspect agreement, cached per direction. Verified — `Bear/e`'s nearest neighbours are Bear (1.000),
Capybara (0.899), `AEXP_BlackBear` (0.887).

**`--control auto`** is a two-pass resolve: generate one template-free **probe** (east, the only
direction that passed 10/10 in both modes), match it against the bank, then use that species for all
three directions. One species for the whole set — not a per-direction match — costs one extra
generation instead of three and keeps e/s/n coherent.

**A coherence bug found and fixed while testing:** the gate was still scoring against a hand-passed
`--ref`, i.e. measuring the output against a species whose silhouette was never used. It rejected a
perfectly good auto east purely for not resembling the Elephant it was never shaped by. `auto` now
sets the gate reference to its own pick unless `--ref` is explicit (and warns when they disagree).

**The acceptance criterion was NOT met, and the box is ticked with that recorded.** `auto` was
supposed to beat `family:pachyderm` on the anteater. It was **worse** — Gorilla removed the snout
entirely ([I9](issues.md#i9)). It also picked a **hedgehog for a wolf** ([I8](issues.md#i8)).

**The salvage is the part nobody planned: knowing when to decline.** Calibrated from real data —
species with a corpus twin score 0.705–0.967, the anteater 0.532 — `AUTO_MIN_MATCH = 0.65` routes
the anteater to **no control**, which is the visually best of the three modes (recognisable snout,
tail and shoulder marking in all three views, versus cape-blobs from the Elephant and snoutless
blobs from the Gorilla). Verified: `auto DECLINED (best match Gorilla 0.53 < 0.65)`.

**[F4](forks.md#f4): `FAMILY_REP` is KEPT.** It encodes semantics the measurement cannot recover.
`auto` ships as an addition whose real value is the decline path.

## 2026-07-26 — P2 (part 1): upscale + scale normalisation

**Upscaler chosen by measurement, not assumption.** Compared LANCZOS / BICUBIC / NEAREST /
two-stage NEAREST→LANCZOS / LANCZOS+unsharp / ESRGAN on the worst case (64px Cat) and a typical one
(128px Bear), scoring mean outline gradient and inspecting each:

| method | 64px Cat | 128px Bear | verdict |
|---|---|---|---|
| LANCZOS (old default) | 21.7 | 31.5 | **zero** strong-edge pixels — outline erased |
| BICUBIC | 21.9 | — | same class |
| NEAREST | 107.4 | — | sharp but visibly **staircased** curves |
| 2/4/8×-then-LANCZOS | 30–54 | — | progressively blockier, no sweet spot |
| **ESRGAN (Remacri, on the box)** | **109.8** | **120.7** | **crisp AND smooth — chosen** |

The box only has a general-purpose Remacri, not an anime/line-art model, and it is still decisively
better. `--upscale lanczos` is retained for A/B, and the prep **falls back to LANCZOS automatically**
if ComfyUI is unreachable so it never hard-fails on a box outage.

**Scale normalised on the longer side** to 0.85 of the frame, aspect preserved — achieved
**0.848 ± 0.0009** (min 0.846, max 0.849) on the smoke set.

**Two plan errors in my own acceptance criteria, corrected in place ([I10](issues.md#i10)):** the
gradient criterion would have selected staircased NEAREST, and the "bbox area = 0.80" criterion is
geometrically unsatisfiable without distorting animals.

**A silent no-op bug, found only by measuring ([I11](issues.md#i11)):** scale normalisation did
nothing for the whole corpus because of two stacked alpha traps — `getbbox()` on RGBA counts any
non-zero channel, and the corpus's transparent background is alpha ≈ 3 rather than 0. The output
looked plausible throughout; only checking the *achieved* fraction exposed it.

## 2026-07-26 — P2 complete (dataset rebuilt and measured)

`.staging/quad-lora-train2/` rebuilt through `build_quad.py` → `prep_train.normalise()`:
**459 images / 459 captions** across the three repeat-weighted folders (153 each), 765 effective
images per epoch, family counts unchanged.

**Before/after over a 120-image random sample of each set:**

| | longer-side fraction | | outline sharpness | |
|---|---|---|---|---|
| | mean | sd | mean | sd |
| OLD (v1, LANCZOS, unnormalised) | 0.755 | 0.1478 | 43.1 | 9.6 |
| **NEW (v2, ESRGAN + normalised)** | **0.848** | **0.0032** | **90.1** | 14.3 |

**Scale consistency improved 46×** (sd 0.148 → 0.003) and **outline sharpness 2.1×** (43.1 → 90.1).
The sharpness gain is smaller than the 4–5× measured on a single 64px source, which is expected: most
of the corpus is 256px, so the average upscale factor — and therefore the damage LANCZOS was doing —
is milder than the worst case.

The plan's stated criterion was "`fill` standard deviation below 0.02". That criterion was replaced
([I10](issues.md#i10)) because bbox *area* cannot be equalised without distorting aspect; the
longer-side equivalent is **sd 0.0032**, comfortably inside the spirit of it.

**Spot-check, 6 species old vs new** (`.staging/p2-spot/before_after.png`): Cat (64px worst case),
Bear, Wolf_Timber, Tiger, Elephant, Giraffe. Every new sprite is visibly crisper and consistently
framed; **no halo, no clipping, no aspect distortion** found. The most visible change is that
subjects now fill the frame consistently — the old Tiger and Giraffe were noticeably small.

## 2026-07-26 — P3 complete (run-4 trained, A/B'd, and NOT shipped)

Run-4: fresh, 1024², dim 48 / alpha 24, grad-accum 4, LR 3e-5 cosine, 15 epochs on the P2-rebuilt
1024 dataset. **4 h 39 m**, final loss 0.0169 (identical to run-3's). VRAM held at 10,379 MiB of
11,264 — within 26 MiB of the probe's prediction.

**A/B verdict: `e07` stays** ([F5](forks.md#f5)). 26/36 vs 25/36, and 32.5% vs 42.8% mean d_aspect.

**But the aggregate hid the real story.** By direction, run-4 **wins east** (15/18 vs 12/18) and
**loses south** (10/18 vs 14/18) — the entire deficit is one direction. Visually
(`.staging/ab4/visual.png`) run-4's east sprites are richer and better drawn than the shipping
model's; its south sprites are head-only portraits inside a drawn white rectangle
([I13](issues.md#i13)).

**Root cause is mine:** pinning `fill = 0.85` gave all 459 training images an identical ~7.5% white
margin, which the model learned as a feature and now draws. The P2 fix that removed arbitrary scale
introduced a learnable constant.

**Method note worth keeping:** the numbers alone would have been reported as "run-4 is slightly
worse". Only looking at the images revealed "better on east, broken on south" — a completely
different conclusion with a completely different next step. The gate passed 10/18 of the broken
south sprites because a framed bust can have a plausible bbox aspect.
