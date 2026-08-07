# Issues — style, not bestiary

_Problems hit, and what the evidence actually showed._

## I1 — 176 of 200 species are one-shot per direction {#i1}
_2026-08-06 · measured locally · **the finding this stream exists for**_

```
200 species · 701 images · mean 3.5 each
   2 images:   1 species
   3 images: 176 species   <- one per direction
   6 images:  17 species
   9 images:   4 species
  12 images:   1 species
  21 images:   1 species
```

Body-plan tags: `rd_quadruped` 498 · `rd_biped` 90 · `rd_winged` 69 · `rd_humanoid` 51 ·
`rd_multiped` 27 · `rd_legless` 27.

So the corpus teaches **one convention with 701 examples** and **200 species with ~1 example each
per direction**. The predecessor stream trained ten runs against this without measuring it, and
every unexplained result falls out of it:

- cat (3 images) and bear (12) never moved across ten runs at any LR, alpha, corpus or warm start
- wolf-east was the best cell in every run — the whole canine group teaches one shared body plan
- the convention itself was learned reliably and early, because it is the only thing with 701 examples

**This is a data ceiling, not a training ceiling.** It is stated here so no future run is spent
trying to reach a 3-image concept by tuning.

## I2 — 229 of 252 caption tokens are backed by ≤3 images {#i2}
_2026-08-06 · measured at P0 · **sharper than [I1](#i1), and the direct case for [F1](forks.md#f1)**_

P0 confirmed [I1](#i1) exactly and then found the more pointed figure. Across 701 captions:

| | |
|---|---|
| distinct non-`rd_` tokens | **252** |
| of those, appearing on **≤3 images** | **229 (91%)** |
| tokens appearing on 233–234 images | the direction/composition phrases only |

So the caption vocabulary splits cleanly in two. A dozen phrases — `side profile`, `facing right`,
`front view`, `back view`, `single creature`, `full body` — carry 233–234 examples each. Everything
else (**229 tokens**) is a species or family name with three examples or fewer.

**The corpus is also perfectly balanced by direction** — east 234, north 233, south 234 — and of
the 176 species with exactly three images, **176 of 176 have exactly one per direction**. Nothing
is skewed; it is simply thin.

**A caption today, verbatim:**

```
rd_style, rd_animal, rd_quadruped, rd_east, side profile, side view, facing right,
feline, tiger, single creature, full body
```

Nine of those eleven tokens have 233+ examples behind them. Two — `feline`, `tiger` — have three.
[F1](forks.md#f1) drops exactly those two classes and keeps the rest.

## I3 — The style-only captions collapse to 27 distinct strings, and 165 images share one {#i3}
_2026-08-06 · measured at P1 · **recorded as a PREDICTION before training, so it can be wrong**_

Dropping species and family tokens leaves **27 distinct captions** across 701 images — nine
body-plan combinations × three directions. The largest class is **165 images sharing one caption**:
every quadruped east view, from cat to giraffe to elephant.

**The [I12](../2026-08-02-lora-beat-e07/issues.md#i12) defect is NOT reintroduced.** That failure
was `rd_quadruped` on 459/459 — a constant, discriminating nothing. Here every body-plan tag has
negatives: quadruped 498/701, biped 90, winged 69, humanoid 51, legless 27, multiped 27. Verified
tag by tag; all six discriminate.

**But there is a real new hazard, and it is the mirror of [F1](forks.md#f1)'s stated risk.** With
165 very different shapes under one label, the model may learn a **blurry average** of "quadruped
east" rather than a crisp convention. Style LoRAs are routinely trained this way — one trigger
across a varied set, where the variety is what prevents memorising any single subject — so this is
a known-viable regime, not an obvious mistake. It is still the thing most likely to go wrong.

**What would confirm it:** outputs that are mushy or non-committal in silhouette, especially where
the corpus's quadrupeds differ most (a giraffe and a cat sharing one caption). **What would refute
it:** crisp masses whose species identity tracks the PROMPT rather than the corpus — which is
exactly the thesis.

[P3](todo.md)'s cat-versus-well-covered-species comparison tests both directions.
