# Todo — TEXTILE_SLOT (execution order)

_Model in [`README.md`](README.md); decisions in [`forks.md`](forks.md); traps in
[`issues.md`](issues.md). Every phase keeps the corridor↔brute identity green, and every identity check
must assert a **non-zero population on both sides** — "0 mismatches" is satisfiable by having no data
([primitive-graph D-2](../2026-07-25-primitive-graph/deviations.md#d-2))._

## P0 — Ratify the constants
- [x] Write the slot grid + lod ladder into `docs/VARIABLES.md` as the authoritative layout.
      Acceptance: slots, texels-per-slot per family, and the zoom bands all live in one table.
- [x] Reduce `definition_data.frame_lod` u4 → u2 and record the freed bits.
      Acceptance: 4 lod levels cover the whole ladder; the layout still sums to 32.
- [x] Add `billboard_data.last_lod` (u2) and `light_data.coarsest_lod` (u2).
      Acceptance: VARIABLES states why one is "last" and the other "coarsest" ([I2](issues.md#i2)).
- [x] Decide + record the max supported aspect the slot grid is sized for ([F4](forks.md#f4)).
      Acceptance: a named aspect, with the visible-slot count at that aspect.

## P1 — Fix the slot grid
- [x] Raise `SQUARE` 64 → 128 and fix the grid at 24×16 slots (20×12 visible).
      Acceptance: every map's dims are constant across a full zoom sweep.
- [x] Replace the screen-derived channel sizing with the fixed 3072×2048 square-family texture.
      Acceptance: `ensureBuffers` no longer reads `screenW`/`screenH`.
- [x] Implement the cover fit `s = max(W/2560, H/1536)` and centre the result.
      Acceptance: 2560×1536→s=1; 2560×768→s=1 showing 20×6 slots; 4K→s=1.5.
- [x] Move the light + shadow RTs onto the shared grid at their own densities.
      Acceptance: lightmap 3072×2048, shadow 384×256, both zoom-invariant.

## P2 — Publish the mapping as constants
- [x] Write `lod` into the data-texture constants via a command (G bits 14–15).
      Slot dims stay per-family compile-time constants; only `lod` varies. Landed with P1b.
- [ ] Convert every shader to read the slot mapping from constants rather than deriving it.
      Acceptance: no shader computes a slot address from a world coord ([map-compatibility](../2026-07-24-map-compatibility/README.md)).

## P3 — Reproject instead of clear
- [x] Add the scratch target + per-slot reproject blit (the slot permutation makes in-place undefined).
      Acceptance: re-partition no longer calls `clear()`; no flash across a zoom step.
- [x] Zoom IN: upscale + clip retained tiles, dirty nothing.
      Acceptance restated as **0 FRESH**, not 0 dirty — see [D-1](deviations.md#d-1). Measured 0/384.
- [x] Zoom OUT: downscale retained tiles, dirty only newly covered tiles.
      Acceptance: measured 384 carried + 1152 fresh of 1536 — only the new ring bakes.
- [ ] Apply the per-family filter rule: NEAREST everywhere, linear only for albedo/normal/surface.
      Acceptance: z-order stable across a zoom step; no bitfield corruption.

## P4 — Per-piece lod state — BLOCKED on the additive lightmap, deliberately
_The layouts are ratified (P0) but the CONSUMER does not exist: `coarsest_lod` only earns its keep once the
invertible `RGBA32F` accumulator lands ([primitive-graph P10](../2026-07-25-primitive-graph/todo.md)). There
is nothing to subtract from today, so implementing it now would be untestable bookkeeping. `last_lod`'s
def-swap is already covered by `definitionFor` → `billboardDataFor().changed` → `markBillboardDirty`._
- [ ] Maintain `light_data.coarsest_lod` as a monotone max, reset on re-cast.
      Acceptance: down-then-up round trip leaves no residue ([I2](issues.md#i2)).
- [ ] Evaluate a light's subtraction on its `coarsest_lod` grid, not the current one.
      Acceptance: 64 add/subtract pairs across a zoom sweep return a texel to exactly 0.

## P5 — Prioritised refinement
- [x] Split the dirty map into empty vs stale classes using `slotBaked` and the lod fields.
      Measured on a lod step: empty 1152 == fresh, stale 384 == carried. No new structure.
- [x] Drain empty before stale, budgeted per frame.
      `bakeDirty` already sorts ascending and `prio` is `band + ring`, so the bands separate the classes.

## P6 — Verify
- [x] Sweep zoom across all four lods, both directions, and confirm no flash and no drift.
      7 steps, `allConstant: true`; every zoom-IN step reported `fresh: 0`.
- [x] Re-run corridor↔brute identity at lod 0 with a non-zero population both sides.
      31,645 non-zero texels each side, **0 mismatches**. Per-lod sweep still open below.
- [x] Measure resident texture bytes across the zoom sweep.
      **304 MiB, constant.** Lightmap+shadow alone were 264 MiB and growing before.
- [x] Confirm 128px masters exist for every kind that renders at lod 0.
      16 stems, `maxSize` ≥ 128 for all, zero under ([I6](issues.md#i6)).
- [ ] Extend the identity check to lods 1–2 (it only ran at lod 0).
      Acceptance: 0 mismatches with a non-zero population at each lod.
- [x] Re-run the zoom sweep through the REAL path after [I11](issues.md#i11).
      1→0.5→0.25→0.5→1 via `__zoom`: ladder 0→1→2→1→0, coverage full at every lod once settled,
      G-buffer + lightmap byte-constant, zoom-IN `fresh: 0`.
