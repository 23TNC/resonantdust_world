# Rendering platform — WebGL2 context, GLSL ES 3.00 shaders (design / shape)

The client's rendering floor. **Every shader is GLSL ES 3.00** as of 2026-07-20 (the `es300-migration`
work). This doc records the current standard; the history (an ES 1.00 phase, then the migration) lives in
git and the work folders.

## What holds

- **Context: WebGL2.** Pixi v8 defaults `preferWebGLVersion: 2`; `main.ts` pins it. Universal in 2026.
- **Shader dialect: GLSL ES 3.00, one standard.** Every program compiles through
  **`compileHighShaderGlProgramES300`** (`game/viewport/es3HighShader.ts`) — a thin wrapper that hands Pixi's
  own `compileHighShaderGlProgram` an extra bit planting `#version 300 es` in the fragment. Pixi's `GlProgram`
  detects that, drops the WebGL1 shims, and re-inserts the directive as line 1. Pixi's high-shader templates
  are already version-agnostic (`in`/`out`/`texture`/`finalColor`), so its vertex / global-uniform /
  round-pixel plumbing rides along **unchanged** — the migration was a per-shader one-line swap, no
  hand-rolled builder. `uint`, bitwise operators, `texelFetch`, integer textures and MRT are all available.
- **Raw filters too:** `GlProgram.from({ vertex, fragment })` with `#version 300 es` in the fragment goes
  ES 3.00 (Pixi's `defaultFilterVert` is `in`/`out` style, so it upgrades cleanly).

## Bit ops — still float-mod for now (behaviour-identical)

The shadow/overlay bitfield math is still `mod(floor(byte*255.0 / exp2(b)), 2.0)` on the unorm8 bytes. That
was kept as-is through the migration (behaviour-preserving); real `uint`/bitwise is now *available* and is
the natural cleanup when the storage moves to integer textures (see below). Both are exact on unorm8.

## Bitfield storage — unorm RGBA8 today, integer textures next

Bitfields live in a **unorm RGBA8** RT (`nearest`, linear, not sRGB): **24 bits in RGB, A held at 1**.

> **RULE (user, 2026-07-20): the alpha channel is NEVER used for data** — premultiply mangles it. So a
> single unorm8 RT holds **at most 24 separable bits** (RGB); A is opacity/colour only.

This 24-bit ceiling is a property of the *unorm8 + fixed-function-blend* storage, **not** of the shader
dialect (that constraint is gone — see above). The lift is the **integer-texture switch** (`caster-lut` C5):
`RGBA8UI` gives 32 bits (integer textures don't premultiply — the A taboo evaporates) and `RGBA32UI` gives
128; the cost is that integer RTs disable fixed-function blend, forcing the OR into a shader (which the cast
is moving to anyway). Plan around 24 until that lands.

## Gotchas

- A GLSL compile error draws the mesh **BLACK / nothing** with only a `console.error` — silent on the
  surface. Verify a migrated shader by looking at what it renders, not just the absence of a thrown error.
- A backtick inside a `/* glsl */` template literal closes the literal and breaks the build.

## References

- `es300-hello` (proved a raw ES 3.00 program renders) → `es300-migration` (unified all shaders).
- Enables the integer-bitfield lift in [`caster-lut`](../../../../work/caster-lut/README.md) C5 and an MRT
  bake-collapse.
