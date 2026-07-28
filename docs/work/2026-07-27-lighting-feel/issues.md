# Issues — lighting feel

_Problems hit, findings, candidate solutions._

## I1 — P0 AO audit: surface-G is EXACTLY flat (2026-07-27) {#i1}

Sampled every kind's disk leaves (`textures/**/surface.*.png`, 42 files) with PIL:

| kind | leaves | G distinct values | G value |
|---|---|---|---|
| biome-thing/default/conifer | 9 | 1 | 255 everywhere |
| biome-thing/default/flora | 13 | 1 | 255 everywhere |
| pawn/animal/wolf | 20 | 1 | 255 everywhere |

The user's "fairly limited" was generous — there is NO AO information in the corpus. Channel
reality of the disk leaf: R = 0 flat, G = 255 flat, B = the AA'd silhouette (256 distinct — the
real payload), A = 255 flat. Consequence: P3's conditional blit item reroutes to the `bin/art`
bake item as planned; consuming G before that bake would multiply by 1.0 (a no-op shipped as a
feature).

## I2 — P0 emissive audit: 12 leaves exist, all wolf (2026-07-27) {#i2}

`textures/pawn/animal/wolf/{1,124,125,555,777,888}/emissive.{n,s}.0.png` — real content (RGBA,
~237 distinct values; glowing-eye variants). Zero emissive for biome things (no torch flame).
Consequence for P3: the `bin/art` emissive PATH already exists (leaf naming, per-variant); the
work is authoring/generating the flame leaf + resolver/co-pack carriage ([F3](forks.md#f3)) +
blit consumption — not pipeline-from-scratch.

## I3 — the torch has no flame art {#i3}

The `torch`/`torch_blue` kinds render the `"white` placeholder square (tinted). Emissive "flame
first" therefore has nothing to mask — the carriage was proven on flora (test) and ships on the
wolf's eyes. Follow-up: author/generate a flame sprite for the torch kinds (bin/art generate or
hand art), then `art emissive` + `art surface` — the lane picks it up with zero code.

## I4 — `art clean <kind> emissive` removed 0 files {#i4}

The clean pattern misses the current per-variant leaf layout (`<variant>/emissive.<dir>.0.png`).
Worked around with `find -name 'emissive*' -delete`. Papercut; fix the glob in `cmd_clean` when
next in `bin/art`.
