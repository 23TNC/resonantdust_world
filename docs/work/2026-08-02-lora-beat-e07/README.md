# Beat `e07` — build the best LoRA we can, fresh, on a non-photoreal base and the 3090 — 2026-08-02

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{prep_train,build_quad,lora_eval,generate}.py`
+ a new `rd_quadruped`. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Follows
[`2026-07-26-sprite-eval-trust`](../2026-07-26-sprite-eval-trust/README.md) (done 18/18) and
[`2026-07-25-sprite-gen-quality`](../2026-07-25-sprite-gen-quality/README.md) (P4/P5 still open
there, not in scope). Shipping LoRA is `rd_quadruped_e07` and stays so until something beats it._

## What the user asked for

> "write this up with the intention of **starting new**. We are **not intending to create a valid
> comparison**, we are intending to create the **best lora we can** utilizing lessons learned from
> our previous runs and advantages of the new hardware."

Plus, from the same conversation: use the 3090's **bf16** and **memory**, train against one of the
**non-photoreal** base models rather than a realistic one, and **emit samples every epoch so the
user selects** rather than firing and forgetting.

**This stream therefore changes many variables at once, deliberately** ([F2](forks.md#f2)). It is
not an experiment and will not attribute its result. The accepted cost, stated once: **if the
output is worse, we will not know which change did it.** The exit condition is not "we learned
why", it is "the user looks at the sprites and they are better."

## The lesson that reframes everything: we have been training against a photorealism model

`bin/lib/generate.py` line 34:

```python
MODEL = "sdxl/cyberrealisticXL_v80.safetensors"
```

Before 2026-07-28 the box held **no other SDXL checkpoint** — so every `rd_quadruped` run, `e07`
included, was fitted onto a **photoreal portrait finetune**, while the target art is flat regions
bounded by hard black outlines: *vector graphics rendered as bitmaps*, in the P2 log's own words.

That reframes three days of failures. The LoRA was not only learning our style — it was spending
capacity **fighting a prior pulling the opposite way**. It also fits the specific failure shapes:
photoreal bases love a framed portrait subject and a naturalistic sitting pose, which is precisely
what runs 4 and 5 produced on south and east.

Not proven, and this stream will not try to prove it ([F2](forks.md#f2)). It is the single most
plausible lesson available and it is cheap to act on, which is enough.

## What we carry forward, and what we drop

| lesson | source | carried as |
|---|---|---|
| **`iou_ref` agrees with the eye** — called both run-4 failures 6/6 where `d_aspect` scored a tie | `sprite-eval-trust` 18/18 | the A/B statistic; the **eye ratifies** ([F3](forks.md#f3)) |
| **ESRGAN upscale beats LANCZOS** — outline sharpness 43.1 → 90.1 (2.1×), no staircasing | `sprite-gen-quality` P2 | **kept** — the one P2 change with no failure attached |
| **Pinned `fill` correlates with every failure**; `e07`'s data sat at sd **0.148**, v2 at 0.0032, run-5's jitter only reached 0.027 | [I1](issues.md#i1) | **restore v1-range scale variance**, not the timid jitter |
| **Photoreal base** | this stream, above | **replaced** ([F4](forks.md#f4)) |
| **11 GB ceiling shaped every config** — run-4 held 10,379 of 11,264 MiB | [I4](issues.md#i4) | dim/α, batch and precision re-chosen on 24 GB, not inherited |
| **Look at the images before accepting a verdict** — 4 occasions where they overturned the numbers | both streams | per-epoch samples; the user picks the **epoch** |

Dropped: single-variable attribution. It was the right tool for `sprite-eval-trust`, which had a
specific hypothesis to kill. We have no hypothesis worth 25 GPU-hours — we have a pile of
well-supported lessons and new hardware, and the user's call is to spend them all at once.

## `e07` is the bar, not the control

We are not running a controlled comparison, but we still need to know whether the new thing is
better. `iou_ref` scores a generated silhouette against the **real corpus sprite** — it never looks
at the model that made it, so it works **across base models**. `e07`'s scoreline stands as a
target no matter what a challenger is built on:

> **26/36 gate · `iou_ref` 0.753 · winner on all six A/B species**

Clear that and the new LoRA ships. Miss it and we say so plainly, exactly as three prior runs did.

## Which base model — the pick, and how confident it is

On the box (all pulled 2026-07-28): **Illustrious-XL-v1.0**, **animagine-xl-4.0**, and
`sd_xl_base_1.0` as a clean reference, beside the incumbent `cyberrealisticXL_v80`.

**My pick is Illustrious-XL v1.0** ([F4](forks.md#f4)). Both it and Animagine are Danbooru-tag
bases with strong **flat-colour, hard-outline** priors — the exact quality our corpus has and the
exact quality `cyberrealisticXL` lacks. Between them, Illustrious has the better prompt adherence
and is the more common foundation for downstream LoRA training, while Animagine 4.0 is tuned more
narrowly to anime character portraiture. Our subjects are quadrupeds in an oblique game-sprite
convention — off-distribution for both, since Danbooru is overwhelmingly human characters — so the
more *steerable* base is the better bet.

**Confidence: this is reasoning, not measurement.** I have not generated a single image with
either. [P1](todo.md) settles it in minutes with a no-LoRA probe across all three rather than by
arguing — the same "measure, don't assume" that chose ESRGAN and caught the silent no-op.

Both are SDXL-architecture, so the existing SDXL ControlNets on the box carry over —
`controlnet-union-promax` and `mistoline-lineart` are both present, and MistoLine is a
line-art-specialised ControlNet that suits this art better than the generic union model. P1
verifies that rather than assuming it.

## Design stance

- **Best output wins; attribution is explicitly not a goal.** Do not re-introduce single-variable
  runs here. If a future stream needs to know *why*, it can bisect then, on a model worth bisecting.
- **The user picks the epoch.** Samples every epoch on a pinned prompt/seed set. `e07`'s own
  epoch-7 provenance says the best checkpoint is mid-run, and end-of-run is not automatically it.
- **Fixed prompts, fixed seeds, fixed species across every sample sheet**, so the user is choosing
  between models rather than between rolls of the dice.
- **Say plainly if it loses.** Three runs have. A fourth loss is a result; shipping a worse model
  because it was expensive is the only real failure available here.

## Future intent this plan must not trim

- **Illustrious-XL is NOT eliminated.** The user's words on choosing Animagine: *"Both models have
  weaknesses. I agree we can start with animagine-xl-4.0 but I am not ruling out future attempts
  with illustrious."* So this is a starting point, not a verdict. The probe measured a **prior**,
  and a prior only correlates with what a base LEARNS — Illustrious could still train better while
  probing worse. Switching costs two constants (`eval_set.json:model`, `generate.py:MODEL`), and
  that cheapness is deliberate: keep it that way.

- **The two-stage architecture** stays the destination: generate a line drawing → use it as the
  ControlNet source → render detail in a second pass. The open
  [`lineart-lora`](../2026-07-27-lineart-lora/README.md) stream feeds that and is a *different
  product* — do not fold this into it. A non-photoreal base likely helps it too, which is a reason
  to settle the base question here first.
- **The anteater generalisation check survives.** A species with no body-plan analogue in the
  corpus (`--control auto` declines it at 0.53) is the honest test; no aggregate replaces it.
- **Grading functions return on the user's sequencing** — "once we have something somewhat
  workable." `iou_ref` carries us until then; nothing here builds a new metric.
