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

## I3 — Position duplicated across `prim_data` + `light_data` under F1-(a) (2026-07-24)
**Problem.** If a light is a real prim (F1-a), its position lives in `prim_data.G` (the placed anchor,
the free-list + cascade key) AND in `light_data.G` (what the gather reads). A move would touch two
records.
**Candidate solutions.** (1) **Derive** `light_data.G` from the light-prim's `prim_data` each build (one
source of truth; the derive is a compare-write so a static light still emits no command). (2) Have the
gather read a light's position from its `prim_data` via a prim reference (retires `light_data.G`, adds an
indirection hop in the hot loop). **Lean: (1)** — keep the gather's direct `light_data` read (no hot-loop
indirection); the CPU derive is cheap and compare-written. Tied to [F1](forks.md#f1).

## I4 — Two kinds of "force-all" are conflated (2026-07-24)
**Problem.** `forceColdDirty`/`forceHotDirty` (`shadowGather.ts:925-926`) are set both by **placement-ish
events** (re-seed, `__manylights`, caster removal fallback `:1396`) and by **genuinely-global changes**
(`__tilt`/`__pitchnormal`/`__worldlight`/`__nsfactor`/`__elevk` — a shader constant changed, so every
tile must re-bake). P3 must not delete the latter while retiring the former.
**Solution.** Placement/removal → scoped `pendingRects` cascade (queue the last reach box before free).
Global constant changes → one explicit `rebakeAll()` kept for exactly that. The distinction is the point:
*placement is scoped; a global constant is not*. See [F4](forks.md#f4).
