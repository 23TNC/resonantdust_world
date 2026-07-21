# Work — mrt-bakes (collapse the 4 G-buffer bake passes into 1 via MRT)

_Opened 2026-07-20. Follows [`es300-migration`](../es300-migration/README.md) (which is the prerequisite —
multi-output fragments need ES 3.00). Component: [`client/pixijs`](../../components/client/pixijs/). Bakes
the four G-buffer channels (`albedo`/`surface`/`normal`/`zdepth_world`) in **one** render pass per square
instead of four, using **multiple render targets**._

## Why — the bake is the hot path, and it draws each square four times

`SquareCache.bakeSquare` loops `for (const ch of this.channels)` and renders the square's prims **once per
channel** (4 channels = 4 scratch renders + up to 4 blits each). MRT lets a single shader write all four
channel outputs in **one** render of the prims — roughly a 4× cut in bake draw calls on the path that runs
every time a square becomes dirty (pan strips, LOD swaps, mover updates, texture-tier landings).

## What — one MRT bake shader + a 4-attachment target

A single ES 3.00 bake shader with four fragment outputs:

```glsl
layout(location = 0) out vec4 oAlbedo;   // material reconstruction (RGB, opaque)
layout(location = 1) out vec4 oSurface;  // R=presence, G=AO, B=coverage
layout(location = 2) out vec4 oNormal;   // silhouette-keyed tangent-space normal
layout(location = 3) out vec4 oDepth;    // tile depth in B
```

rendered into a `RenderTarget({ colorTextures: [albedoScratch, surfaceScratch, normalScratch, depthScratch] })`
(same slot-sized scratch the bake already uses, ×4), then each attachment blitted to its channel's slot +
apron (the existing blit/apron logic, unchanged). One `gl.drawBuffers` write hits the same `(x,y)` in all
four attachments, so the four channel buffers stay in lockstep — which they already are (identical
`fixedCW×fixedCH`).

## Key findings (both gating unknowns already resolved)

- **MRT is Pixi-native** — `RenderTarget.colorTextures: TextureSource[]`, and `GlRenderTargetAdaptor`
  attaches each with `gl.framebufferTexture2D(…, COLOR_ATTACHMENT0 + i, …)`. No raw-GL framebuffer
  management ([I-1](issues.md#i-1)).
- **ES 3.00 is the enabler** — `layout(location=i) out` is ES 3.00-only, so this is unreachable before the
  migration. The migration wasn't just cleanup; it unlocked this.

## The real work — one universal material, one fragment

The cost isn't the plumbing, it's consolidating the four per-prim materials
(material/surface/normal/depth — now the single [mrtBakeShader](../../../client/pixijs/src/game/viewport/mrtBakeShader.ts)) into one
fragment that computes all four outputs from one prim draw. The approach ([F2](forks.md#f2)):

- **No tier branch — a "solid material".** Rather than the MRT shader choosing between a material path and a
  flat-tint path, make the flat/geo case a **degenerate material** (white maps + flat-up normal + full
  coverage + `tint = geoColor` + tile depth). The material reconstruction already handles tint and null
  layers, so white × tint reproduces the flat sprite exactly. **Every** prim flows through the one material
  path; the fragment has a single code path. This also collapses the four independent real-tier gates into
  **one** solid-vs-real decision per prim, feeding all four outputs consistently ([F2](forks.md#f2)).
- **Shared silhouette = shared discard.** All four channels key on the same coverage (`surface.B`), so **one
  `discard` governs all four outputs** — a pixel outside the prim isn't part of *any* channel. A clean
  simplification, not a complication ([I-6](issues.md#i-6)).

So there's a **prep step**: extend the per-prim "material" to a superset (albedo residual/layers + surface +
normal + depth + tint) and make every `resolve` return one — the flat cases as solid materials. That unifies
the albedo bake path even before MRT; the MRT fragment then just consumes it.

## Scope — behaviour-preserving

Each channel's composite must stay **pixel-identical** to the current separate bakes ([I-4](issues.md#i-4)),
verified per channel via `/overlayRT albedo-cold|surface-cold|normal-cold|zdepth-world-cold`. This is a
performance refactor of the bake, not a visual change.

## Alignment

Independent of the shadow/bitfield line — this is the second ranked ES 3.00 win (the first being the
integer-texture bitfield in `caster-lut` C5). It touches only `SquareCache`'s bake + the bake shaders.

## State

Phased in [`todo.md`](todo.md); the target/shader-merge calls in [`forks.md`](forks.md); the MRT / sampler /
behaviour gotchas in [`issues.md`](issues.md). B1 (a 4-output spike into a 4-attachment target) proves the
mechanism before the shader merge.
