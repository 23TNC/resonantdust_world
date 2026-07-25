# Light prims — make lights first-class world objects (placed + auto-dirtied like things) — 2026-07-24

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the shadow/light bake
(`shadowGather.ts`) + the unified data texture (`coldShadowData.ts`) + the prim cache
(`SquareCache.ts`). Layouts are authoritative in [`VARIABLES.md`](../../VARIABLES.md) (the `light_data`,
`prim_data`, `prim_definition_data` bands). Phases in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); problems in [`issues.md`](issues.md). Builds on
[`2026-07-23-unified-data`](../2026-07-23-unified-data/README.md) +
[`2026-07-23-presence-in-data`](../2026-07-23-presence-in-data/README.md) +
[`2026-07-23-def-frame-anchors`](../2026-07-23-def-frame-anchors/README.md)._

## The problem (user, 2026-07-24)
Lights are **not world objects** — they're a bespoke debug scaffold. `ShadowGather` holds a private
`this.lights: Light[]` array, seeded by `seed()` / `__manylights()` (debug hooks), and the only way
a light reaches the bake is `buildLights()` (`coldShadowData.ts:417`), called from `tick()`
(`shadowGather.ts:1349`) **gated on the hand-set `this.coldDirty` flag**. Nothing about a light flows
through the pipeline that actual world content uses.

Contrast a **prim** (a tree): world content → `SquareCache` → `standingPrims()` (`SquareCache.ts:350`)
→ `shadows.tick(standingPrims(), …)` every frame (`Viewport.ts:350`) → `buildCasters()`
(`shadowGather.ts:1116`) allocates its def + record into the data texture and **dirties automatically**
via the prim cascade + free-list + zone eviction. Placement, movement, and removal are all handled by
the content path — no hand-set flags.

This session proved the gap the hard way: to run a 50-light perf test I pushed 50 lights straight into
`g.lights` and set `g.coldDirty = true` by hand. Nothing happened — `coldData.lightCount` stayed **3**
(the seed lights). The render loop is **change-gated**, so `tick()` never re-ran `buildLights()` with the
new array, and even if it had, the whole approach is a manual injection with no home in the world. **Lights
are effectively unusable as a game feature** until they become placeable objects. That's this stream.

## Two pillars

### 1. A light IS a prim — place it in the world, not inject it into the gather
Model a light as a world object with its own kind/def, delivered to the bake through the **same path
things take** (content → `SquareCache` → a `tick()` prim list → data-texture allocation). The two
records already sit in parallel bands of the unified data texture:
- **`prim_data`** (rows 64–127) — position (`position_anchor_reference`) + a `definition_index`. A light
  gets a record here so its **position** is a placed prim: move the prim → one texel changes → the prim
  dirty cascade fires. This is the anchor everything else keys off.
- **`light_data`** (rows 128–191) — colour · intensity · z · reach · emitter · `hot` · `cast_shadows`.
  Today this is written directly from `this.lights`. Under this stream it becomes **derived from the
  light-prim + its light-def**, not from a bespoke array.
- **light def** — the static light properties (colour/reach/emitter/height/hot/cast) belong to a
  **definition** (like a prim's atlas-frame def), so "torch" vs "moonlight" are defs and a placed light
  just references one + carries a position. Whether this reuses `prim_definition_data` with a light flag
  or is a new light-def band is [forks.md#f2](forks.md#f2).

The exact record wiring — light writes both `prim_data` + `light_data`, vs `light_data` derived from a
light-prim each frame, vs retiring `light_data` and reading light props from the def — is
[forks.md#f1](forks.md#f1).

### 2. Dirty light behaviour, systematised — stop flipping flags by hand
Today the gather juggles `coldDirty`, `forceColdDirty`, `forceHotDirty`, `lightsVer`, `pendingRects`,
`lastCasterCount`, `markLightMove()`, and `buildDirty()` — and the debug hooks set several of them by
hand on every mutation (the exact thing that failed this session). Once a light is a prim, its
dirtying should ride the **existing prim cascade** (a prim change dirties its tiles → the lights reaching
those tiles → those lights' cast regions), the same machinery `buildCasters`/`markPrimChange` already
run for casters. **Placing / moving / removing a light-prim dirties the correct rects automatically**;
the manual `coldDirty`-and-friends flag-flipping retires. The full flag inventory + who sets/clears each
+ the target automatic model is enumerated in [`todo.md`](todo.md) (P0) and [`issues.md`](issues.md).

## The payoff
Lights become an **authorable, placeable game feature**: a light is declared in content (a light kind,
alongside `<thing>` in `content/data/things.rd`) and/or attached to an emissive prim, placed by worldgen
or gameplay, cached and delivered like any object, and baked with correct automatic dirtying. The
`seed()`/`__manylights()`/`this.lights` scaffold + the hand-set flags are deleted. This is the
prerequisite for every downstream lighting effect (day/night, emissive things, torches) — they all need
lights that exist **in the world**, not in a debug array.

## What it composes with / does NOT change
- **Unblocks, doesn't re-architect the bake**: the gather/lightmap math, the cold/hot class split, the
  presence cull (≤14 lights/tile), and corridor↔brute identity are unchanged. This stream changes **how a
  light gets INTO the data texture and how its change dirties the bake**, not how the bake reads it.
- Rides [`2026-07-23-presence-in-data`](../2026-07-23-presence-in-data/README.md)'s per-tile light cull and
  the two-layer eviction (a light-prim evicts on zone exit like any prim).
- The `light_data` bit-layout is owned by [`VARIABLES.md`](../../VARIABLES.md); any change to it lands there
  first, then the code conforms.
