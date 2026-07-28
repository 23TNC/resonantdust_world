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

## I3 — P0 measurement corrects the stream's own arithmetic, in both directions

The README's cost table was derived from texel counts before anything was measured. Both halves were
wrong, and interestingly not in the same direction.

**`SLOTS × TEXTILE_SQUARE` sizes three maps, not one.** The A/B commit, the README and the plan all say
"the lightmap". `fw/fh` actually allocates `coldLightRT` (rgba32float, 32 MiB), `hotLightRT` (32 MiB)
**and** `receiverFineRT` (r32uint, 8 MiB) — 72 MiB at 64/tile, 288 MiB at 128/tile. Pinning
`TEXTILE_LIGHT` therefore saves **216 MiB**, not ~48 MiB. The fine maps, not the art maps, were the
dominant term in the A/B's "192 MiB freed".

**The art family is 8 channels, not 4.** `VARIABLES.md` lists albedo/normal/surface/zdepth for
`TEXTILE_SQUARE`; there is a complete **warm** duplicate of all four for movers (`__viewport.warm`), so
the real footprint is 8 × 9.56 MiB. Raising `SQUARE` costs **+245 MiB**, not ~120 MiB.

| | README claimed | measured / projected |
|---|---|---|
| saved by pinning the light dial | ~48 MiB | **216 MiB** |
| paid by raising the art dial | ~120 MiB | **245 MiB** |
| total at `SQUARE = 128`, pinned | not stated | **~420 MiB** |
| total at `SQUARE = 128`, joined | not stated | **~636 MiB** |

**This strengthens the stream's thesis and raises its price at the same time.** The split is worth 4.5×
what was claimed, which is the whole argument for doing it — but ~420 MiB of resident maps is a real
number and the honest one to hold P3 against. `VARIABLES.md` under-documents the art family by 2×
([I1](issues.md) already covers the doc reckoning; this adds the warm row to P4's work).

**A third consequence, unplanned:** `receiverFineRT` rides `TEXTILE_SQUARE` but is consumed by the
*lighting* pass, not the display. It is receiver geometry sampled per lighting texel, so it belongs on
`TEXTILE_LIGHT` with the lightmap — P1 must move all three, and the plan's wording ("the fine lightmap
RT") names only one. Treated as a plan-wording defect, not a design change: moving it is required for the
split to be a no-op.
