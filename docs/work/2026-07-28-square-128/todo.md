# Plan — SQUARE 128, lighting pinned at 64

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.** Every phase must hold these:

- **`FINE_RATIO` = 4**, before and after. It is `lightmap/shadow` texels and neither operand changes; if
  it moves, the split was done wrong.
- **corridor↔brute identity = 0 differing texels** (`__corridor(false)` + `__shadowDiff()`).
- **Zoom sweep 1 → 0.5 → 0.25 → 1 with no misregistration.** Every lod bug in this codebase has been a
  slot-address bug, and this stream changes a slot address.

**Fixture:** area1 at zoom 1 (`?user=Claude&focus=100,50&zoom=1`), 3 content torches. Perf uses the
calibrated orbit harness from
`2026-07-27-plane-intersection/completed.md` — cold draw
discriminated by RENDER TARGET, fixed frame count, phase from the frame index.

## P0 — Measure both costs separately, before changing anything

- [x] Record resident bytes per map family at `SQUARE = 64` from the live context, not from arithmetic. Acceptance: a table of art / lightmap / shadow / tile bytes whose total matches the browser's reported GPU memory within 10%.
- [x] Record the lighting-pass ms at `SQUARE = 64` over the orbit harness at reach 4/8/12. Acceptance: three numbers, each stable to ±0.01 ms across two runs.
- [x] Resize ONLY the lightmap RT to 4096×2048 (temporarily, `SQUARE` untouched) and re-measure. Acceptance: ms rises ~3–4×, confirming lightmap texels drive the lighting cost and art texels do not. Revert the resize.
- [x] Capture a zoom-1 crop of the wolf and one conifer as the before-image, and read back the wolf's atlas frame side in px. Acceptance: the frame side is recorded, so P3 can prove it doubled rather than assume it.

## P1 — Split the constant (no value change, must be a provable no-op)

- [x] Add `TEXTILE_LIGHT = 64` to `squareMath.ts` with a doc saying it is pinned and does NOT track `SQUARE`. Acceptance: exported, typechecks, no consumer yet.
- [x] Point the fine lightmap RT allocation at `TEXTILE_LIGHT` instead of `TEXTILE_SQUARE` (`shadowGather.ts:2039`). Acceptance: the RT is still 2048×1024 — the value is identical, only the source moved.
- [x] Point `FINE_RATIO` at `TEXTILE_LIGHT / SHADOW_TEXELS` (`shadowGather.ts:37`). Acceptance: `FINE` still prints 4 in the generated `LIGHT_FRAG` source.
- [x] Feed the lightmap's `uLSlot` from `TEXTILE_LIGHT >> lod` rather than `win.slotPx` (`Viewport.ts:456`), and rewrite the comment that asserts the two are the same. Acceptance: identical value at every lod while `SQUARE` is still 64.
- [x] Run the stream acceptance at `SQUARE = 64` with the split in place. Acceptance: 0 differing texels, clean zoom sweep, lighting ms within ±0.01 ms of the P0 baseline — a true no-op.

## P2 — Raise SQUARE to 128

- [x] Set `SQUARE = 128` and delete the A/B paragraph in its doc comment, replacing it with why the two dials are now separate. Acceptance: `UNIT` = 8, `TEXTILE_UNIT` = 16, `TEXTILE_LIGHT` = 64, asserted in a console readback.
- [x] Verify the shadow map is byte-for-byte the same size and the presence/bucket/dirty maps are untouched. Acceptance: shadow RT still 512×256, tile maps still 32×16.
- [x] Verify `REFERENCE_W/H` now read 3584×1536 and the cover fit still keeps the viewport inside the visible slots at both 1080p and 4K. Acceptance: no overscan visible at either resolution.
- [x] Run the stream acceptance at `SQUARE = 128`. Acceptance: 0 differing texels, clean zoom sweep, and the lighting ms still within noise of the P0 baseline — the split held under the raise.

## P3 — Give the art the resolution back

- [x] Audit whether the mastered sprites actually carry ≥128 px per tile, or whether the corpus was authored against the 64 cap. Acceptance: a count of defs whose master side is below their `size · 128`.
- [x] Re-master whatever P0/P3 found short, wolf first. Acceptance: 0 entries were short (14 of 27 carry 512), so nothing to re-master — recorded in [F6](forks.md#f6); art-pipeline work belongs to `2026-07-28-art-128-tiles`.
- [x] Re-publish the corpus and re-bake, then capture the same zoom-1 crop as P0. Acceptance: side-by-side against the before-image shows the wolf resolving detail the 64 build could not carry.
- [x] Re-measure resident bytes and confirm the art maps grew ~4× while the lightmap did not move. Acceptance: matches the README's table, or the table is corrected to the measurement.

## P4 — Reconcile the docs

- [x] Fix `VARIABLES.md` line 461, which says `1 unit = SQUARE/16 = 4px` while the same document says `SQUARE = 128` ([I1](issues.md)). Acceptance: 8 px, consistent with every other row.
- [x] Change the lightmap row (305–306) to name `TEXTILE_LIGHT` = 64 / 2048×1024, since it no longer shares `TEXTILE_SQUARE`'s value. Acceptance: the texel-family table lists four distinct families with their real sizes.
- [x] Re-read every `SQUARE`-derived figure in `VARIABLES.md` against the built code and correct whichever is wrong. Acceptance: `ppu` at lod 0 = 8, max frame side 2048, REFERENCE 3584×1536 all confirmed live.
- [x] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, `completed.md` records the measured before/after.
