# Rendering platform — WebGL2, GLSL ES 3.00, the bespoke `gl/` engine

The client's rendering floor. The webgl client runs a **bespoke engine**
(`client/webgl/src/gl/` — renderer, programs, textures, framebuffers), not a framework. The
pixijs client and its Pixi-v8 high-shader wrapper (`compileHighShaderGlProgramES300`) are
deleted (2026-07-28); that history lives in git and the `es300-hello`/`es300-migration` work
folders.

## What holds

- **Context: WebGL2**, universal in 2026. No WebGL1 fallback, no framework shims.
- **Shader dialect: GLSL ES 3.00, one standard** — every shader is authored `#version 300 es`
  directly. `uint`/bitwise ops, `texelFetch`, integer textures, and MRT are in-bounds and in
  live use (RGBA32UI shadow RTs, R32UI receiver maps, MRT G-buffer bakes) — the old unorm8
  float-mod bitfields and their 24-bit ceiling are history.
- **Formats** are declared per-channel through `gl/texture.ts` (`TexFormat` — unorm8, 16f,
  32f, integer u32 variants). **NEAREST is the universal rescale rule** for encoded/bitfield
  maps (textile-slot); LINEAR only where data is genuinely continuous (the noise atlas).
- **Blend discipline**: bakes alpha-blend, so an MRT attachment's alpha is its BLEND FACTOR,
  not storage — spare data lanes ride only in attachments whose alpha stays 1. Exact additive
  accumulators use `ONE, ONE` with quantised integer deposits; the decay map fades in place via
  `ZERO, CONSTANT_COLOR`.

## Gotchas (recurring, memory-backed)

- A GLSL compile error renders **black/nothing** with only a `console.error` — verify a shader
  by what it draws, not the absence of a thrown error.
- A backtick anywhere in a `/* glsl */` template literal (even a comment) closes the literal
  and breaks the whole build.
- A body-modified variable in a `for`-condition can miscompile and silently skip iterations —
  constant bounds + `break` inside.
- Never hardcode an atlas/page size in a shader — the bound page switches with zoom/LOD; use
  `textureSize()`.

## Why bespoke

The renderer's core surfaces (toroidal slot-grid caches, integer shadow RTs, exact additive
lightmaps, dirty-rect draws, MRT bakes) are all raw-GL shapes; the framework layer added
ceremony without owning any of them. The engine is small, and every abstraction in it exists
because a live consumer (SquareCache, ShadowGather, the blit) needed it.
