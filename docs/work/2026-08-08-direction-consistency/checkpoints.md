# Checkpoints — the LoRAs this stream renders with

_Provenance for the models this stream treats as frozen inputs ([F1](forks.md#f1)). Training is
settled; nothing here is a subject of this plan._

## Retained on the user's instruction, 2026-08-08

> *"Please ensure we hold onto r20g07 and r20g08 at a minimum."*

Both are promoted into `models/loras` under the naming scheme, **copied explicitly by generation
number rather than by glob** — kohya writes the final generation with no numeric suffix, and a
`-0000*` glob silently drops it ([style-not-species I6](../2026-08-06-lora-style-not-species/issues.md#i6)).

| file | source | size |
|---|---|---|
| `rd_diremph_anima_r20_g07.safetensors` | `output_run20/rd_diremph_anima-000007.safetensors` | 436 MB |
| `rd_diremph_anima_r20_g08.safetensors` | `output_run20/rd_diremph_anima-000008.safetensors` | 436 MB |

**All 12 generations of run-20 are also retained** on the array at `output_run20/` — 5.2 GB against
16 TB free. Keeping the lot costs nothing and means "what did g09 look like?" stays answerable
without retraining.

## Run-20 — direction emphasis, full corpus

`animagine-xl-4.0` · 701 images, style-only captions with the direction tag moved to the caption
head, repeated 3×, `keep_tokens=6` · 768 · `dim 64 / alpha 64` · `LR 5e-5` unet / `2.5e-5` TE ·
batch 4 · bf16 · AdamW · single stage · 12 generations / 2112 steps · final `avr_loss` 0.0194.
Script: `train_run20.sh`. Trained 2026-08-07, 12:35 → 13:52 (76 min).

**Why g07 is the stream's default.** It is the generation the user identified as producing a good
south wolf, and the one that rendered a usable three-view set through the real pipeline at
`cn 0.5 / cn-end 0.9 / strength 0.7`. g08 is retained as the immediate alternate; swapping is a
one-line change, and [P0](todo.md)'s ruler exists partly so that choice can be made on a number.

**The caption form this LoRA expects** — load-bearing for [I1](issues.md#i1):

```
rd_<dir>, rd_<dir>, rd_<dir>, rd_style, rd_animal, rd_<bodyplan>, <direction phrases>,
single creature, full body
```

## Also on the box, not used by this stream

`rd_south_anima_r17_{g08,g10}` (south-only, 234 images) and `rd_east_anima_r18_{g08,g10}`
(east-only) from the same queue. Run-17 produced the most reliable south *pose* of any run — 6/6
front-facing, ~4/6 with the tail as a correct single upward spike — and is the natural fallback if
r20g07's south turns out to be seed-fragile. Run-18 was the control that showed 234 images does not
sit below the data floor.
