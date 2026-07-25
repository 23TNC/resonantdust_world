# Light prims — make lights first-class world objects (placed + auto-dirtied like things) — 2026-07-24

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the shadow/light bake
(`shadowGather.ts`) + the unified data texture (`coldShadowData.ts`) + the primitive cache
(`SquareCache.ts`). Layouts are authoritative in [`VARIABLES.md`](../../VARIABLES.md) (the `light_data`,
`billboard_data`, `billboard_definition_data` bands).

**Vocabulary (ratified 2026-07-24).** A **primitive** is a DSL-declared world object. It *presents* as a
**billboard** (a view-plane sprite — the shadow *caster* + lit surface: trees, pawns) or as a **light**
(an emitter) — or both (an emissive sprite). The data bands are per-**presentation**:
`billboard_definition_data` + `billboard_data` = the billboard presentation; `light_data`
(+ a future `light_definition_data`, [F2](forks.md#f2)) = the light presentation. What the code used to
call "prim" was always the billboard; "primitive" is the umbrella. (Rename landed 2026-07-24 —
`prim_* → billboard_*`, `markPrimChange → markBillboardDirty`; the `Primitive` cache type stays.) Phases in [`todo.md`](todo.md); decisions in
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

Contrast a **billboard** (a tree): world content → `SquareCache` → `standingPrims()` (`SquareCache.ts:350`)
→ `shadows.tick(standingPrims(), …)` every frame (`Viewport.ts:350`) → `buildCasters()`
(`shadowGather.ts:1116`) allocates its def + record into the data texture and **dirties automatically**
via the billboard cascade (`markBillboardDirty`) + free-list + zone eviction. Placement, movement, and
removal are all handled by the content path — no hand-set flags. The light presentation should work the
same way.

This session proved the gap the hard way: to run a 50-light perf test I pushed 50 lights straight into
`g.lights` and set `g.coldDirty = true` by hand. Nothing happened — `coldData.lightCount` stayed **3**
(the seed lights). The render loop is **change-gated**, so `tick()` never re-ran `buildLights()` with the
new array, and even if it had, the whole approach is a manual injection with no home in the world. **Lights
are effectively unusable as a game feature** until they become placeable objects. That's this stream.

## Two pillars

### 1. The light presentation is placed, not injected
A **light** is one presentation of a primitive. Deliver it to the bake through the **same path** the
billboard presentation uses (content → `SquareCache` → a `tick()` primitive list → data-texture
allocation), so it rides the free-list, eviction, and the scoped dirty cascade for free. The
presentation records sit in **parallel bands** of the unified data texture:
- **`billboard_definition_data` + `billboard_data`** (rows 0–127) — the **billboard** presentation: an
  atlas-frame def + a placed record (position + `definition_index`). A primitive that presents as a
  billboard writes these (the tree path today).
- **`light_data`** (rows 128–191) — the **light** presentation: position + colour · intensity · z · reach ·
  emitter · `hot` · `cast_shadows`. A primitive that presents as a light writes this. Today it's written
  from the `this.lights` scaffold; under this stream, from the placed light primitive — allocated with the
  same free-list + compare-write + `mark` the billboard record uses (`billboardDataFor`).
- **`light_definition_data`** ([F2](forks.md#f2), lean: a new band **parallel to
  `billboard_definition_data`**) — the static light props (colour/reach/emitter/height/hot/cast) as a
  shared **def**, so "torch"/"moonlight" are defs a placed light references; `light_data` then carries a
  position + a `definition_index`, exactly mirroring `billboard_data`.

A primitive that presents as **both** (an emissive sprite — a torch with a glow) writes `billboard_data`
**and** `light_data`: one placed object, two presentations, one shared position. Whether a pure light
also needs a `billboard_data` record (it doesn't — light_data carries its position) is [F1](forks.md#f1).

### 2. Dirty behaviour, systematised — two named entry points, stop flipping flags by hand (user)
Today the gather juggles `coldDirty`, `forceColdDirty`, `forceHotDirty`, `lightsVer`, `pendingRects`,
`lastCasterCount`, `markLightMove()`, and `buildDirty()` — and the debug hooks set several of them by
hand on every mutation (the exact thing that failed this session). The model (user): **two entry points
by presentation** —
- **`markBillboardDirty`** (the renamed `markPrimChange`, `shadowGather.ts:1185`) — a billboard changed
  (placed / moved / re-def'd) → dirty its tiles + cascade to the lights reaching them.
- **`markLightDirty`** (new) — a light changed (placed / moved / props) → dirty its cast region.

Both **cascade into `pendingRects` → `buildDirty`** (the existing scoped-rect path, `:1177-1183` /
`:1246-1256`) — so placement/movement/removal dirties the correct rects automatically and the manual
`coldDirty`/`forceColdDirty`/`lightsVer` flag-flipping retires. **Agreed follow-on (user): generalise the
cold/hot split** — today a light's class (static→cold / dynamic→hot) is bound to `L.dynamic` and routed by
ad-hoc `cls` args; the two `markDirty` entry points should carry class generically so a presentation's
dirty lands in the right class pass without bespoke wiring. Flag inventory + target model: [`todo.md`](todo.md)
P3, [`issues.md`](issues.md), [F4](forks.md#f4).

## The payoff
Lights become an **authorable, placeable game feature**: a light is declared in content (a light kind,
alongside `<thing>` in `content/data/things.rd`) and/or attached to an emissive primitive, placed by worldgen
or gameplay, cached and delivered like any object, and baked with correct automatic dirtying. The
`seed()`/`__manylights()`/`this.lights` scaffold + the hand-set flags are deleted. This is the
prerequisite for every downstream lighting effect (day/night, emissive things, torches) — they all need
lights that exist **in the world**, not in a debug array.

## What it composes with / does NOT change
- **Unblocks, doesn't re-architect the bake**: the gather/lightmap math, the cold/hot class **semantics**
  (static baked once / dynamic per-frame — the routing is generalised, not the split), the presence cull
  (≤14 lights/tile), and corridor↔brute identity are unchanged. This stream changes **how a
  light gets INTO the data texture and how its change dirties the bake**, not how the bake reads it.
- Rides [`2026-07-23-presence-in-data`](../2026-07-23-presence-in-data/README.md)'s per-tile light cull and
  the two-layer eviction (a light-prim evicts on zone exit like any prim).
- The `light_data` bit-layout is owned by [`VARIABLES.md`](../../VARIABLES.md); any change to it lands there
  first, then the code conforms.
