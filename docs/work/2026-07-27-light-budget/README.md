# A prioritised light-update budget — 2026-07-27

_Components: [`client/webgl`](../../components/client/) (`game/viewport/shadowGather.ts`,
`Viewport.ts`). Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md)._

## The decision (user, 2026-07-27)

> _"the budget for light updates becomes 512 until we improve performance […] Please use priority, and
> try to give priority to hot lights without starving cold lights. lights closer to the anchor take
> priority over those further away. Please build in a method for prims to mark that a light needs to be
> of higher priority so we have tools available."_

## Why — the shadow gather is the one unbudgeted path

The G-buffer bake is already capped. `Viewport.tick` spends a fixed allowance per frame and leaves the
rest queued:

    this.warm.bakeDirty(BAKE_BUDGET);                                              // 128 squares
    this.map.bakeDirty(Math.max(BAKE_BUDGET - this.warm.lastBaked, COLD_BAKE_FLOOR));  // >= 64

**The lighting path has no equivalent.** `buildDirty` marks every dirty slot, uploads it as a texture,
and the gather runs as ONE fullscreen draw where clean tiles `discard`. However many tiles are dirty,
they are all recomputed in that single draw — no cap, no carry-over.

Measured on the analytic-interval build (one orbiting light, zoom 1, 240 frames):

| reach | dirty tiles / frame | ms | ms per tile-light pair |
|---|---|---|---|
| 4 | 110 | 0.170 | 0.00155 |
| 8 | 263 | 0.394 | 0.00150 |
| 12 | 404 | 0.692 | 0.00171 |

**Cost is flat per tile-light pair** (~0.0015 ms) and the tile count scales as reach². At zoom 1 the
window is 32 × 16 = **512 slots**, so a single reach-12 light in motion already dirties **~79 % of the
visible world every frame**.

And a tile carries up to **16** lights (`PRES_SLOTS = TILE_SLOTS * 2`; all three shader loops run
`slot < 16`), so the ceiling is 404 × 16 ≈ 6 500 pairs ≈ **11 ms in one draw**.

## This is a stability fix first, a throughput fix second

An unbounded single draw is what has been **killing the GPU** repeatedly: the pre-plane-intersection
build cannot even load — its cold start dirties all 512 slots at the old ~0.0045 ms/pair and the draw
runs long enough to trip the watchdog, losing the context before a frame is presented
([plane-intersection I6](../2026-07-27-plane-intersection/issues.md)). A budget converts *"context lost,
renderer dead"* into *"lighting lags for two frames"*. That is worth doing even if throughput never
improves.

## Shape

**Budget in tile-LIGHT PAIRS, not tiles** ([F1](forks.md#f1)) — "512 tiles" is anywhere from 0.9 ms to
14 ms depending on how many lights cover them, which is a range rather than a budget. Pairs are the unit
that maps to constant cost. Admission is still decided per tile (the dirty texture is per tile); the
budget is simply *spent* in pairs as tiles are admitted.

**Priority is per light, applied per tile as a max.** `markLightDirty` already queues a rect per light,
so each rect carries a priority and an overlapped tile takes the highest. Inputs, in the order the user
gave them:

1. **hot** lights outrank cold,
2. **distance to the camera anchor** — nearer first,
3. an explicit **boost a prim can set**, so future gameplay code has the lever.

**Cold lights must not starve**, which pure hot-first would guarantee. Aging is the primary mechanism
([F2](forks.md#f2)): a deferred tile's priority rises with each frame it waits, so service is bounded
rather than merely likely.

**Deferring lighting is not like deferring terrain.** Shadow-cold is *accumulated*, so a skipped tile
shows the PREVIOUS frame's lighting — visibly wrong, not merely low-detail. Drain order and a measurable
staleness bound therefore matter more here than they do for the G-buffer.

## What this does NOT do

It rations the dirty set; it does not shrink it. The larger win is that most of a moving light's 404
dirty tiles change by less than the 8-bit lightmap can represent — far from the light the falloff is
nearly flat and the delta rounds to zero. Thresholding that would collapse the set rather than queue it.
Recorded as [F4](forks.md#f4) and deliberately out of scope: a budget must exist first, because it is
what stops the crash.
