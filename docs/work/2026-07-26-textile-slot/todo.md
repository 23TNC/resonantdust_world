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
- [ ] Write `lod`, `slot_width`, `slot_height` into the data-texture constants via a command.
      Acceptance: a lod change is one command write, like a light update.
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

## P4 — Per-piece lod state
- [ ] Maintain `billboard_data.last_lod` and swap the def when it differs from current.
      Acceptance: a zoom settles to correct-lod art with no blanket re-bake.
- [ ] Maintain `light_data.coarsest_lod` as a monotone max, reset on re-cast.
      Acceptance: down-then-up round trip leaves no residue ([I2](issues.md#i2)).
- [ ] Evaluate a light's subtraction on its `coarsest_lod` grid, not the current one.
      Acceptance: 64 add/subtract pairs across a zoom sweep return a texel to exactly 0.

## P5 — Prioritised refinement
- [ ] Split the dirty map into empty vs stale classes using `slotBaked` and the lod fields.
      Acceptance: `dirty`'s existing number payload carries the class; no new structure.
- [ ] Drain empty before stale, budgeted per frame.
      Acceptance: no visibly empty tile during a zoom+pan; stale tiles refine after.

## P6 — Verify
- [ ] Sweep zoom across all four lods, both directions, and confirm no flash and no drift.
      Acceptance: screenshot-free — assert dirty counts and map dims per step.
- [ ] Re-run corridor↔brute identity at each lod.
      Acceptance: 0 mismatches with a non-zero population on both sides.
- [ ] Measure resident texture bytes across the zoom sweep.
      Acceptance: constant, and ≈291 MiB total.
- [ ] Confirm 128px masters exist for every kind that renders at lod 0.
      Acceptance: a list of kinds lacking one, or none.
