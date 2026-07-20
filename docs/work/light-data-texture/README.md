# Work — light-data-texture (feed light data to the shader through a texture)

_Opened 2026-07-20. Builds directly on the verified [`shadow-tiered`](../shadow-tiered/README.md) 4-RT
pipeline. Component: [`client/pixijs`](../../components/client/pixijs/). De-risks the mechanism the
**dense many-lights** design needs — you can't pass hundreds of lights as uniforms, so light data must
live in a **texture** the shader samples._

## Why — uniforms don't scale, textures do

`shadow-tiered` holds its 5 lights in a JS array and hardcodes their colours in the display shader. That
never scales: the durable design ([`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md))
is **many** point lights. The standard answer is a **data texture** — one texel-column per light, sampled
in the shader by light index. This experiment proves that path end-to-end on our stack (Pixi high-shader,
GLSL ES 1.00), with the shadow **coloured from the texture** and the colours **re-rolled once per second** so
a live data flow is unmistakable.

## The texture — 5×2, one column per light

- **5 wide × 2 tall.** Column `x = light index` (0–4). The 2 rows are that light's 2 data pixels.
- `nearest` sampling; sample light `k` row `r` at uv `((k + 0.5) / 5, (r + 0.5) / 2)` (colour = row 1 →
  uv.y `1.5 / 2`).
- Rewritten (re-uploaded) **every frame** from a JS-side `Float32Array` (5·2·4 = 40 values).

### Field layout (per light column, 2 rows × RGBA)

| row | R | G | B | A |
|-----|-----|-----|-----|-----|
| 0 | world_x | world_y | world_z | radius |
| 1 | red | green | blue | alpha |

Because each channel is a full `f32`, position goes straight in as world-px (no region→zone→tile→anchor
split needed — that hierarchy only exists to fit world coords into u8 bytes, see the u8 fallback below) and
**intensity is folded into the RGBA colour** (its magnitude carries brightness). That collapses the old
5-row layout to **2 rows**, halving the texture to 5×2. The **u8 fallback** ([F1](forks.md#f1)) still needs
the wider hierarchical packing.

## Format — float32 if we can, u8 if we must ([F1](forks.md#f1))

- **True `u32` (integer texture, `usampler2D` + `texelFetch`)** needs GLSL **ES 3.00**. Pixi's high-shader
  compiles **ES 1.00** (`uint` fails — the `webgl2-es300` finding, [I-1](issues.md#i-1)). **Ruled out.**
- **`RGBA32F` float texture (recommended)** — 4× full-precision `f32` per pixel, sampled via `texture2D`
  in ES 1.00 (WebGL2 core; `nearest` needs no float-linear extension). This is the practical form of "use
  u32s": full precision, store real world-px / radius / intensity / colour with **no packing or
  quantization**. Needs `precision highp float` ([I-2](issues.md#i-2)).
- **`RGBA8` unorm (fallback)** — u8 can't hold world-px in one channel, so it needs the wider
  region→zone→tile→anchor hierarchical packing (the pre-compaction 5-row layout); decode
  `v = texel.c * 255.0`. Only if the float texture misbehaves on this context ([I-4](issues.md#i-4)).

## What consumes it

1. **Core proof — colour.** The display shader, for each shadow bit it decodes, samples that light's
   colour (row 3) from the data texture and tints the shadow with it — **replacing the hardcoded
   `LIGHT_COLORS`**. JS re-rolls all 5 lights' colours **once per second**, rewrites the texture. Shadows
   must then change colour on that cadence (steady between rolls), independently per light ⇒ the
   texture→shader path is live ([I-3](issues.md#i-3): colour must come *only* from the texture, or the
   proof is vacuous).
2. **Stretch — position ([F2](forks.md#f2)).** Have the cast (or the debug markers) read a light's
   position/radius from the texture instead of the JS `lights[]` array, unifying the source of truth.
   Bigger change (the cast is JS `Graphics` today), so it's a stretch, not the gate.

## Alignment with the durable design

This is the data-plumbing half of `tiered-lighting`'s many-lights model: the shader consumes an arbitrary
count of lights from a texture rather than a fixed uniform block. If it proves out, the shadow engine's
lights graduate from a JS array + hardcoded colours to a per-light data texture that scales to the real
count, and the shadow colour becomes a genuine per-light property.

## State

Phased in [`todo.md`](todo.md); the format/scope calls in [`forks.md`](forks.md); the ES-1.00 / precision
gotchas in [`issues.md`](issues.md). Extends `shadow-tiered`'s `shadowCast.ts` + `shadowCastShaders.ts`;
does not touch the working 4-RT shadow logic beyond swapping the colour source.
