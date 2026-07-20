# Tiered lighting — cold/warm/rt with a shared bitfield shadow engine (durable intent)

**Status (2026-07-19).** The target lighting architecture. This is the old game's proven design
(`../resonantdust/view/src/game/lighting` + `viewport/rects`) plus **two upgrades**: a 32-bit cold-shadow
bitfield (lifts the cold shadow-caster cap 3→32) and a dedicated always-fresh priority map. The current
renderer is the 0.1 forward pass (`design/lighting.md`); this is what it grows into.

## The frame — dense many-lights, split by update rate

A **dense many-lights** world (point lights on DSL prims; darkness + light pools is the aesthetic —
ambient/sun are a faint floor). The dominating cost is rebuilding **shadows** (per-light), so lights are
split by how often they move, and all three tiers share **one scatter→bitfield shadow engine**.

| tier | # lights | light DATA in | shadow stored in | when computed |
|---|---|---|---|---|
| **cold** | 32 **per rect** (unbounded across world) | a **per-rect texture** (2 texels/light) | `shadow-cold` — **32-bit bitfield** | **baked** into `lightmap-cold` (on dirty) |
| **warm** | 32 **global** | **uniforms** | `shadow-warm` (32-bit bitfield, round-robin) + `shadow-hot` (4 fresh this frame) | **display**, 4 lights refreshed/frame → 8-frame cycle |
| **rt** | 4 **global** priority | **uniforms** (with warm → 36) | `shadow-rt` (4 lanes, always fresh, never round-robin) | **display**, every frame |

**Display, per pixel:**
```
lit = albedo × ( lightmap_cold                                  // hundreds of static lights, BAKED (N·L already in it)
               + Σ₃₂ warm·falloff·N·L·!occ(warm|hot)            // occ from the 32-bit warm bitfield / fresh shadow-hot lane
               + Σ₄  rt·falloff·N·L·!occ(rt) )                  // occ from the always-fresh shadow-rt lanes
```

## Cold — baked, 32 lights/rect, 32-bit shadow bitfield

Static lights (the authored bulk, hundreds). Baked into `lightmap-cold`, a derived `SquareCache`
composite, rebaked only when a rect's geometry or an in-range cold light changes.

- **Lights come from a per-rect texture**, not uniforms — each rect's nearest 32 differ, so a per-rect
  texture (2 texels/light: `xy`,`z`,`radius` then colour,brightness) avoids re-uploading 32 uniforms per
  rect. (The old game's `coldDataTex`.)
- **Shadows are a 32-bit `shadow-cold` bitfield** (built on dirty via the shared scatter engine — 32
  lights = 8 scatter passes at 4 lanes each, written into the bitfield). The bake reads a light's bit and
  drops its term where occluded. **This lifts the cold shadow-caster cap from 3 to 32** — every cold light
  in a rect casts a shadow, the thing the old game's 3-lane `uColdShadow` could not.
- **N·L is baked in** — the bake samples the normal composite, so `albedo × lightmap_cold` is correct for
  ground + static things with no display-time light loop.
- `shadow-cold` is a **bake-time input only** — consumed when `lightmap-cold` re-bakes; the display never
  reads it. Cold's 32-light shadows cost nothing per frame, only on dirty.

## Warm — display-summed, round-robin bitfield

32 global dynamic lights (movers). Evaluated **at display** every frame (they move, so nothing bakes).

- **Light data in uniforms** (global — the same 32 everywhere).
- **`shadow-warm`** is a 32-bit-per-pixel occlusion bitfield (bit i = light i occluded). Refreshed
  **round-robin**: each frame the shared scatter engine rasterizes **4 fresh** lights into `shadow-hot`
  (4 lanes), then a **ping-pong writeback** sets those 4 bits in `shadow-warm`. 4/frame → the whole 32
  refresh in **8 frames** (~130ms); a warm light's shadow is at most that stale.
- Display: the **4 fresh** lights read their `shadow-hot` lane directly (zero-lag); the other **28** read
  their `shadow-warm` bit (a real `uint` bitwise extract — the client is GLSL ES 3.00, see
  [`design/rendering-platform.md`](../design/rendering-platform.md)).

## RT — display-summed, always fresh (zero staleness)

4 highest-priority lights (the cursor, key torches). Same display path as warm, but their shadows go into
**`shadow-rt`** (4 lanes) **rasterized every frame** — never round-robin, never written to a bitfield. So
they have **zero staleness**. Their data rides the same uniform array as warm (36 total).

## The shared scatter→bitfield engine

One primitive serves cold, warm, and rt:

- **`projectCaster`** shears a caster's earcut silhouette (the `outline` sidecar) through a light onto the
  ground (`h/(lightZ−h)`).
- **`scatterShader`** rasterizes it into one **lane** of an RGBA map: `outColor = uChannel` (a `1` in one
  lane — bypasses the tint premultiply that couples RGB↔A), **`max` blend** (coverage clamps at 1). The
  `uChannel` trick gives **4 clean lanes/map**.
- **Ping-pong writeback** packs fresh lanes into the 32-bit bitfield (`blendMode "none"` verbatim RGBA so
  alpha survives).

Warm drives it round-robin (4/frame); cold drives it on-dirty (8 passes to fill 32 bits); rt drives it
every-frame into its own map. **Same code, different light sets + triggers.**

## RTs + budget

`lightmap-cold`, `shadow-cold` (×2 ping-pong), `shadow-warm` (×2 ping-pong), `shadow-hot`, `shadow-rt`,
+ the per-rect **cold light-data texture**. Uniform lights: 32 warm + 4 rt = **36** (72 vec4 — well within
limits). All these RTs are RGBA8, `nearest`, and any verbatim/bitfield write is non-premultiplied.

## What's the old game vs net-new

- **From the old game (proven):** the G-buffer rect tiers, cold `lightmap` bake, `projectCaster`, the
  scatter shader (`uChannel`/4-lane/max), the 32-bit warm bitfield + ping-pong, the cold light-data
  texture, the display bitfield gate.
- **Net-new (the two upgrades):** `shadow-cold` as a **32-bit bitfield** (old game was RGB=3);
  **`shadow-rt`** as a dedicated always-fresh priority map (old game left the cursor in the round-robin).

## Anti-goals (the traps)

- **Do NOT bake warm/rt lights.** They move; baking forces a re-accumulation you can't cheaply undo
  (a baked sum can't subtract one term). Only **cold** bakes — it doesn't move.
- **Do NOT cap cold shadows at 3 (RGB lanes).** That's the old `uColdShadow`; use the 32-bit bitfield.
- **Do NOT use `add` blend for the scatter lanes** — `max`, so overlapping caster tris clamp at 1.
- **Do NOT put cold lights in global uniforms** — they're per-rect; use the per-rect texture.

## References

- Work (in-flight): [`docs/work/shadows/`](../../../../work/shadows/README.md) — the shadow-engine
  foundation restart (the prior `work/lighting` port was nuked 2026-07-19, archived out-of-repo).
- Current renderer: [`design/lighting.md`](../design/lighting.md).
- Prior working design: `../resonantdust/view/src/game/lighting` (`bitfield.ts`, `shadowMaskShader.ts`,
  `rectLightBakeShader.ts`, `coldLightTex.ts`, `rectDisplayShader.ts`, `RectComposite.ts`) +
  `../resonantdust/docs/tiered_lighting.md`.
