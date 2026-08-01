# Plan — z positioning

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.**

- **Look at it on screen, at zoom, before claiming anything.** Three of the four defects in
  [lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) were mis-read from source and
  called fixed twice. Geometry is verified by looking.
- **ONE projection function**, shared by the draw path and the shadow path, in one file. A writer and
  a reader disagreeing on one rule has now cost this project four separate bugs.
- **A ground-level prim must render bit-identically** to today at every phase. Elevation is new
  behaviour for elevated things only; if it moves something standing on the floor, it is wrong.
- **Every perf claim from `__framecost()`**, which syncs with `readPixels` — `gl.finish()` does not
  sync here ([lighting-rework I10](../2026-07-31-lighting-rework/issues.md#i10)).

**Fixture:** `?user=Claude&focus=100,50&zoom=1` for cost, **`&zoom=3`** for geometry — a head-sized
offset is a handful of pixels at zoom 1 and invisible in a screenshot.

## P0a — Pin the two unknowns before writing any transform

- [ ] Settle whether `world_angle` is the diagrams' 55° or the code's `worldTiltDeg = 65` ([I7](issues.md#i7)). Acceptance: one value, its reference axis named, written into the shared function's doc comment — 55 is not 65's complement, so this is not a formatting difference.
- [ ] Settle the `world.z` coefficient ([I8](issues.md#i8)). Acceptance: the two terms are confirmed equal on purpose or corrected to a sin/cos pair — at 55° that is 0.819 vs 0.574, a 43% swing in the height term.

## P0 — Make height observable at all

- [ ] Give lights an authored height and write it to `unitZ` ([I1](issues.md#i1)). Acceptance: the shader's `Lz` is non-zero, read back from a probe — until it is, the caster height test is `0 <= H` and proves nothing.
- [ ] Show that the height test now discriminates. Acceptance: raising one light's height visibly shortens its casters' shadows on screen at zoom 3; a screenshot per height.
- [ ] Add an elevation probe: a prim's `unitZ`, its card's world extent, and where the projection puts it. Acceptance: one call answers "how high does the code think this is, and where does it draw it".

## P0b — The channel relayout ([I5](issues.md#i5))

- [ ] Move `seed` + `rotation` to GREEN and `definition_index` to BLUE; `u4 layer` becomes `u4 fine.z`. Acceptance: both words still total exactly 32 bits, and every shader lane read is updated with them.
- [ ] Retire `layer`/`seed`/`rotation` from `definition_data` to `u16 reserved`. Acceptance: nothing reads the retired lanes; `presence`'s sort still uses the caller's value, which is where it always came from.
- [ ] Update `VARIABLES.md` and the "BLUE and ALPHA are copied" line. Acceptance: the copy rule names the lanes that are actually copied now.
- [ ] Prove the relayout changed no pixel. Acceptance: with every height 0, the render and the shadow buffer are identical to before — this is a move, not a behaviour change.

## P1 — The caster card starts at its own height

- [ ] Make `occludes()` and `refineOccluded()` span `[z, z + H]` instead of `[0, H]` ([F3](forks.md#f3)). Acceptance: a caster at z casts a shadow displaced by the geometry, not one starting at the ground.
- [ ] Prove a ground prim is unchanged. Acceptance: with every `unitZ` at 0 the shadow buffer is bit-identical to before the change — elevation must cost the floor case nothing.
- [ ] Re-check the walk against the exhaustive reference with mixed heights. Acceptance: 0 differing occlusion decisions, the same check P4 used, with casters at several heights.

## P2 — The screen↔world transform ([F6](forks.md#f6), [I8](issues.md#i8))

_There is no draw-side constant: the screen-north shift IS the elevation, 1:1. The tilt lives in
`unit.z`'s decomposition and in the shadow's transform, and nowhere else._

- [ ] Write the decomposition + its inverse in TS **and** GLSL in one file, off the live tilt. Acceptance: `elevation → unit.z` and `unit.z → elevation, ground_y` round-trip across the `u8` range, CPU and GPU agreeing, like `reachFromIntensity`.
- [ ] Give `occludes()` the screen→world transform it has never had ([I8](issues.md#i8)). Acceptance: shadow length matches the geometry for a known caster height under a known light height — measured, not tuned to look right.
- [ ] Re-check the ground case. Acceptance: with every elevation 0 the shadow buffer is bit-identical to before — the transform must be a no-op at the floor.

## P3 — The head rides elevation

- [ ] Add `offset.z` to the pawn DSL and retire `head.offset.y` ([F5](forks.md#f5)). Acceptance: the corpus authors a height; no y-fudge remains on the head slot.
- [ ] Carry it through `MoverPart` and `content.moverParts` as `offsetZ`. Acceptance: the loader reads it, and a missing value defaults to 0 rather than to the old y behaviour.
- [ ] Apply the projection in `MoverLayer` and write `unitZ` into the record. Acceptance: the head draws where it does today (A/B at zoom 3) AND its record carries a non-zero height.
- [ ] Show the two shadows became one. Acceptance: a zoom-3 capture of body+head casting a single aligned shadow, beside today's two offset ones — the reason the stream exists.

## P4 — Where a thing stands, for lighting as well as shadow

- [ ] Sample the lightmap at the prim's BASE ROW, not its drawn position ([lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4). Acceptance: a billboard is lit by the tile it stands on; `zdepth.B` already carries the base row.
- [ ] Settle the z-order tie now that head and body share a base row ([F4](forks.md#f4)). Acceptance: the parts order deterministically, and the rule is written down rather than resting on draw order.
- [ ] Rename the draw-order `z` away from the collision ([F1](forks.md#f1)). Acceptance: `slotZ`/`zIndex`/`depth` no longer read as height; one word means one thing.

## P5 — The verdict

- [ ] Re-measure the frame with elevation live. Acceptance: no measurable change against the pre-stream baseline — this adds arithmetic to existing passes, not passes.
- [ ] Capture the A/B set at zoom 3. Acceptance: ground prim unchanged, head aligned, one shadow; each beside its before-image.
- [ ] Record what elevation now buys for free. Acceptance: a sentence each on wall torches, carried items and any elevated caster, so the generalisation is not rediscovered.
- [ ] Update `VARIABLES.md` on `unit.z`'s meaning and units. Acceptance: it says height in units, names the projection, and distinguishes it from the draw-order lane.
