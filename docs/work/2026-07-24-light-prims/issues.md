# Light prims — issues

_Problems + candidate solutions. Chronological append._

## I1 — The manual light injection doesn't propagate (the motivating failure) (2026-07-24)
**Problem.** To run a 50-light perf test I pushed 50 lights straight into `g.lights` and set
`g.coldDirty = true`. Result: `coldData.lightCount` stayed **3** (the seed lights) — the injection never
reached the bake. Two causes: (1) the render loop is **change-gated**, so `ShadowGather.tick()`
(`shadowGather.ts:1349`) doesn't re-run just because a JS array mutated — `buildLights` (`:1388`) only
fires inside `tick`, on a real frame; (2) even when it fires, `this.lights` is a **bespoke debug array**
with no home in the world, so there is no placement event to trigger a frame or dirty the right tiles.
**Solution.** This whole stream — a light becomes a placed prim, delivered through the per-frame prim
list, allocated + dirtied by the prim pipeline. Placement itself drives the frame + the dirty.

## I2 — Per-tile presence cap: nearest ≤14 lights/tile (2026-07-24)
**Problem.** `light_presence_lo/hi` hold ≤14 light slots per tile (`VARIABLES.md` sets 3+5; the gather
loops `slot < 14`, `GATHER_FRAG`). With many overlapping lights (the 50-light test: 12-tile reach,
heavy overlap) a tile keeps only its **nearest 14** reaching lights; the rest contribute nothing there.
**Analysis.** This is **by design** — it bounds gather cost to O(14) lights/texel (the many-lights
affordability lever). It is not a bug, but authors placing dense lights must know coverage is
nearest-14/tile, and a light that "does nothing" in a dense cluster may simply have lost the slot race.
**Status.** Document, don't change here. If dense clusters need more, that's a separate widen-the-cap
stream (costs bits + gather time), not this one.

## I3 — Position duplicated for a primitive that presents as BOTH (2026-07-24)
**Problem.** A **pure light** has no duplication — its position lives only in `light_data.G` ([F1](forks.md#f1)).
But a primitive presenting as **both** (an emissive sprite: torch + glow) has its position in
`billboard_data.G` (the caster anchor) AND `light_data.G` (the emitter). A move must update both.
**Candidate solutions.** (1) **Write both from the one placement** — each presentation record is
authoritative for its own read path; a compare-write means a static primitive still emits no command
(lean, per [F1](forks.md#f1)). (2) Single source in `billboard_data`, **derive** `light_data.G` each build.
Both are cheap CPU-side; (1) avoids any read-time indirection. **Lean: (1).**

## I4 — Two kinds of "force-all" are conflated (2026-07-24)
**Problem.** `forceColdDirty`/`forceHotDirty` (`shadowGather.ts:925-926`) are set both by **placement-ish
events** (re-seed, `__manylights`, caster removal fallback `:1396`) and by **genuinely-global changes**
(`__tilt`/`__pitchnormal`/`__worldlight`/`__nsfactor`/`__elevk` — a shader constant changed, so every
tile must re-bake). P3 must not delete the latter while retiring the former.
**Solution.** Placement/removal → scoped `pendingRects` cascade (queue the last reach box before free).
Global constant changes → one explicit `rebakeAll()` kept for exactly that. The distinction is the point:
*placement is scoped; a global constant is not*. See [F4](forks.md#f4).
