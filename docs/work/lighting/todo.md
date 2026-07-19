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

## P1 · Cold lightmap (baked static light, no shadow yet)

- [ ] **`cold_lightmap` bake** on the `SquareCache` rect tiers — a per-rect quad samples the rect's
      NORMAL slot and writes `ambient + Σ cold·brightness·max(N·L,0)·falloff²` into a `lightmap` channel,
      **world-space** (pans free), dirty-rect rebake (`lightDirty` = a cold light in range changed).
      Port `rectLightBakeShader.ts`.
- [ ] **cold `data`/`color` light textures** (per-rect, rect-local rgba8) — `encodeColdLight`
      (`x−128,y−128,z,radius` + `r,g,b,brightness`) so **cards** (things) can re-evaluate cold lights on
      their OWN normal at display (the bake used the ground normal). Port `coldLightTex.ts`.
- [ ] Display: `lit = albedo × (cold_lightmap + <existing dynamic loop>)`. Verify static lights render +
      **amortize** (no per-frame cost at rest; only dirty rects rebake).

## P2 · Dynamic pool (real-time, gate stubbed all-lit)

- [ ] Restructure the current 32-light live loop into the **dynamic pool**: 32 lights in uniforms
      (`32 × 2 vec4`), structured as **4 channels × 8** to dodge ES-1.00 dynamic indexing; per light
      `falloff·N·L`, gated by `gate(i)` (stub `gate=1`). Keep it visually identical to today, just
      restructured for the bitfield gate P3 adds.

## P3 · Projected-silhouette shadows (the per-light win) — retire the stopgaps

- [ ] **Scatter maps** — 2 RGBA maps (8 lanes). Each frame, for this frame's 8-light batch: gather
      nearby casters, **project each caster's earcut triangulation** through the light onto the ground
      (`h/(lightZ−h)` shear) and rasterize into its lane (`uChannel` uniform output, `max`-blend, one
      container `render`). Port `shadowMaskShader.ts` onto the **new outline sidecars**.
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
