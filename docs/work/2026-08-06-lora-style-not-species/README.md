# Teach the LoRA a STYLE, not a bestiary — 2026-08-06

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{prep_train,build_quad,lora_eval,generate}.py`
+ a new `rd_style` LoRA. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings
in [`issues.md`](issues.md). Successor to
[`2026-08-02-lora-beat-e07`](../2026-08-02-lora-beat-e07/README.md), which spent ~20 GPU-hours over
10 runs establishing what is settled below. Shipping LoRA is still `rd_quadruped_e07`._

## The measurement this whole stream rests on

```
200 species · 701 images · mean 3.5 each
   176 of 200 species have EXACTLY 3 images — one per direction
   only 6 species have more than 6
```

The corpus holds **701 examples of the RimWorld convention** and **one example of each species per
direction**. Those are wildly different sample sizes, and we have been training one model to learn
both at once.

It explains the whole of the predecessor stream. The convention is learned reliably in every run —
limbless horizontal masses, hard black outlines, flat fills — because 701 images teach it. Species
identity is never learned, because `tiger` and `cat` have **three images each**. Wolf-east is the
strongest cell in every run not because wolves are special but because the entire canine group
teaches one shared body plan. And **no training configuration ever moved cat or bear**, across ten
runs, because none can.

## The thesis

**Caption for style and body plan only. Drop species names entirely.**

Then the LoRA has 701 examples of exactly one concept, and at generation time the **base model**
supplies "what a tiger looks like" from its own internet-scale training while the **LoRA** supplies
"and draw it our way". That is a division of labour matched to what each side actually has data
for — instead of asking a 3-image token to compete with animagine's own knowledge of tigers.

This is a hypothesis with a cheap test, not a certainty. It costs one training run.

## What the predecessor settled — do NOT re-vary these

Ten runs bought these. Re-opening any of them repeats work already paid for; the evidence is in
[`beat-e07/issues.md`](../2026-08-02-lora-beat-e07/issues.md).

| settled | evidence |
|---|---|
| **Base = `animagine-xl-4.0`** | Illustrious learns detail and loses the convention — sprite-sheet ducks, off-frame tigers, naturalistic snakes (R14) |
| **Full 701 corpus, all six body plans** | quad-only makes `rd_quadruped` a constant on 459/459 images, which teaches nothing (I12) |
| **`dim 64 / alpha 64` (1.0×), `LR 5e-5`** | alpha = dim/2 underfits; 4× signal overcooks (I10) |
| **768, batch 4, bf16, AdamW** | measured on the 3090; 1024 only interpolates a 768 corpus |
| **Stop at ~12 generations** | every run peaks ~g8–10 and degrades; g12 is the reference, not the target |
| **Single stage** | stage two never demonstrably helped; R13 won by keeping the full corpus, R15 by branching later |
| **Structural comparison only** | two identical runs differ by **17.31/255** mean, 0/75 cells bit-identical |
| **Hue is discarded downstream** | the game re-tints via layer maps; only BRIGHTNESS survives |
| **ControlNet does not fix geometry** | it constrains outline, not interior — r11g08's east came out a featureless slab |

## Design stance

- **One concept, 701 examples.** Every change is judged by whether it increases the number of
  examples teaching the thing we want learned.
- **The base is a collaborator, not an obstacle.** The predecessor treated animagine's priors as
  something to overcome. For species identity it is the better-resourced party by many orders of
  magnitude — use it.
- **Judge silhouette mass, not anatomy.** RimWorld animals are solid masses with unbroken bottom
  edges. Leg articulation is a regression, and reading it as progress cost the predecessor a run.
- **The user picks the generation from per-generation samples**, on fixed prompts and seeds.
- **Say plainly if it loses.** Ten runs have failed to beat `e07`. An eleventh that fails is a
  result; shipping something worse because it was expensive is the only real failure available.

## Future intent this plan must not trim

- **The data ceiling is real and this stream does not fix it.** If *specific* animals must come out
  right, that needs ~10–20 images per species per direction — a data-acquisition project, not a
  training one. This stream targets *arbitrary* creatures rendered in our convention, which is what
  the generator is actually for.
- **The two-stage architecture stays the destination**: generate a line drawing → use it as the
  ControlNet source → render detail in a second pass. The open
  [`lineart-lora`](../2026-07-27-lineart-lora/README.md) stream feeds that. A style-only LoRA is a
  better *second-stage renderer* than a species-aware one, so this work serves it.
- **Illustrious is not permanently eliminated** (user, 2026-08-03). It lost on convention, not on
  capability; a future attempt with different captioning is legitimate.
- **The anteater generalisation check survives** — a species with no corpus analogue is the honest
  test, and it is exactly what a style-only LoRA should be *best* at.
