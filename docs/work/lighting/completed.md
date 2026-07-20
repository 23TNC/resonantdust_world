# Completed — lighting

_Done + verified. Items move here from [`todo.md`](todo.md). Append-only history; authoritative for
what's actually shipped. Commit + verification per row._

---

- **2026-07-18** · **P0 — `LightRig` tier routing** (`1268ed1`). `PointLight` carries `tier`
  (`cold`|`dynamic`); `LightRig` routes each — `coldLights()`/`packCold()` feed the bake, `shadowCasters()`
  the (interim) shadow path, `pack()` the display pool. The `{x,y,z,radius,color,brightness,castsShadow,tier}`
  struct is the unified light. Verified: cold lights bake, dynamic lights display.

- **2026-07-18** · **B1 — outline serve + consume** (`1f3e7ff` edge, `7e68eb9` client). Edge serves the
  per-leaf silhouette on demand: `GET /textures/meta/{stem}` → `meta.json` (`serve_meta`). Client
  `OutlineCache.get(stem)` fetches + decodes the earcut `Outline` (polygons + tris), `null` until it lands.
  Verified: `…/conifer/…/e` → the outline JSON; P3 scatter + P4 casters both consume it.

- **2026-07-18** · **P1 — `cold_lightmap` bake, live** (`9ad4a38` shader, `ec6fb8e` integration,
  `098da6a` `/showRT`). A derived `SquareCache` composite ([F6](forks.md#f6)): `bakeLightmapSquare` samples
  each square's baked NORMAL slot + sums the cold lights (`ambient + Σ cold·brightness·max(N·L,0)·falloff²`),
  world-space, on the toroidal/dirty/apron machinery. `LightingBakeShader` + `LightRig.packCold` +
  `enableLightBake`; display adds `lightmap-cold`. **Verified live** — `?ambient=0.3&coldlight` renders a
  baked amortized pool, console clean; `lightmap-cold` visible in `/showRT`.

- **2026-07-19** · **P3 — scatter shader + geometry (foundation)** (`37f6cc8`). `scatterShader.ts`: the
  per-light lane shader (`outColor = uChannel`, `max`-blend), `makeShadowMaskShader`, `makeShadowGeometry`,
  `channelForLight`, `SHADOW_MAPS`/caps/height-falloff constants. `projectCaster.ts` (`ea9a26f`): the shared
  billboard-silhouette shear (cold + dynamic). Foundation only — the `ScatterPass` that drives it per-frame
  is still in [`todo.md`](todo.md) P3.

- **2026-07-19** · **P2 core — cold shadow 3→32-bit bitfield build** (`a40bb9e` blocks, `14f4b0b` build).
  `shadow-cold` is now a **32-bit occlusion bitfield** (up to 32 casters/rect), not RGB=3. Ported
  `warmCombineShader` (ping-pong bit writeback) + `coldLightTex`; `SquareCache.bakeColdShadowSquare`
  rebuilt as a 4-batch scatter→combine (8 lanes/batch via `MAX_SHADOW_LIGHTS`) with a square-level
  light-reach cull + `nBatches`-by-count; `lightingBakeShader` reads `bf_bit`; `LightRig` cap 3→32.
  **Verified working** via a diagnostic (nLights 1, prims 323, 8190 verts built + bits set — the
  "empty shadow-cold" scare was a misread: a set bit-0 = value 1/255, ~black in `/showRT`).
  **Follow-ups (todo P2):** wire the per-rect `coldLightTex` (bake still uses `uLightData[32]` uniforms);
  `projectCaster` hits the 8192-vert cap (~16 conifers) → excess casters dropped, needs a bigger buffer
  or the decimated shadow silhouette; visual shadow-wedge confirm + a >3-light test.

- **2026-07-19** · **P1 — scatter blend fix + bitfield helpers** (`a37be74`). Shadow-lane blend
  `add`→`max` (`SquareCache`, coverage clamps at 1). Ported `lighting/bitfield.ts` from the old game:
  `packBitfield`/`bitSetCPU` (CPU oracle), `BITFIELD_GLSL` (`bf_byte`/`bf_bit`, ES-1.00 float-mod),
  `runBitfieldSpike` (dev round-trip check). The shadow storage for `shadow-warm` + `shadow-cold`.
  Typecheck clean. _(P1's alpha-lane throughput verify folds into P3 — only testable once the ScatterPass
  renders into lanes.)_

- **2026-07-19** · **P4 interim — materialized `shadow-cold` cold shadows** (`ea9a26f`, `9a1b045`,
  `583d87d` rename). `SquareCache.bakeColdShadowSquare` projects the ≤3 nearest cold lights' caster
  silhouettes (`projectCaster`) into a derived `shadow-cold` composite (R/G/B lanes); the cold lightmap bake
  samples it and subtracts the shadowed light's term (`1 − shadow·SHADOW_STRENGTH`). `OutlineCache.takeResolved`
  → `invalidateAll` re-bakes as outlines land. Fixed a slot-UV double-offset (`vRawUv`). **Verified live** —
  a debug passthrough matched `lightmap-cold` to `shadow-cold` pixel-for-pixel.
  ⚠ **Interim, capped at 3 lights** — the target is the **32-bit `shadow-cold` bitfield** (32 casters,
  [`todo.md`](todo.md) P2, [F7](forks.md#f7), [D-1](deviations.md)); this RGB=3 path is what it replaces.
