# Todo — lighting

_Phased so the renderer keeps working after each step (the single-pass stopgap is replaced piece by
piece). Items move to [`completed.md`](completed.md) as they land + verify. Design: [`README`](README.md)
· strategy [`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md) ·
decisions [`forks.md`](forks.md) · deps [`blockers.md`](blockers.md)._

_Shipped so far (in [`completed.md`](completed.md)): P0 tier routing, B1 outline serve/consume, P1 cold
lightmap, P3 scatter-shader foundation, P4 **interim** materialized `shadow-cold`._

---

## P1 follow-ups · cold lightmap refinements

- [ ] **Cold `data`/`color` light textures** so **cards** (things) re-evaluate cold lights on their OWN
      normal (the bake uses the ground normal). Port `coldLightTex.ts`.
- [ ] A proper **`lightDirty`** trigger — rebake a rect when an in-range cold light changes even if
      geometry didn't; today it relies on a geometry rebake / `invalidateAll`.

## P2 · Dynamic pool (real-time, gate stubbed all-lit)

- [ ] Restructure the current 32-light live loop into the **dynamic pool**: 32 lights in uniforms
      (`32 × 2 vec4`), structured **4 channels × 8** to dodge ES-1.00 dynamic indexing; per light
      `falloff·N·L`, gated by `gate(i)` (stub `gate=1`). Visually identical to today — just restructured
      for the bitfield gate P3 adds.

## P3 · Projected-silhouette shadows, dynamic (the per-light win) — retire the stopgaps

- [ ] Bit helpers: float-mod byte/bit extraction (GLSL ES 1.00, `mod(floor(byte/exp2(b)),2.0)`) shared by
      the display + combine shaders (the `warm_shadowmap` gate reads these).
- [ ] **`ScatterPass` driving** — the 2 RGBA maps + per-light render: adapt `Viewport.buildCasters` to
      yield `{feetX, groundY, footNY, h, w, left, stem}` + `OutlineCache.get(stem)`; per shadow-casting
      light (≤8), gather its casters (within radius), run `projectCaster` into its lane. **⚠ the
      `projectCaster` shear constants were tuned to the OLD coord space — re-tune in the browser
      (`/showRT` the scatter maps).**
- [ ] **`warm_shadowmap`** — 32-bit/pixel, world-space, **double-buffered ping-pong**: each frame read
      `prev warm + the 8 fresh scatter channels`, write `next warm` with those 8 bits updated (round-robin
      32/8 = 4-frame cycle; deferred writeback = "commit last frame's slice at the top of this one").
- [ ] Display `gate(i)` = the warm bit, with a **cross-fade** for the 8 fresh (`0.5·(warmBit+freshBit)`)
      so a shadow's catch-up is a 1-frame fade, not a snap.
- [ ] **Retire** the wedge [`shadowPass.ts`](../../../client/pixijs/src/game/viewport/shadowPass.ts) + the
      single global-shadow multiply in the display. Verify multi-light: two shadow-casters, B lighting A's
      shadow does NOT un-shadow it. (Also reconcile the `nsProject` axis — [D-2](deviations.md).)

## P4 · Cold shadows — inline occlusion in the bake, replacing the interim map

**Target = the inline sweep** ([intent/tiered-lighting.md](../../components/client/pixijs/intent/tiered-lighting.md),
[F7](forks.md#f7)): the bake tests each light's projected silhouettes per pixel and only adds the light
where unshadowed — no materialized map, **unbounded** cold shadow-casters. Replaces the interim
`shadow-cold` composite ([completed.md](completed.md), [D-1](deviations.md)). Gated on [B3](blockers.md#b3)
(the inline data-texture + point-in-shape technique, matched to `../resonantdust`).

- [ ] **Cold lights → a light-data texture** (rgba8, N texels/light: `xy`,`z`,color,intensity,radius),
      replacing the `uLightData[32]` uniform array in `lightingBakeShader`. Unbounded loop from the texture.
- [ ] **Projected-caster geometry into a data texture** the bake indexes per light (radius-culled), + a
      hard-decimated shadow silhouette (≪ the ~180-pt render outline) — see [B3](blockers.md#b3).
- [ ] **Inline occlusion in the bake sum** — per pixel, per in-range light: point-in-silhouette test;
      `sum += lit ? color·intensity·N·L·falloff : 0`.
- [ ] **Remove the interim materialized path** — `shadow-cold` composite + `bakeColdShadowSquare` +
      `enableColdShadow`/`setColdShadowLights` + `LightRig.coldShadowLights()` + the `/showRT` slot +
      `uColdShadow` in the bake shader.
- [ ] Verify live: >3 cold shadow-casters on one square ALL cast (the cap is gone); shadows track lights.

## P5 · Content + cleanup

- [ ] **DSL static lights** — author cold point lights in content, classify light→tier at worldgen/load
      (the "dense many-lights" payoff; [B2](blockers.md#b2)).
- [ ] Migrate the **cursor** light into the dynamic pool; delete the flat live-loop stopgap + dead
      single-shadow plumbing.
- [ ] Graduate the [`README`](README.md) architecture into a `client/pixijs` **design** doc.

---

**Done when:** the display is `albedo × (cold_lightmap + Σ dynamic·falloff·N·L·gate)` — many static
lights amortized in cold (inline shadows, no cap), 32 dynamic with per-light projected-silhouette shadows
(round-robin + fade), no global-mask cross-contamination, the wedge/stopgap paths deleted, browser-verified.
