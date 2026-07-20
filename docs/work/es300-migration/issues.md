# Issues — es300-migration

_Gotchas to respect. The dialect swap is mechanical; the risk is all in replacing what the high-shader bits
gave for free, without changing a pixel._

---

## I-1 · The transform plumbing is THE risk — match Pixi's global uniform block

The high-shader's `localUniformBit` + global bits fed every vertex `uProjectionMatrix`,
`uWorldTransformMatrix`, roundPixels, etc. A raw ES 3.00 vertex must declare and consume the same, or
world-aligned meshes (bakes, the world-buffer merge, the display) render in the wrong place. Pixi v8's
WebGL2 path binds these as a **`globalUniforms` UBO** (`layout(std140) uniform globalUniforms { … }`) plus
a per-object transform. **Nail the exact block layout/name Pixi binds** (read Pixi's source / log the bound
uniforms) — the shared builder ([M1](todo.md)) encapsulates it once. `es300-hello` dodged this with a
clip-space passthrough; the bakes cannot.

## I-2 · roundPixels + resolution

`roundPixelsBit` snapped vertices to the pixel grid; some bakes/displays depend on it for crisp,
seam-free slots. The builder must replicate it (or prove a given shader doesn't need it — the composite is
`nearest`, so snapping may be moot for the bakes but matters for screen displays). Carry `uResolution`.

## I-3 · Texture binding conventions

`textureBit` set up `uTexture`/`uSampler`/`uTextureMatrix`. Raw ES 3.00 samples with `texture(sampler,
uv)` / `texelFetch(sampler, ivec2, 0)`; confirm Pixi binds our named sampler resources (the `Shader`
`resources` keys → sampler uniforms) the same way. `texelFetch` needs integer coords + no `uTextureMatrix`
remap — good for the data textures, but world composites still sample by uv.

## I-4 · ES 1.00 and ES 3.00 coexist → migrate incrementally

Proven in `es300-hello`: each program declares its own `#version` and a WebGL2 context runs both. So the
migration is safe **one shader at a time** — a half-migrated client (some ES1, some ES3) renders fine. Keep
the tree green and screenshot-verify after each shader; never a big-bang.

## I-5 · Behaviour-preserving — pixel-identical, verified per shader

This migration must not change output. After each shader: compare against the pre-migration render
(`/overlayRT <channel>`, `/shadowcast`, the albedo display). The float-mod → `uint` conversion must be
*exactly* equivalent on unorm8: `mod(floor(v*255/2^i),2)>0.5` ⇔ `(uint(v*255+0.5) >> i) & 1u == 1u`. Watch
rounding (`+0.5` before the `uint` cast).

## I-6 · The decode filter needs an ES 3.00 *vertex* too

A filter links a vertex + fragment; you can't pair Pixi's ES 1.00 default filter vertex with an ES 3.00
fragment ([es300-hello I-2](../es300-hello/issues.md)). The migrated `makeShadowDecodeFilter` supplies its
own `#version 300 es` vertex (a filter-space passthrough), not just the fragment.

## I-7 · Texture formats stay unorm RGBA8 this migration — A still = 1

The integer-texture switch (`RGBA8UI`/`RGBA32UI`, and the 32/128-bit ceiling lift) is the NEXT step
(`caster-lut` C5), because it disables fixed-function blend and forces the shader cast. Here the shadow RTs
stay unorm RGBA8, bits still in the RED byte, **A held at 1** — `uint` bit ops read the unorm bytes, they
don't change storage. Don't conflate the two.
