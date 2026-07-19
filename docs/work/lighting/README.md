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
| **Cold** | ≤32 **per rect** (unlimited across world) | per-rect `data`/`color` rgba8 (rect-local) | CPU per-pixel 32-bit **active map** | pre-summed `cold_lightmap`, dirty-rect rebake | on settle / content change |
| **Dynamic** (warm+hot) | 32 **global** | uniforms | 2 RGBA **scatter maps** (8 lights/frame) → 32-bit `warm_shadowmap` (ping-pong) | recomputed every frame at display | every frame (pos), shadow round-robin |

"Hot" and "warm" are **freshness states of one 32-light dynamic pool**, not separate tiers — each frame 8
of the 32 get a fresh scattered shadow, the other 24 carry a ≤3-frame-stale cached shadow.

**Display composite, per pixel:**
```
lit = albedo × ( cold_lightmap                                  // 1 texture read (ambient + baked cold)
               + Σ_32 dynamic[i] · falloff · N·L · gate(i) )    // gate from the warm bit / fresh scatter
```

**Shadows are projected earcut silhouettes** (the [art-metadata](../art-metadata) `outline` sidecars):
each caster's triangulation is sheared through a light onto the ground (`h/(lightZ−h)`) and rasterized
into one **channel** of an RGBA scatter map. Cheaper than a coverage-mask quad (filled tris, no
per-fragment fetch, holes free) — the reason we ported earcut. This **replaces** both stopgaps: the
per-caster wedge [`shadowPass.ts`](../../../client/pixijs/src/game/viewport/shadowPass.ts) and the single
global shadow in the display.

## The load-bearing constraints (from `tiered_lighting.md` — don't relitigate)

- **A texture channel is 8 bits, a texel is 32.** Lights are quantization-tolerant → rgba8 is enough for
  light *data* (depth's precision bar does not apply).
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

**Net-new** — the `cold_lightmap` bake on the rect tiers + cold `data`/`color`/`active` maps; the
`Light { …, castsShadow, tier }` unification + rig routing (cold vs dynamic); the projected-silhouette
**scatter** (consuming the outlines) → 2 RGBA maps; the `warm_shadowmap` bitfield + **ping-pong** combine
(round-robin, deferred writeback); the display rework (cold_lightmap + dynamic loop + gate + cross-fade).
**Stopgaps to retire:** the wedge `shadowPass`, the single global shadow, the flat 32-light live loop.

**Dependencies:** the outline **serve/consume** path (art-metadata P3 — fold `meta.json` into the
manifest, client decodes it); a source of **static (cold) lights** — DSL point lights in content (the
"dense many-lights" the cold tier exists for). See [`blockers.md`](blockers.md).

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md). The architecture here should
graduate to a `client/pixijs` **design** doc once the port stabilizes (per docs conventions — work
executes, design endures).
