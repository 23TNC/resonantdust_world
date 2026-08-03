# Issues — beat e07

_Problems hit, and what the evidence actually showed. Findings recorded at plan time are marked as
such — they were read out of the prior streams' logs, not measured under this one._

## I1 — The jitter refutation stopped 5.4× short of `e07`'s own condition {#i1}
_2026-08-02 · read at plan time from [`sprite-eval-trust`](../2026-07-26-sprite-eval-trust/completed.md)_

Run-5 tested "does scale variance matter?" by jittering `fill`, and the answer was recorded as a
clean refutation — correctly, because it varied exactly one thing. But the range it covered is not
the range that separates the shipping model from the failures:

| dataset | fill sd |
|---|---|
| v1 — what `e07` was trained on | **0.1478** |
| v2 (P2 rebuild, pinned) — runs 3/4/5 | 0.0032 |
| v2 + jitter — run-5 | 0.027 |

Run-5 moved 0.003 → 0.027. `e07`'s data sat at **0.148**, still **5.4×** more variable than the
jittered set and **46×** more than the pinned one. So the finding that survives is "jitter of this
magnitude changes nothing", not "scale variance doesn't matter" — the latter was never tested at
`e07`'s magnitude.

This is not a criticism of that experiment, which did what it set out to do. It is the reason
scale normalisation is [F2](forks.md#f2)'s first target rather than a closed question.

## I2 — `prep_train` silently swaps the upscaler when ComfyUI is down {#i2}
_2026-08-02 · read at plan time from `bin/lib/prep_train.py`; **live right now**_

The prep "falls back to LANCZOS automatically if ComfyUI is unreachable, so the prep never
hard-fails on a box outage." That is a sensible default for a convenience script and a **trap for
a single-variable experiment**: the upscaler is one of the three P2 variables this stream is trying
to isolate, and a rebuild run while the box is down silently produces a *different dataset* under
the same command.

It is live: ComfyUI is currently **not running** on the unraid box, is **absent from the autostart
list**, and its restart policy is `no`. So the very next dataset rebuild would take the fallback
path by default.

[P0](todo.md) makes the fallback loud (opt-in via an explicit flag) before any rebuild happens.
The same class of bug already cost this project once — P2's scale normalisation was a **silent
no-op for the whole corpus** and looked plausible throughout, found only by measuring the achieved
fraction.

## I3 — P2 bundled three changes, so runs 3/4/5 are not attributable {#i3}
_2026-08-02 · read at plan time from [`sprite-gen-quality`](../2026-07-25-sprite-gen-quality/completed.md)_

The P2 rebuild changed, in one step:

1. **upscaler** — LANCZOS → ESRGAN (Remacri), outline sharpness 43.1 → 90.1
2. **scale normalisation** — unnormalised → longer side pinned to 0.85, fill sd 0.1478 → 0.0032
3. **resolution** — 768 → 1024²

Every model trained since inherits all three, and every model trained since has lost to `e07`.
No run has isolated any of them. Run-5's single-variable discipline is exactly the right tool and
was pointed at a fourth variable (jitter *within* 2) instead.

Each of the three was independently well-justified by measurement — this is not a claim that P2 was
wrong. It is that "measurably better inputs" and "a better-performing model" were assumed to follow
from one another, and three runs now say otherwise for at least one of them.

## I4 — The 2080 Ti's VRAM ceiling shaped every config decision on record {#i4}
_2026-08-02 · read at plan time_

Run-4 held **10,379 MiB of 11,264** — within 26 MiB of its predicted budget. So `dim 48 / alpha 24`
and `grad-accum 4` were not chosen as optima; they were the largest that fit. Any comparison
between those settings and something the 3090 can now hold is a comparison against a constraint,
not against a considered baseline.

Recorded so [P3](todo.md) does not read the old numbers as a tuned starting point — and so the new
headroom gets spent one turn at a time rather than all at once, which would recreate exactly the
attribution problem in [I3](#i3).

## I5 — Every LoRA to date was trained against a PHOTOREALISM base {#i5}
_2026-08-02 · read from `bin/lib/generate.py` + the box's checkpoint dir · **the headline lesson**_

```python
MODEL = "sdxl/cyberrealisticXL_v80.safetensors"   # bin/lib/generate.py:34
```

The box's `checkpoints/sdxl/` holds four files. Three — `Illustrious-XL-v1.0`,
`animagine-xl-4.0`, `sd_xl_base_1.0` — are all dated **2026-07-28**, pulled during the hardware
conversation. The fourth, `cyberrealisticXL_v80`, is dated **2026-03-23**. So for the whole life of
`rd_quadruped` (runs 1–5, `e07` included) the only SDXL checkpoints available were a photoreal
portrait finetune and `sdxlturbo`.

The target art is *"flat regions bounded by hard outlines — vector graphics rendered as bitmaps"*
(the P2 log's own description). A photorealism finetune's prior pulls the opposite way, so the LoRA
was not only learning our convention, it was spending capacity **overcoming its own base**.

It also fits the failure *shapes* rather than just being a plausible story: photoreal bases favour
a framed portrait subject and a naturalistic sitting pose, which is exactly what runs 4 and 5
produced on south and east respectively.

**Not proven, and this stream will not try to prove it** ([F2](forks.md#f2)). It is recorded as the
most plausible lesson available, it is cheap to act on, and [F4](forks.md#f4) acts on it.

## I6 — The A/B that produced the shipping verdict does not exist as code {#i6}
_2026-08-02 · found at P0.4 · **live** — it changes what [P0.5](todo.md) can mean_

`sprite-eval-trust` P4 reports the verdict as *"6 species × e/s × 3 seeds with the frozen P0 gate
plus `iou_ref` and signed aspect"*, and `e07` ships on **26/36 · `iou_ref` 0.753**. The outputs are
on disk (`.staging/ab5/run4_768_<species>_<dir>_<seed>.png` — bear/cat/fox/pig/tiger/wolf ×
e/s × 7700/7701/7702, exactly 36 cells).

**The driver was never committed.** `lora_eval.py` hard-codes a *different* matrix — 4 subjects
(wolf/tiger/bear/cat), 3 directions, one seed each, seeds 1001–1004 — and nothing in `bin/` or
`.staging/` mentions 7700. So the shipping comparison is **not reproducible from the repo**; only
its results survive, in prose and in loose PNGs.

Consequences, both acted on rather than noted:

- **[P0.5](todo.md) cannot "re-run the frozen A/B"** as written — there is no frozen A/B to run.
  It becomes: rebuild the matrix from the pinned set and re-establish the bar, accepting that the
  new number may not land exactly on 26/36 because the driver is a reconstruction.
- **[P0.4](todo.md) is upgraded from hygiene to load-bearing.** `eval_set.json` now holds the
  subjects, directions, seeds, prompts and base checkpoint, and `lora_eval.py` reads it instead of
  its own constants. A future verdict is reproducible by construction.

The same class as [I2](#i2): a thing that mattered lived somewhere nothing could check it.

## I7 — Neither candidate base can draw a front-facing quadruped {#i7}
_2026-08-02 · measured at P1.1 · **may reframe the "south" failure entirely**_

In the no-LoRA probe, **both** bases produced usable side profiles and **both** collapsed on the
front (`s`) view — Illustrious returned angular face emblems, Animagine returned blobby
penguin-like shapes and, on `pig_s`, six disconnected pieces.

That matters because *"south renders head-only portraits"* has been treated as a dataset defect
through runs 3, 4 and 5, and diagnosed variously as a learned white margin and as pinned `fill`.
Here it appears **before any LoRA exists**, on two unrelated bases.

The plausible reading: a front-on quadruped is genuinely rare in any illustration corpus (Danbooru
is overwhelmingly human characters, drawn facing the viewer — an orientation that does not
transfer to four legs), so the prior has little to offer and falls back on faces. If so, south is
partly a **prior** problem, and the LoRA is being asked to teach a view the base actively resists.

**Not acted on in this stream** — [F2](forks.md#f2) says build, don't attribute. Recorded because
it is the first evidence that south's difficulty is not purely ours to fix in the dataset, and
because it predicts south will stay the weak direction whichever base wins.

## I8 — The v3 build came out 52% corrupt, invisibly {#i8}
_2026-08-02 · caught at P2.5 by the eyeball check · **the dataset was rebuilt, not shipped**_

The 701-image v3 set looked finished — 701 png / 701 txt, outline sharpness 92.9, no errors. The
spot-check sheet showed **wolf, tiger and pig as blank white plates**. They are not blank: the
subject is present, correct, and **~25 px inside a 1024 frame**.

Across all 701, the subject-fraction distribution is **perfectly bimodal**:

| subject fills | count | |
|---|---|---|
| < 0.10 | **366** | **52.2%** — corrupt |
| 0.10 – 0.60 | **0** | nothing in between |
| > 0.60 | 335 | 47.8% — correct |

Binary, not a gradient. And **the same inputs upscale correctly on a fresh run** — re-running
`esrgan()` by hand on both a failing (Wolf_Timber) and a passing (Bear) source gave
`128 → 512 → 2048`, subject fraction 1.000 for both. Source size does not predict it: Wolf 128 px
failed, Bear 128 px passed, Tiger 256 px failed, Cat 64 px passed.

So the upscale returns unscaled content intermittently under a long run, and **says nothing**. The
output is a clean white plate with a speck — it survives every check the pipeline had, including
the sharpness measure, because a mostly-white image has few edges but the few it has are sharp.

**Not root-caused, and deliberately so.** It is intermittent, does not reproduce on demand, and
this stream builds rather than investigates ([F2](forks.md#f2)). The fix is the one this pipeline
has now needed three times — **assert the post-condition instead of trusting the step**
([I2](#i2) was the LANCZOS fallback, and the P2-era alpha-bbox no-op was the first). `prep_train`
now measures the subject fraction of every image it writes against its own source and aborts if it
collapsed. Verified against the corrupt wolf (caught) and the good bear (passed).

**Worth stating plainly:** had P2.5 been skipped as a formality, we would have trained ~3 hours on
a set where half the images teach the model that subjects are tiny specks on white — and the most
likely reading of that result would have been "the new base is bad".

## I9 — The drawn FRAME is back on natural-fill data — the pinned-`fill` diagnosis was wrong {#i9}
_2026-08-02 · run-6 epoch samples · **refutes the run-4 root cause**_

Run-4's south failure (head-only portrait inside a drawn white rectangle) was root-caused as
*"pinning `fill = 0.85` gave all 459 images an identical ~7.5% white margin, which the model
learned as a feature and now draws."*

**Run-6 draws the rectangle anyway**, at every sampled epoch, while trained on `quad_dataset` —
which has **natural fill variance (sd ~0.15)**, not the pinned 0.003. Different base model,
different precision, different rank, no warm start, and the artefact is unchanged.

So the margin cannot be the cause: the property said to teach it is absent. Combined with
[I7](#i7) — both candidate bases produced face-emblems for front views with **no LoRA at all** —
the better reading is that **front-view quadrupeds are a prior problem the LoRA cannot fix from
459 images**, and the frame is what the model reaches for when asked to compose a subject it has
no body plan for.

Consistent with everything on record: run-5 jittered the fill and changed nothing
([I1](#i1)), which was read as "the range was too small". It now looks like the hypothesis was
simply wrong.

**East is unaffected and good** — by epoch 10 the tiger is a clean single flat sprite in lying
profile with a hard outline, which is the convention. The failure is direction-specific, not
global.
