# Issues — mrt-bakes

_Gotchas to respect. The plumbing is Pixi-native; the work is the shader merge + the bake restructure._

---

## I-1 · MRT is Pixi-native — RESOLVED up front

`RenderTarget({ colorTextures: BindableTexture[] })` + `GlRenderTargetAdaptor` attaching each via
`gl.framebufferTexture2D(…, gl.COLOR_ATTACHMENT0 + i, …)`. No raw-GL framebuffer/`drawBuffers` juggling. The
one thing B1 must pin: **how `renderer.render` targets a multi-attachment `RenderTarget`** (the bake currently
targets a single `RenderTexture` via `renderer.render({ target })`) — confirm the API and that all four
attachments receive their `out`.

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
