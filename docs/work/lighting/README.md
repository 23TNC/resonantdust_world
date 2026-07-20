# Work — lighting (tiered cold / dynamic light-accumulation)

> **STATUS 2026-07-19 — NUKED, restarting.** The interim build failed and was removed: the whole
> lighting + shadow stack (`LightRig`, `lightingShader`, `lightingBakeShader`, `shadowPass`,
> `scatterShader`, `projectCaster`, `warmCombineShader`, the cold `lightmap`/`shadow` channels, the
> cursor light + shadows, `OutlineCache`) is gone. The viewport now renders **unlit albedo** via a new
> `albedoBlitShader` (warm-over-cold composite preserved so pawns still draw); the G-buffer bakes
> (albedo/normal/surface/zdepth) survive as the retry foundation. The **Why** + **Architecture** below
> and the intent/design docs remain the target to rebuild toward — but the phased plan / interim state
> in `todo.md` + `completed.md` describe the reverted attempt and need re-planning before the next go.

_Opened 2026-07-18. Port the old game's **tiered lighting** onto the new game's rect/G-buffer, replacing
the current single-pass stopgap (ambient + sun + a 32-light real-time loop gated by ONE global cursor
shadow). Authoritative prior design: `../resonantdust/docs/tiered_lighting.md` (design-locked). Depends
on the [art-metadata](../art-metadata/README.md) **outline** sidecars (now built) + their serve path._

## Why — the problem the current renderer can't solve

Lighting a 2.5D board with **many** lights, each with **per-light** shadows. The cost that dominates is
rebuilding shadows — a *per-light* cost — so lights are split by how often they change. The reverted
interim `lightingShader.ts` summed all lights into
one `direct` term and multiplied it by a **single** screen-space shadow (from `shadowCasters()[0]`, the
cursor). That's a global lit/unlit mask: it breaks the instant a second shadow-caster exists (light B
lighting a spot "un-shadows" it for A). The fix is **per-light occlusion inside the accumulation**, which
the tiered model gives by construction — each light's contribution is computed *with its own occlusion*
and **added**, so summing can't cross-contaminate.

## Architecture (target)

**Three tiers by update rate, sharing one scatter→bitfield shadow engine.** Full design (the authoritative
owner): [**intent/tiered-lighting.md**](../../components/client/pixijs/intent/tiered-lighting.md) — not
restated here.

| tier | # lights | light data | shadow | computed |
|---|---|---|---|---|
| **cold** | 32/rect | per-rect **texture** | `shadow-cold` **32-bit bitfield** (32 casters) | **baked** `lightmap-cold`, on dirty |
| **warm** | 32 global | uniforms | `shadow-warm` 32-bit bitfield (round-robin) + `shadow-hot` (4 fresh) | display, 4/frame → 8-frame cycle |
| **rt** | 4 global | uniforms | `shadow-rt` (always fresh) | display, every frame |

```
lit = albedo × ( lightmap_cold + Σ₃₂ warm·falloff·N·L·!occ(warm|hot) + Σ₄ rt·falloff·N·L·!occ(rt) )
```

This is the old game's proven design + two upgrades: **`shadow-cold` as a 32-bit bitfield** (old game was
RGB=3) and a dedicated always-fresh **`shadow-rt`**. It **replaces** the stopgaps — the wedge
`shadowPass` and the single global shadow.
All tiers share `projectCaster` + the scatter shader (`outColor=uChannel`, `max` blend, 4 lanes/map).

## The load-bearing constraints (from `tiered_lighting.md` — don't relitigate)

_These govern the shared scatter→bitfield engine (cold + warm + rt all use it). The `max`-blend + 4-lane
`uChannel` constraints are why a scatter map holds 4 lights and the occlusion field is a 32-bit bitfield
(not RGB=3). "Cold can't subtract" is why cold **bakes** while warm/rt sum at display._

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
cold+warm, dirty bake, toroidal, normal/albedo/surface/zdepth); the display mesh + shader; the
**outline** sidecars (art-metadata). (`LightRig` — point lights, cursor, z/radius — was part of the
nuked build; the retry re-adds a light source.)

**Net-new** — the cold **light-data texture** + the **32-bit `shadow-cold` bitfield** ([F7](forks.md#f7),
32 casters, bake-time); the dynamic **scatter → `shadow-warm` bitfield** + **ping-pong** writeback
(round-robin) + the fresh `shadow-hot`; the always-fresh **`shadow-rt`** priority map; the display rework
(`lightmap_cold` + the 32-warm + 4-rt loop gated by the bitfields + cross-fade).
**Stopgaps to retire:** the wedge `shadowPass`, the single global shadow, the RGB=3 cold cap.

**Dependencies:** the outline **serve/consume** path (art-metadata P3 — fold `meta.json` into the
manifest, client decodes it); a source of **static (cold) lights** — DSL point lights in content (the
"dense many-lights" the cold tier exists for). See [`blockers.md`](blockers.md).

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md). The durable **target** (which
outlives this work dir) lives in [`client/pixijs/intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md)
— the cold inline-sweep strategy + the traps. The rest here (phased plan, interim build, blockers)
graduates into that intent + a `client/pixijs` **design** doc once the port stabilizes (per docs
conventions — work executes, design endures).
