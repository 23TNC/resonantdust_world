# Lighting + shader rework — build the new method — 2026-07-31

_Component: [`client/webgl`](../../components/client/webgl/). The user's design is
[`docs/intent/2026-07-31-rework.md`](../../intent/2026-07-31-rework.md) — **that file is the source of
truth for record layouts**; this stream implements it. Plan in [`todo.md`](todo.md); the adjustments I
made for performance and error handling are in [`forks.md`](forks.md)._

**Depends on** [`2026-07-31-lighting-strip`](../2026-07-31-lighting-strip/README.md) reaching the end
of its P2. The strip's P1 has landed (the renderer is unlit, 0.045 ms/frame, 1 draw); P2 deletes
`shadowGather.ts` and frees the old RTs. This stream builds into that cleared space.

## What the design is

Everything is a **prim**. A prim carries a position in units, a definition index, and three type
lanes — `cast_type`, `receive_type`, `emit_type` — that say whether it throws a shadow, catches one,
or emits light. There is no separate light table, no billboard/light/tile taxonomy, and no `set`
nibble: **one flat `u16` index space**.

| record | shape | holds |
|---|---|---|
| `prim_data` | 1 px per prim | position, definition, type lanes, colours, seed, rotation, intensity |
| `definition_data` | **16 px** per definition, indexed by rotation | atlas frame, subframe bbox, anchor, defaults |
| `light` | 1 px per **tile** | the 8 most important light prims reaching this tile (`u16` indices) |
| `presence` | 1 px per **tile** | the 8 receivers on this tile, **layer-sorted**, slot 0 = the tile itself |
| `shadow` | 3 px per **unit** | per light: the caster occluding the ground, plus 8 `(caster, receiver)` pairs |

Then a two-tier evaluation: a **per-unit** pass (256 fragments/tile) that finds *which caster* occludes
each light, and a **per-pixel** pass that uses that identity to test the caster's actual texture at the
pixel — so the shadow edge is silhouette-exact without any search.

## Why this is a rework and not a patch

Three structural failures killed the old system, and the design dissolves each **by construction**
rather than by fixing it:

| old failure | why it is gone |
|---|---|
| A caster reference was `u20` (`u4 set \| u16 id`), so 8 never fit a 128-bit texel — every workaround hung Chrome, doubled the walk, or carried no information ([strip I1](../2026-07-31-lighting-strip/issues.md#i1)) | one flat table ⇒ a reference is exactly `u16` ⇒ 8 fit a px with nothing left over |
| `max(coverage)` kept HOW MUCH and discarded WHICH, so the fine pass re-walked the whole corridor to rediscover the occluder — 9.29 ms of a 10.88 ms pass ([I2](../2026-07-31-lighting-strip/issues.md#i2)) | the stored value **is** the caster identity |
| hot/cold tiers existed because you cannot subtract one contributor from an accumulated map — forcing a class taxonomy, prev-buffers, and negative deltas ([I5](../2026-07-31-lighting-strip/issues.md#i5)) | one slot per light ⇒ every change is *(light slot) × (dirty region)* ⇒ **no classes at all** |

The third is the biggest: the hot/cold axis, the class-aware dirty machinery, `coldShadowPrevRT` /
`hotShadowPrevRT`, and the mover-shadow seam `hot-sync P4` was chasing all stop existing rather than
getting fixed.

## The adjustments I made

The user asked for performance and error handling. Full reasoning in [`forks.md`](forks.md); the
short list:

**Performance**

- **Split the write side from the read side** ([F1](forks.md#f1)). 8 per-light slots make removal
  exact; a **summed map** means the display pass does *one* fetch instead of eight. The delta update
  is one blended draw, and it is exactly the differential that was designed and never wired.
- **`RGB10_A2` at ¼ scale** ([F2](forks.md#f2)) — keeps today's 4× overbright, blends in core WebGL2,
  same 4 bytes as `RGBA8`, and drops the `EXT_float_blend` hard dependency on the write path.
- **One draw for all 8 lights** ([F3](forks.md#f3)), with the light index derived from the fragment's
  x. No MRT — MRT hung Chrome twice and was never root-caused.
- **The per-pixel refine is gated** to units that actually hold a caster ([F5](forks.md#f5)); its
  selectivity is a measured percentage, not an assumption.
- **Reach is derived from intensity** ([F6](forks.md#f6)) by one shared function, so the CPU building
  the per-tile light set and the GPU bounding the corridor walk cannot disagree.

**Error handling**

- **Validate on write, not on read** ([F7](forks.md#f7)). `definition_index + rotation` must never
  read a neighbouring definition; the CPU clamps and asserts, so the GPU hot loop pays nothing.
- **Sentinel discipline** ([F8](forks.md#f8)): index 0 is "nothing" in every index space, and every
  fetch guards.
- **Recycled indices are checked, not trusted** ([F9](forks.md#f9)) — a light slot verifies
  `emit_type != 0`, a caster verifies `cast_type != 0`, from a record already being read.
- **Overflow is counted, never silent** ([F10](forks.md#f10)). More than 8 lights or 8 receivers on a
  tile drops the excess *and increments a counter the debug panel shows*. The old system dropped in
  silence, which is how a light could vanish and look like a shader bug.

## The number this stream exists to move

**Moving lights at reach 16, zoom 1, inside 8 ms** — half a 60 fps frame. The old system reached
15–16. The floor to build up from is the stripped renderer: **0.045 ms/frame, 1 draw**.

## Known risk, stated up front

**The per-pixel pass is the one cost I cannot bound from the design.** It is per screen pixel × 8
lights with a receiver loop inside. The per-unit gather is not the worry — it runs at the same
resolution the old gather did, which measured 0.603 ms at one light / reach 16. P3 therefore builds
the lighting pass *before* shadows and measures cost-per-light on its own, so the expensive part is
priced before anything is built on top of it.
