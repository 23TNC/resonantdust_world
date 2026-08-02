# Beat `e07` — find why every retrain loses, with the 3090 and the user's eyes — 2026-08-02

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{prep_train,build_quad,lora_eval,generate}.py`
+ the `rd_quadruped` LoRA. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Follows
[`2026-07-26-sprite-eval-trust`](../2026-07-26-sprite-eval-trust/README.md) (done 18/18) and
[`2026-07-25-sprite-gen-quality`](../2026-07-25-sprite-gen-quality/README.md) (P4/P5 still open
there, not in scope here). Shipping LoRA is STILL `rd_quadruped_e07`._

## What the user asked for

> "train a new lora using the 3090 the same as we did on the 2080ti, but this time with better
> hardware and feature support … spit out samples for each generation … I select the best and
> re-train from that generation … repeating until we have something somewhat workable. Once we
> have something somewhat workable we'll look back into functions to grade the systems work."

The instinct behind it — **stop firing and forgetting, put the user's eyes in the loop** — is
right, and this project has four separate occasions where looking at the images overturned the
numbers. This stream keeps that and changes the *shape* of the loop, for the reasons below.

## Three retrains have now lost to `e07`, and the reason is still unknown

| run | dataset | config | result |
|---|---|---|---|
| `e07` **(ships)** | **v1** — LANCZOS, unnormalised, fill sd **0.1478** | — | 26/36 gate, `iou_ref` **0.753**, wins all six species |
| run-3 | v2 | — | did not ship |
| run-4 | v2 — ESRGAN + normalised, fill sd 0.0032, 1024² | dim 48/α 24, ga 4, LR 3e-5 cosine, 15 ep, 4 h 39 m | 25/36. **Wins east** 15/18 vs 12/18, **loses south** 10/18 vs 14/18 |
| run-5 | v2 + jittered fill (sd 0.027) | same as run-4 | 4 h 35 m. Visually identical to run-4 |

**The common factor in all three failures is the P2 dataset rebuild.** `e07` is the only model
trained on **v1**. Everything since has been trained on **v2**, and everything since has lost.

That is not a hypothesis anyone has tested, because P2 bundled **three** changes at once — upscaler
(LANCZOS → ESRGAN), scale normalisation (unnormalised → longer side pinned to 0.85), and resolution
(768 → 1024²). Runs 3, 4 and 5 all inherit all three. Run-5 was a genuinely clean single-variable
test, but of a **fourth** variable (jitter *within* normalisation), not of any of the three.

**And the jitter test did not restore v1's condition** ([I1](issues.md#i1)). It moved fill sd
0.003 → 0.027. `e07`'s data sat at **0.148** — still 5.4× more variable than the jittered set. So
"scale variance doesn't matter" is refuted only over the range actually tested, which stopped well
short of the shipping model's.

## What this stream does instead of ten generations

**Decompose P2 one variable at a time against the `e07` baseline**, with the user picking the
winner from images at each step. Selection over *chosen differences*, not over checkpoints.

Why not the loop as described — see [F1](forks.md#f1) in full, but in short: "re-train ten more
from the best" doesn't map onto gradient descent. There is no crossover between two LoRAs, and
continuing training from a winner is not mutation — it is the same descent going further, which on
459 images at 15 epochs means memorising. Ten runs is also 20–25 GPU-hours **per round** on the
3090 and 30 sample sets for the user to review, spent searching an axis (config) that has never
been shown to be the problem, while the axis that correlates perfectly with every failure (the
dataset) goes untested.

## Design stance

- **One variable per run. Enforced.** Run-5 is the model: it varied exactly one thing, so its
  result was *attributable*, and it killed a wrong hypothesis in one run. Run-4 varied three and
  produced an argument. Every run here names its single delta in `completed.md` before it starts.
- **The user picks from images; the metrics are the tiebreak, not the judge.** `iou_ref` now agrees
  with the eye and is kept as the A/B statistic — but the four occasions where images overturned
  numbers all stand, and the harness already emits a visual sheet. The eye ratifies.
- **Per-epoch samples are the selection surface, and they already exist.** Runs 4 and 5 both
  emitted them on the same five prompts and seeds. `e07`'s epoch-7 provenance is itself evidence
  that the best checkpoint is mid-run — so sample every epoch, and let the user pick the *epoch*,
  which costs one run rather than ten.
- **Fixed prompts, fixed seeds, fixed species set across every run.** Otherwise the user is
  selecting on sampling noise.
- **Say plainly when a run loses.** Three have. A fourth that loses is a result, not a failure —
  what would be a failure is shipping one because it was expensive.

## What the 3090 actually buys

Not just speed. Run-4 held 10,379 MiB of the 2080 Ti's 11,264 — it was **memory-bound**, and every
config choice since has been made under that ceiling. With 24 GB: grad-accum 4 can collapse to a
real batch, `dim`/`alpha` are no longer capped by VRAM, and **bf16 becomes available at all**
(Turing has none; Ampere has it). bf16 over fp16 removes the loss-scaling failure mode rather than
making anything faster. These are new *degrees of freedom*, which is exactly why they must not all
be turned at once — see the single-variable rule above.

## Future intent this plan must not trim

- **The two-stage architecture** the user described stays the destination: generate a line drawing
  → use it as the ControlNet source → render detail in a second pass. The open
  [`lineart-lora`](../2026-07-27-lineart-lora/README.md) stream feeds that, and this stream must not
  fold into it — a structure-only LoRA and a better `rd_quadruped` are different products.
- **The anteater acceptance test survives.** A species with no body-plan analogue in the corpus
  (`--control auto` declines it at 0.53) is the honest generalisation check, and no aggregate score
  replaces it.
- **Grading functions come back later, by the user's own sequencing** — "once we have something
  somewhat workable." `iou_ref` stays in use meanwhile; nothing here builds a new metric.
