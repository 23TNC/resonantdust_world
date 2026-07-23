# Lighting rebuild — 2026-07-22

_Component: [`client/webgl`](../../components/client/) · viewport shadow/lighting
(`game/viewport/shadowGather.ts`, `game/viewport/coldShadowData.ts`)._

Full teardown + clean re-implementation of the billboard-quad shadow system. Supersedes
`2026-07-22-shadow-corridor` (archived → `~/archive/shadow-corridor/`; the corridor march was
removed — see [`issues.md`](issues.md) for why). Phases in [`todo.md`](todo.md); the LOD-coupling
decision is [`forks.md`](forks.md#f1).

**Foundation: [`map-model.md`](map-model.md) — the shared toroidal TILE grid.** Every map (data
textures AND full-res) is `cols × rows` **tiles**, same shape, same wrap; each picks a per-tile
resolution (TILES=1 · UNITS=16 · SQUARE=64). This is the decision the rebuild stands on, and the fix
for the reach-walk bug ([`issues.md`](issues.md)). Read it first.

The system had accreted debug scaffolding, a broken 1-tile corridor march, a `u/v`-inversion with
precision cliffs, and a **zoom-dependent** dropout (casters gated on texture-LOD resolution). We
tear it back to a **geometric, texture-agnostic quad** shadow that works at every zoom, get it
correct with brute force, texture it, then re-earn the corridor as a pure optimization that must
match the brute-force output bit-for-bit.

## The model

- A **caster** is a flat 3D **billboard card**: base on the ground (`z = 0`), top tilted 65° north
  and elevated. Project its four corners from the light onto the ground → a convex **ground quad**.
- A ground point `P` is **occluded** by a caster iff `P` lies **inside** that quad. This is a
  point-in-convex-quad test (four edge cross-products share a sign) — **purely geometric, needs no
  knowledge of the sprite texture**. This is the invariant that kills the whole `u/v`-out-of-range
  class of bug: we never invert a projection, we only test membership.
- The shadow map stores, per unit-cell, the **colour of the occluding light(s)**.

## Hard constants (do not re-derive, do not mis-transcribe)

| quantity | value | notes |
|---|---|---|
| **unit** | `1/16` tile = **4 px** | `UNIT = SQUARE/16`, `SQUARE = 64 px` |
| **units per tile** | **16** | `UPT = SQUARE/UNIT` |
| **shadow-map resolution** | **16×16 texels/tile** | 1 texel = **1 square unit** = 4 px |
| **light** | tile **(54,21)**, **z = 40 units** off the ground | the debug seed |
| **tree caster height** | **2 tiles = 32 units** (128 px) | ← TREES ARE TWO TILES TALL |
| **per-tile capacity** | **≤ 8 lights**, **≤ 8 shadow casters** | presence + caster-bucket textures |
| **all math** | **in units** | px only on the JS bucketing/presence side |

## Data textures

- **Light data** — per light: position, **colour**, **reach**. (Debug gizmo: draw the light as a
  filled circle of its colour + a circle at its reach.)
- **Presence** (per tile) — which lights act on this tile (≤ 8).
- **Caster buckets** (per tile) — which shadow-casting prims sit on this tile (≤ 8).

## Per-pixel gather (the fragment)

For each shadow-map texel (= one square unit on a tile):
1. For each **light acting on this tile** (presence), check the light's **position + reach**.
2. **Walk every tile within reach** for shadow casters (brute force for now — corridor comes last).
3. For each caster, build its **projected quad** from the caster + light and test if the texel is
   **inside** it (geometric).
4. If occluded, write the **light's colour** into the shadow map.

## Non-negotiables

- **Must render at every zoom level.** The current code fails this because casters were gated on
  texture-LOD resolution ([`issues.md#zoom`](issues.md)). The rebuild must not couple shadow
  presence to texture loading.
- **No `u/v`-out-of-range rejects** — we only sample the sprite *after* proving `P` is inside the
  quad, so the sample coordinate is in-range by construction.
- **Prim `x/y` never changes.** Phase 3 tightens the shadow bbox via the sprite frame's
  `x/y/w/h`; that shifts the *shadow quad*, not the prim's world position.
