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
