# Todo — lighting

_Phased so the renderer keeps working after each step (the current single-pass stopgap is replaced
piece by piece, not big-bang). Items move to `completed.md` as they land + verify. Design:
[`README`](README.md) · decisions [`forks.md`](forks.md) · deps [`blockers.md`](blockers.md)._

---

## P0 · Foundation (plumbing — no visual change)

- [ ] **`Light` unification** — one struct `{x, y, z, radius, r, g, b, brightness, castsShadow, tier}`
      (replaces the current ad-hoc `PointLight`); `LightRig` routes each to **cold** (static, per-rect
      bake) vs the **dynamic** pool (moving / real-time) by `tier`.
- [ ] **Serve + consume the outline** (art-metadata P3): fold `meta.json` into the content manifest
      (like `atlas.json`); client decodes `outline` (Sidecar → per-def polygons + earcut tris) +
      `channel_tints`. Gate P3 (scatter) on this.
- [ ] Bit helpers: float-mod byte/bit extraction (GLSL ES 1.00, `mod(floor(byte/exp2(b)),2.0)`) shared
      by the display + combine shaders.

## P1 · Cold lightmap (baked static light, no shadow yet) — ✅ core DONE + live-verified

- [x] **`cold_lightmap` bake** — a **derived** `SquareCache` composite (F6): `bakeLightmapSquare` samples
      each just-baked square's NORMAL slot + sums the cold lights into `lightmap-cold`
      (`Σ cold·brightness·max(N·L,0)·falloff²`), world-space via `uRectWorld`, reusing the toroidal
      scratch/apron/dirty machinery. `LightingBakeShader` + `LightRig.packCold` + `enableLightBake`.
- [x] Display samples `lightmap-cold` and adds it to the light sum (`+ cold`). `?coldlight` seeds a debug
      static light. **Verified live** — `?ambient=0.3&coldlight` renders a baked amortized orange pool on
      the ground (soft falloff, world-space, trees on top), console clean.
- [ ] _(follow-up)_ **cold `data`/`color` light textures** so **cards** (things) re-evaluate cold lights
      on their OWN normal (the bake uses the ground normal). Port `coldLightTex.ts`.
- [ ] _(follow-up)_ a proper **`lightDirty`** trigger (rebake a rect when a cold light in range changes
      even if geometry didn't); today a cold-light change relies on a geometry rebake / `invalidateAll`.

## P2 · Dynamic pool (real-time, gate stubbed all-lit)

- [ ] Restructure the current 32-light live loop into the **dynamic pool**: 32 lights in uniforms
      (`32 × 2 vec4`), structured as **4 channels × 8** to dodge ES-1.00 dynamic indexing; per light
      `falloff·N·L`, gated by `gate(i)` (stub `gate=1`). Keep it visually identical to today, just
      restructured for the bitfield gate P3 adds.

## P3 · Projected-silhouette shadows (the per-light win) — retire the stopgaps

- [x] **Scatter shader + geometry** (`37f6cc8`) — `scatterShader.ts` ported: the lane shader
      (`outColor = uChannel`, `max`-blend), `makeShadowGeometry`, + the tuning constants
      (`SHADOW_MAPS`/caps/height-falloff/`channelForLight`). B1 (`OutlineCache`) feeds it.
- [ ] **`ScatterPass` driving** — the 2 RGBA maps + per-light render: adapt the Viewport's
      `buildCasters` to yield `{feetX, groundY, footNY, h, w, left, stem}` + `OutlineCache.get(stem)`;
      per shadow-casting light (≤8), gather its casters (within radius), **port `projectCaster`** (the
      billboard shear per outline vertex, emitting the earcut tris into the geo buffer), render each
      light's casters into its lane. **⚠ the `projectCaster` shear constants were tuned to the OLD
      coordinate space — re-tune in the browser (`/showRT` the scatter maps).**
- [ ] **`warm_shadowmap`** — 32-bit/pixel, world-space, **double-buffered ping-pong**: each frame read
      `prev warm + the 8 fresh scatter channels`, write `next warm` with those 8 bits updated
      (round-robin 32/8 = 4-frame cycle; the deferred writeback = "commit last frame's slice at the top
      of this one"). Port `bitfield.ts` combine.
- [ ] Display `gate(i)` = the warm bit, with a **cross-fade** for the 8 fresh (`0.5·(warmBit+freshBit)`)
      so a shadow's catch-up is a 1-frame fade, not a snap.
- [ ] **Retire** the wedge [`shadowPass.ts`](../../../client/pixijs/src/game/viewport/shadowPass.ts) +
      the single global-shadow multiply in the display. Verify multi-light: two shadow-casters, B lighting
      A's shadow does NOT un-shadow it.

## P4 · Cold shadows (baked, per-pixel)

- [ ] **cold `active` map** — CPU-built 32-bit/pixel, rect-aware (a cold light's per-pixel occlusion,
      handling shadows crossing rect boundaries — the thing GPU scatter can't do per-rect). Fold into the
      `cold_lightmap` bake (`Σ cold·N·falloff·(1−shadow)`).

## P5 · Content + cleanup

- [ ] **DSL static lights** — author cold point lights in content (`content/…`), the source the cold
      tier exists for; classify light→tier at worldgen/load. (The "dense many-lights" payoff.)
- [ ] Migrate the **cursor** light into the dynamic pool (real-time slot); delete the flat live-loop
      stopgap + any dead single-shadow plumbing.
- [ ] Graduate the [`README`](README.md) architecture into a `client/pixijs` **design** doc.

---

**Done when:** the display is `albedo × (cold_lightmap + Σ dynamic·falloff·N·L·gate)` — many static
lights amortized in cold, 32 dynamic with per-light projected-silhouette shadows (round-robin + fade),
no global-mask cross-contamination, the wedge/stopgap paths deleted, browser-verified.
