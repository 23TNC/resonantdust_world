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
