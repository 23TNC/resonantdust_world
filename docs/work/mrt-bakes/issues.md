# Issues — mrt-bakes

_Gotchas to respect. The plumbing is Pixi-native; the work is the shader merge + the bake restructure._

---

## I-1 · MRT works, but needs a MANUAL `gl.drawBuffers` — proven in B1 (2026-07-20)

Half-native: `RenderTarget({ colorTextures })` + `GlRenderTargetAdaptor` **attach** all four textures
(`gl.framebufferTexture2D(…, COLOR_ATTACHMENT0 + i, …)`), and `renderer.render({ target: renderTarget })`
accepts a `RenderTarget` (`RenderSurface = ICanvas | BindableTexture | RenderTarget`). **But Pixi NEVER calls
`gl.drawBuffers` anywhere** (grep-confirmed), and WebGL2 writes only to attachment 0 by default — so
`layout(location=1..3) out` would write nowhere without intervention.

**The fix (B1-proven, `es300MrtSpike.ts`):** after `renderer.renderTarget.bind(target, false)` (which creates
+ binds the FBO), call `gl.drawBuffers([COLOR_ATTACHMENT0, …1, …2, …3])` on `renderer.gl`, then
`renderer.render({ container, target })`. `drawBuffers` is **per-FBO state**, so it persists for that FBO
across re-binds and doesn't pollute the screen FBO. The B4 bake must do this once the scratch FBO exists (and
re-apply after any resize that recreates the FBO). Verified: four `out`s → four distinct attachment colours,
scene unaffected.

## I-2 · ES 3.00 is the prerequisite

`layout(location=i) out vec4` is ES 3.00-only. This work is only possible because `es300-migration` unified
the shaders. The MRT bake shader compiles through `compileHighShaderGlProgramES300` (or a raw ES 3.00
program if the high-shader templating fights four outputs — its fragment template hard-codes a single
`finalColor` out, so a **raw program may be cleaner here** — decide in B2).

## I-3 · All four attachments share dims + slot/apron layout

The four channel buffers are already identical `fixedCW×fixedCH`. MRT writes the same `(x,y)` to every
attachment, so a single slot render fills all four channels' slots coherently. The scratch is likewise one
slot-sized set of four attachments.

## I-4 · Behaviour-preserving — pixel-identical per channel

Each channel's output must match the current separate bake exactly. Verify all four via `/overlayRT
*-cold`, not just the albedo display. The material/surface/normal/depth formulas move verbatim into the one
fragment — don't "improve" them mid-move.

## I-5 · Sampler-unit budget

The merged fragment binds every input all four channels need at once: `albedo`(residual) + `layers` +
`surface` source + `normal` map + the noise atlas + (material) params. That's ~5–6 samplers — well under
`MAX_TEXTURE_IMAGE_UNITS` (≥ 16 on WebGL2), but count them; the four separate bakes each bound only their
own, so this is the first time they coexist.

## I-6 · One `discard` governs all four outputs — a feature, not a bug

All four channels key on the same silhouette (`surface.B` coverage), so a fragment outside the prim is
`discard`ed for *every* channel at once (it belongs to none). That's correct and simpler than four separate
coverage tests. But note: the surface bake writes `presence` where covered and the depth/normal key on it —
so compute coverage first, `discard` if zero, then all four outputs write.

## I-7 · The flat-tint (geo) path is the "solid material", not a branch

Geo-tier / untextured prims bake today as a flat tinted sprite. Under the [F2](forks.md#f2) approach they're
a **solid material** (white maps + flat-up + full coverage + `geoColor` tint + tile depth), so the one
material path produces all four channels for them — no branch. Verify the flat cases specifically (ground
tiles, geo-tier things, untextured rects) bake **identically** through the material path: `white × tint`
must equal the old flat sprite, flat-up normal, opaque surface (`0x00ffff`), and the right tile depth. This
is why B2 (universal material) is verifiable on its own, before any MRT.

## I-8 · The merged fragment's subtleties (scoped from the 4 shaders, 2026-07-20)

Reading material/surface/normal/depth to design the one 4-out fragment surfaced what B3 must get right for
**pixel-identical** output:

- **One shared `discard` on `cov = surface.B < 0.5`** works for all four — because in the [F2](forks.md#f2)
  solid-material model, ground/flat prims carry a **white surface** (`cov = 1`), so they never discard and
  fill their whole tile in every channel; real things carry real coverage and discard outside the
  silhouette (ground behind survives in all four). No per-channel coverage test.
- **`oSurface = vec4(1.0, s.g, s.b, 1.0)`** (presence=1, ao=surface.G, coverage=surface.B).
- **`oNormal`** = normal-map sample, or flat-up `vec3(0.5,0.5,1.0)` when the stem has none.
- **`oDepth = vec4(0,0, tileDepth<0 ? 0.0 : tileDepth, 1.0)`** — ground writes black, things write depth. In
  the solid model, ground's `cov=1` so the shared discard never fires on it; the `<0` branch just picks
  black.
- **Albedo tint is the trap.** The material path applies tint via the per-material `uChA` channels and does
  **NOT** multiply `prim.tint`; the flat sprite path applied `prim.tint`/`geoColor`. So the merged shader
  needs a `uTint` multiply on `oAlbedo` = the resolved tint (**white** for real materials → no-op; `geoColor`
  for solid) to preserve both. Verify real things are white-tinted so the no-op holds.

These are why B3 is careful work, and why each channel must be diffed via `/overlayRT *-cold`, not just the
albedo display.

## I-9 · B3 shader approach — high-shader template vs raw program (2026-07-20)

The merged fragment needs FOUR outputs, but Pixi's high-shader fragment template hard-codes a single
`out vec4 finalColor` (location 0) and `finalColor = outColor * vColor`. Two ways:
- **High-shader + extra outs (try first):** `compileHighShaderGlProgramES300` (keeps Pixi's transform/uniform
  plumbing that B2 already uses) and declare `layout(location=1..3) out vec4 oSurface/oNormal/oDepth;` in the
  bit header, writing them in `{{main}}`; `finalColor` stays location 0 = oAlbedo. Risk: mixing one implicit
  location (finalColor) with explicit 1..3 — most WebGL2 drivers assign finalColor→0, but confirm it links.
- **Raw ES 3.00 program:** full control of `layout(location=0..3)`, but must re-declare the vertex transform
  (uProjectionMatrix·uWorldTransformMatrix·uTransformMatrix) AND confirm Pixi feeds `uTransformMatrix` (the
  mesh's local transform) to a raw program — unverified (es300MrtSpike used a clip-space vertex, no transform).

**Plan:** try the high-shader+extra-outs first (reuses the proven B2 plumbing); fall back to raw only if the
output locations won't link. Also: gather the merged inputs from the albedo material (residual/layers/surface
+ tint — surface doubles as coverage AND the surface output) + the normal resolve (normal tex + hasNormal) +
the depth resolve (tileDepth); the surface *resolve* isn't needed separately (white → (1,1,1) → same output).
