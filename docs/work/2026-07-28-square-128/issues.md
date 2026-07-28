# Issues — SQUARE 128

## I1 — `VARIABLES.md` disagrees with the code, and with itself

The A/B commit `2b1025a` ("SQUARE 128 -> 64") changed `squareMath.ts` and left the authoritative
document alone. `VARIABLES.md` still specifies the 128 world throughout:

| line | claim | true at `SQUARE = 64`? |
|---|---|---|
| 279 | `SQUARE = 128`; maximum art size is 128 px | no — 64 |
| 305 | `TEXTILE_SQUARE` = 128, texture 4352 × 2304 | no — 2176 × 1152 |
| 306 | lightmap 128, 4096 × 2048 | no — 2048 × 1024 |
| 315 | lod 0 → 128 px | no — 64 |
| 601 | `ppu = SQUARE/(16·2^lod)` → lod 0: 8 | no — 4 |
| 618 | max frame side = 16 tiles × 128 = 2048 | no — 1024 |

**And line 461 contradicts all of them**: "a compile-time constant `1 unit = SQUARE/16 = 4px`". 4 px is
`SQUARE = 64`. So the document describes two different worlds in two places, and neither matches the
built client uniformly.

**Why this matters beyond tidiness.** `VARIABLES.md` outranks code by convention — a reader resolving a
constant is supposed to trust it. Anyone who did so while `SQUARE` was 64 would have computed atlas
frame sides, `ppu`, and texture footprints all 2× wrong, silently. `ppu` in particular is live shader
input (`sampleCard`), so a hand-check against the doc would have "found" a bug that was not there.

**Handled by this stream**, in both directions: [P2](todo.md) makes the code match the document's 128,
and [P4](todo.md) fixes line 461 and re-points the lightmap row at the new `TEXTILE_LIGHT` — the one row
where the document, not the code, is the thing that must change.

**Root cause worth naming:** the A/B was landed as a `test(viewport):` commit — an experiment — but was
never reverted or ratified, so a temporary probe became the shipped value with no doc update. The
convention that would have caught it is that a constant ratified in `VARIABLES.md` cannot be changed by
a `test(` commit without either restoring it or amending the document.

## I2 — The A/B's headline figures bundle two independent effects

`2b1025a` reports "lighting 3.3× cheaper, 192 MiB freed, art softer" as one result. Those are three
different mechanisms behind one constant:

- **3.3× lighting** — driven by lightmap texels only. `TEXTILE_LIGHT` keeps this.
- **~48 MiB** — the lightmap RT (RGBA16F 4096×2048 → 2048×1024). Kept.
- **~144 MiB** — the four art maps at 4352×2304 → 2176×1152. **This is what the stream gives back**, and
  it is the actual price of sharper art.
- **"art softer"** — the cost, now paid back.

Nobody was wrong to bundle them at the time; the constant genuinely did move all four together. But the
bundled number is why "revert the A/B" reads as giving back a 3.3× win when it does not. The split is
only possible because the effects were never actually coupled — just co-located behind one name.

**Unverified until [P0](todo.md).** The MiB figures above are derived from texel counts and format
sizes, not measured. P0 measures them, and [P3](todo.md) requires the README's table to be corrected to
the measurement if it disagrees.
