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

- [ ] Reintroduce the world tilt as a live parameter at **65°** ([I7](issues.md#i7)) — it does not exist; the strip deleted it with `shadowGather.ts`. Acceptance: one source, read by BOTH the record writer and the shadow transform, with the reference axis named.
- [x] Pin the game→world transform's coefficients ([I8](issues.md#i8)). Acceptance: `unit.z`'s split into `world.y` and `world.z` is stated once and derived from the live tilt — the solve is metric, so a wrong coefficient biases falloff and `N·L`, not just geometry.

## P0 — Make height observable at all

- [x] RE-READ the shadow path before planning against it ([I1](issues.md#i1)). Acceptance: the current `occludesAt` is quoted in `completed.md` — heights ARE populated and interpolated, and two diagnoses were already given from stale memory.
- [x] Confirm the height test discriminates TODAY. Acceptance: raising one light's height visibly shortens its casters' shadows at zoom 3 — if it does, the gap is the metric ([I9](issues.md#i9)), not the machinery.
- [x] Add an elevation probe: a prim's `unitZ`, its card's world extent, and where the projection puts it. Acceptance: one call answers "how high does the code think this is, and where does it draw it".

## P0b — The channel relayout ([I5](issues.md#i5))

- [x] Move `seed` + `rotation` to GREEN and `definition_index` to BLUE; `u4 layer` becomes `u4 fine.z`. Acceptance: both words still total exactly 32 bits, and every shader lane read is updated with them.
- [ ] Settle `definition_data`'s retired lanes ([I16](issues.md#i16) — the original item's acceptance is false: `silhouetteHit` reads the def's `seed` as `pxPerUnit × 8`). Acceptance: a decision recorded — the recommendation is to close it as unnecessary, since the definition record has no bit pressure to relieve.
- [x] Update `VARIABLES.md` and the "BLUE and ALPHA are copied" line. Acceptance: the copy rule names the lanes that are actually copied now.
- [x] Prove the relayout changed no pixel. Acceptance: with every height 0, the render and the shadow buffer are identical to before — this is a move, not a behaviour change.

## P1 — The caster card starts at its own height

- [x] Make `occludes()` and `refineOccluded()` span `[z, z + H]` instead of `[0, H]` ([F3](forks.md#f3)). Acceptance: a caster at z casts a shadow displaced by the geometry, not one starting at the ground.
- [x] Prove a ground prim is unchanged. Acceptance: with every `unitZ` at 0 the shadow buffer is bit-identical to before the change — elevation must cost the floor case nothing.
- [x] Re-check the walk against the exhaustive reference with mixed heights. Acceptance: 0 differing occlusion decisions, the same check P4 used, with casters at several heights.

## P2 — The screen↔world transform ([F6](forks.md#f6), [I8](issues.md#i8))

_There is no draw-side constant: the screen-north shift IS the elevation, 1:1. The tilt lives in
`unit.z`'s decomposition and in the shadow's transform, and nowhere else._

- [ ] Write the decomposition + its inverse in TS **and** GLSL in one file, off the live tilt. Acceptance: `elevation → unit.z` and `unit.z → elevation, ground_y` round-trip across the `u8` range, CPU and GPU agreeing, like `reachFromIntensity`.
- [ ] Give `occludes()` the screen→world transform it has never had ([I8](issues.md#i8)). Acceptance: shadow length matches the geometry for a known caster height under a known light height — measured, not tuned to look right.
- [ ] Re-check the ground case. Acceptance: with every elevation 0 the shadow buffer is bit-identical to before — the transform must be a no-op at the floor.

## P2b — Separate the coordinate systems ([F7](forks.md#f7))

- [ ] Make `RecordSync` write GAME coordinates — the prim's ground tile plus elevation — instead of the drawn position. Acceptance: a head and its body write the SAME `unit.x/y`, differing only in `unit.z`.
- [ ] Convert `P` screen→WORLD once per texel and solve in world ([F8](forks.md#f8)). Acceptance: no caster read converts per-test; falloff `d` and `ldir` are computed on world quantities, since both are metric and neither survives an affine change of space.
- [ ] Fix `targetH` to include the receiver's OWN elevation ([F8](forks.md#f8)). Acceptance: `E + (baseY - P.y)`, height above the ground; an elevated receiver is shadow-tested at its true height, and a floor receiver is bit-identical.
- [ ] Prove presence keys on game coordinates. Acceptance: head and body appear in the SAME presence tile, read back from the mirror.

## P3 — The head rides elevation

- [ ] Add `offset.z` to the pawn DSL and retire `head.offset.y` ([F5](forks.md#f5)). Acceptance: the corpus authors a height; no y-fudge remains on the head slot.
- [ ] Carry it through `MoverPart` and `content.moverParts` as `offsetZ`. Acceptance: the loader reads it, and a missing value defaults to 0 rather than to the old y behaviour.
- [ ] Apply the projection in `MoverLayer` and write `unitZ` into the record. Acceptance: the head draws where it does today (A/B at zoom 3) AND its record carries a non-zero height.
- [ ] Show the two shadows became one. Acceptance: a zoom-3 capture of body+head casting a single aligned shadow, beside today's two offset ones — the reason the stream exists.

## P4 — Where a thing stands, for lighting as well as shadow

- [ ] Sample the lightmap at the prim's BASE ROW, not its drawn position ([lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4). Acceptance: a billboard is lit by the tile it stands on; `zdepth.B` already carries the base row.
- [ ] Order by `screen.z` against a COMMON reference ([F4](forks.md#f4)). Acceptance: head sorts before body because it is nearer the viewer, not because of graph walk order; the reference plane is scene-wide, so depths from different prims compare.
- [ ] Rename the draw-order `z` away from the collision ([F1](forks.md#f1)). Acceptance: `slotZ`/`zIndex`/`depth` no longer read as height; one word means one thing.

## P5 — The verdict

- [ ] Re-measure the frame with elevation live. Acceptance: no measurable change against the pre-stream baseline — this adds arithmetic to existing passes, not passes.
- [ ] Capture the A/B set at zoom 3. Acceptance: ground prim unchanged, head aligned, one shadow; each beside its before-image.
- [ ] Record what elevation now buys for free. Acceptance: a sentence each on wall torches, carried items and any elevated caster, so the generalisation is not rediscovered.
- [ ] Update `VARIABLES.md` on `unit.z`'s meaning and units. Acceptance: it says height in units, names the projection, and distinguishes it from the draw-order lane.
