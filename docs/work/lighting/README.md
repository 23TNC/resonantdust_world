# Work — lighting (tiered cold / dynamic light-accumulation)

_Opened 2026-07-18. Port the old game's **tiered lighting** onto the new game's rect/G-buffer, replacing
the current single-pass stopgap (ambient + sun + a 32-light real-time loop gated by ONE global cursor
shadow). Authoritative prior design: `../resonantdust/docs/tiered_lighting.md` (design-locked). Depends
on the [art-metadata](../art-metadata/README.md) **outline** sidecars (now built) + their serve path._

## Why — the problem the current renderer can't solve

Lighting a 2.5D board with **many** lights, each with **per-light** shadows. The cost that dominates is
rebuilding shadows — a *per-light* cost — so lights are split by how often they change. The new game's
[`lightingShader.ts`](../../../client/pixijs/src/game/viewport/lightingShader.ts) sums all lights into
one `direct` term and multiplies it by a **single** screen-space shadow (from `shadowCasters()[0]`, the
cursor). That's a global lit/unlit mask: it breaks the instant a second shadow-caster exists (light B
lighting a spot "un-shadows" it for A). The fix is **per-light occlusion inside the accumulation**, which
the tiered model gives by construction — each light's contribution is computed *with its own occlusion*
and **added**, so summing can't cross-contaminate.

## Architecture (target)

Two tiers, composited into the display:

| Tier | Count | Light data | Shadow build | Lightmap | Update |
|---|---|---|---|---|---|
| **Cold** | **unlimited** (radius-culled per rect + world) | **light-data texture** (rgba8, N texels/light) | **inline** per-px per-light occlusion — **no shadow map** | pre-summed `cold_lightmap`, dirty-rect rebake | on settle / content change |
| **Dynamic** (warm+hot) | 32 **global** | uniforms | 2 RGBA **scatter maps** (8 lights/frame) → 32-bit `warm_shadowmap` (ping-pong) | recomputed every frame at display | every frame (pos), shadow round-robin |

The two tiers build shadows **oppositely**, on purpose. **Dynamic** runs every frame over the whole
viewport, so it *can't* afford a per-pixel light sweep — it **rasterizes** each light's silhouettes into
a map (scatter → bitfield) and the display *reads* the map. **Cold** is amortized (only dirty rects, a
few per frame), so it does the sweep **inline** and never materializes a shadow map — which is what frees
it from the map's channel cap and lets it carry *unlimited* lights (see [Cold strategy](#cold-lighting-strategy--the-inline-sweep)).

"Hot" and "warm" are **freshness states of one 32-light dynamic pool**, not separate tiers — each frame 8
of the 32 get a fresh scattered shadow, the other 24 carry a ≤3-frame-stale cached shadow.

**Display composite, per pixel:**
```
lit = albedo × ( cold_lightmap                                  // 1 texture read (ambient + baked cold)
               + Σ_32 dynamic[i] · falloff · N·L · gate(i) )    // gate from the warm bit / fresh scatter
```

**Both tiers project the same earcut silhouettes** (the [art-metadata](../art-metadata) `outline`
sidecars): a caster's triangulation is sheared through a light onto the ground (`h/(lightZ−h)`). What
differs is the consume: **dynamic** rasterizes them into a **channel** of an RGBA scatter map (filled
tris, no per-fragment fetch, holes free — the reason we ported earcut, and cheaper than a coverage quad
on the per-frame path); **cold** tests them **inline** (below), no map. Both **replace** the stopgaps —
the per-caster wedge [`shadowPass.ts`](../../../client/pixijs/src/game/viewport/shadowPass.ts) and the
single global shadow in the display.

## Cold lighting strategy — the inline sweep

The cold tier's full strategy (bake `lightmap-cold` per dirty square; loop a **light-data texture** per
pixel; test each light's silhouettes **inline**; no shadow map → **unbounded** cold shadow-casters) is the
durable target and lives in
[**intent/tiered-lighting.md**](../../components/client/pixijs/intent/tiered-lighting.md) — the owner for
it; not restated here. The open sub-problem (getting the projected geometry into the fragment shader +
decimating the silhouette) is [B3](blockers.md#b3). The current build is the interim materialized
`shadow-cold` map ([completed.md](completed.md), [D-1](deviations.md)), replaced by the sweep in
[`todo.md`](todo.md) P4.

## The load-bearing constraints (from `tiered_lighting.md` — don't relitigate)

_Scope: the channel/bitfield constraints below govern the **dynamic** tier's **rasterized-scatter**
shadow build. The **cold** tier does a **gather** (inline per-pixel sweep), free of the merge wall (4th
bullet), so it inherits only **"cold can't subtract."** The channel cap does **not** apply to cold — that
is exactly why the interim `shadow-cold` map ([D-1](deviations.md)) is only interim._

- **A texture channel is 8 bits, a texel is 32.** Lights are quantization-tolerant → rgba8 is enough for
  light *data* (depth's precision bar does not apply). (Applies to both — the cold light-data texture too.)
- **The GPU can't merge a bitfield by blend** (`max` drops bits, `add` carries) → each scattered light
  needs its **own channel** (4/RGBA; premultiplied tint couples rgb↔alpha, so the `uChannel`-uniform
  trick recovers the 4th lane → 8 with two maps).
- **Identity must survive** — a *count* of "how many lights reach here" is useless (removing a warm torch
  ≠ removing a cold blue). Every encoding keeps light identity.
- **Gather has no merge problem** — summing lights in one shader invocation is free; the merge wall is
  purely about building shadows by **rasterized scatter**.
- **Cold can't subtract** — a baked `Σ lights` can't drop one term, so a dirty cold region re-sums all
  its lights (adding a light blends in cheaply; a light *leaving* makes the rect light-dirty → rebuild).
  That's why *moving* lights live in the dynamic pool, not cold.
- **Bit extraction is float-mod on GLSL ES 1.00** — no `uint`/`#version 300 es` migration under the
  most-touched shader; rgba8 stores `n/255` exactly, so `mod(floor(byte/exp2(b)),2.0)` is exact.

## What's already in place (new game) vs net-new

**In place** — the G-buffer **rect tiers** ([`SquareCache`](../../../client/pixijs/src/game/viewport/SquareCache.ts):
cold+warm, dirty bake, toroidal, normal/albedo/surface/zdepth); [`LightRig`](../../../client/pixijs/src/game/lighting/LightRig.ts)
(point lights, cursor, z/radius); the display mesh + shader; the **outline** sidecars (art-metadata).

**Net-new** — the `cold_lightmap` bake on the rect tiers + the cold **light-data texture** + **inline
occlusion** in the bake ([F7](forks.md#f7), no shadow map); the `Light { …, castsShadow, tier }`
unification + rig routing (cold vs dynamic); the dynamic projected-silhouette **scatter** (consuming the
outlines) → 2 RGBA maps; the `warm_shadowmap` bitfield + **ping-pong** combine (round-robin, deferred
writeback); the display rework (cold_lightmap + dynamic loop + gate + cross-fade).
**Stopgaps to retire:** the wedge `shadowPass`, the single global shadow, the flat 32-light live loop.

**Dependencies:** the outline **serve/consume** path (art-metadata P3 — fold `meta.json` into the
manifest, client decodes it); a source of **static (cold) lights** — DSL point lights in content (the
"dense many-lights" the cold tier exists for). See [`blockers.md`](blockers.md).

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md). The durable **target** (which
outlives this work dir) lives in [`client/pixijs/intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md)
— the cold inline-sweep strategy + the traps. The rest here (phased plan, interim build, blockers)
graduates into that intent + a `client/pixijs` **design** doc once the port stabilizes (per docs
conventions — work executes, design endures).
