# Completed — verification log

_Append-only. Dated entries: what landed and **how it was checked**. Item text lives in
[`todo.md`](todo.md) with its box ticked._

_(nothing landed yet — the stream opened 2026-07-26 with the plan only)_

## 2026-07-26 — P0 complete (the ruler now sees the pose)

**`iou_ref`** — IoU of the generated silhouette against the real corpus sprite, both cropped to their
own bbox and rescaled into a common grid first (normalising position and scale, so the metric asks
"is this the same *shape*" rather than "is it in the same place").

**It catches both run-4 failures that the gate missed:**

| | e07 | run-4 | result |
|---|---|---|---|
| **south** (framed busts) | 0.792 / 0.903 / 0.801 / 0.808 / 0.745 / 0.816 | 0.691 / 0.824 / 0.752 / 0.656 / 0.741 / 0.815 | e07 higher **6/6** |
| **east** (pose drift) | mean **0.694** | mean **0.619** | e07 higher **6/6** |

East is the important one: `d_aspect` scored it a **tie** (34.8% vs 34.1%) because it takes an
absolute difference. `iou_ref` separates it cleanly on every species.

**`d_aspect_signed`** confirms the mechanism directly: e07 east **+27.9%** (14/18 positive — too
long, a proportion wobble) versus run-4 **−29.7%** (only 3/18 positive — too compact, i.e. sitting
rather than lying). The sign was the whole signal the unsigned metric discarded.

**Gate calibration: no change adopted ([F3](forks.md#f3)).** Six variants scored against the 67
labelled sprites; the current gate's 3 errors beat every alternative, and adding `iou_ref` cost
false negatives (5–7 errors). The resolution is that two questions were being conflated — *is this
sprite usable* (gate) versus *is this model better* (`iou_ref`). `iou_ref` is adopted as the **A/B
ranking metric**, not a pass/fail check.

This is the second time a metric with real discriminative power turned out to be wrong as a hard
gate; the first was `d_fill` in the predecessor.

## 2026-07-26 — P1 complete (the visual check is part of the harness now)

`lora_eval.contact_sheet(cells, out)` tiles labelled images into one reviewable PNG — column headers
from the config, rows from the subject, blank cells for anything missing so a long run never aborts
on one bad file.

**Wired in with no extra flag:**
- `lora_eval --pipeline` writes `sheet.png` beside `sweep.csv`, one column per dn/cn/cn_end config
- `generate.py --candidates` writes `sheet.png` into the output kind, **with the gate verdict in the
  column header** (`e ok`, `s REJECT`)

Both print the sheet path with a line telling the reader to look at it *before* trusting the table.

**Verified:** a 2-config × 6-species call produced a readable 409×1243 grid; a live 3-candidate
`generate` run wrote a 115 KB sheet showing all 6 sprites with their verdicts.

The sheet immediately earned itself — the e07-vs-run4 east grid makes the pose drift obvious at a
glance (e07 lying low, run-4 sitting up with a raised head, curled cat, upright fox) and also shows a
**white frame around run-4's tiger on EAST**, i.e. the [I3](issues.md#i3) frame artefact is not
confined to south as the earlier analysis assumed.

## 2026-07-26 — P2 (jitter) + P3 (diagnosis)

**Deterministic fill jitter.** `prep_train.jittered_fill(fill, jitter, key)` derives the per-image
fraction from a SHA-256 of the source filename — reproducible by construction, because an
irreproducible dataset makes every later A/B unfalsifiable. Verified: same file twice gives an
identical value; three different files give three different values; over 400 keys, mean 0.847,
**sd 0.029** (the item asked for 0.02–0.05, against the pinned build's 0.003). `build_quad.py`
routes through it, so both datasets share one prep path.

**Pose drift diagnosed ([I5](issues.md#i5))** — three findings:
1. **The prep is exonerated** — training-image aspect matches the raw corpus to within 0.03 on all
   six species, and the sheet shows every training image in the correct lying profile.
2. **The drift is inference-resolution-dependent** — run-4 east signed error is **+10.6% at 768** but
   **−29.7% at 1024**, matched seeds. The original A/B therefore judged run-4 at its *worst*
   resolution, and this inverts the resolution-matching rule the predecessor measured.
3. **It does not rescue run-4** — re-scored with both at 768, e07 still wins on gate (26/36 vs
   19/32), `iou_ref` (0.753 vs 0.654), east iou (0.694 vs 0.599) and south iou (0.811 vs 0.717).

**A defect in the new contact sheet, found by using it:** RGBA sprites were converted straight to RGB,
rendering transparency **black** — the corpus column looked broken when the data was fine. Now
composited on white first. A review tool that misrepresents correct art is worse than none.

## 2026-07-27 — P2 complete + run-5 launched (the single-variable test)

**Jittered 1024 dataset built and verified.** 459 images / 459 captions, uniform 1024², longer-side
fraction **0.847 ± 0.0268** (range 0.798–0.896) and outline sharpness **88.8**. Acceptance asked for
sd in 0.02–0.05 with sharpness ≥85: **PASS**. The pinned build's sd was 0.003 — the learnable
constant is gone, and the P2 sharpness gain is retained.

**Run-5 launched as a true single-variable test.** Identical to run-4 in *every* setting — fresh,
1024², dim 48 / alpha 24, grad-accum 4, LR 3e-5 cosine, 15 epochs, same prompts, same seed — with
**only the dataset changed** (jittered fill instead of pinned). Run-4 changed data + rank +
resolution simultaneously and therefore could not attribute its own failure; this run can.

Confirmed started: 765 effective images, 15 epochs, 2,880 optimizer steps, VRAM 10,379 MiB (matching
run-4 exactly, as expected from an identical config), 54 °C.

**What the result will mean either way:**
- **If the frame artefact and sitting poses disappear** → [I5](issues.md#i5)'s leading hypothesis is
  confirmed: a pinned fill taught the model to draw its margin, and that margin pushed composition
  inward. Rank 48 and 1024 are exonerated.
- **If they persist** → the margin is *not* the cause, and the remaining suspects are rank 48 and
  1024-training, which would then need isolating individually.

Either outcome is informative, which is the point of changing one thing.

## 2026-07-27 — early read on run-5: INCONCLUSIVE at epoch 1 (recorded, not spun)

Pulled run-5's first sample set (~20 min in) hoping for an early verdict on the frame hypothesis,
since run-4's artefact was visible in its own early samples. Compared side by side at matched epoch
(`.staging/run5-samples/ep1_compare.png`).

**It does not settle anything, and the honest call is to say so.** At epoch 1 both runs are barely
trained — neither produces the flat RimWorld convention, both render generic standing animals, and a
white halo appears around subjects in **both**. At this stage that halo is as plausibly undertrained
output as a learned margin, so it cannot discriminate. Run-4's frame artefact only became
unambiguous around epochs 7–14.

The fair comparison point is **epoch 7+**, where run-4 samples already exist for the same five
prompts and seeds. That is ~2.3 h into the run.

Recording this rather than reading a hypothesis into five rough images — the whole stream exists
because measurements got trusted past what they could support.

## 2026-07-27 — P4 complete (run-5 A/B; e07 retained)

Run-5 finished: 15 epochs, **4 h 35 m**, loss 0.016. Installed as `rd_quad3_e15`, A/B'd against `e07`
and run-4 over 6 species × e/s × 3 seeds with the frozen P0 gate plus `iou_ref` and signed aspect.

**Verdict: `e07` ships** ([F4](forks.md#f4)) — 26/36 gate, `iou_ref` 0.753, winner on all six species.

**The images were reviewed before the verdict was accepted, as the plan requires.** The auto-emitted
sheet shows run-5 still producing framed portraits on south and sitting poses on east — visually
identical to run-4. The numbers and the images agree, which is the first time in this project they
have on a retrain verdict, and is the point of P0/P1.

**The hypothesis is refuted, cleanly.** Jitter changed the data measurably (fill sd 0.003 → 0.027)
and changed the output not at all. Because run-5 varied exactly one thing, that is attributable —
unlike run-4, which varied three.

---

### Stream complete — 18/18

The evaluation now agrees with the eye (`iou_ref` called both run-4 failures 6/6 where `d_aspect`
scored a tie), the visual check is part of the harness, and a wrong hypothesis was killed by a
controlled experiment rather than argued about.
