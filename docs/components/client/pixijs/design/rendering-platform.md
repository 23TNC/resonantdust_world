# Rendering platform — WebGL2 context, GLSL ES 1.00 shaders (design / shape)

The client's rendering floor. Decided 2026-07-20, then **corrected the same day** when the `bitfield-rt`
experiment disproved the initial premise — read the correction note before planning shader work.

## What actually holds

- **Context: WebGL2.** Pixi v8 defaults `preferWebGLVersion: 2`; `main.ts` pins it. WebGL2 is universal
  in 2026, so this is a safe baseline — but see the caveat: a WebGL2 *context* does **not** make our
  shaders ES 3.00.
- **Shader dialect: GLSL ES 1.00.** Pixi v8's high-shader system (`compileHighShaderGlProgram` + the stock
  `localUniformBit` / `textureBit` / `roundPixelsBit`) compiles to **ES 1.00 with WebGL1-compat shims**
  (`#define in varying`, `#define finalColor gl_FragColor`, `precision mediump float;`, **no
  `#version 300 es`**) — *even on a WebGL2 context*. So through the bit system there is **no `uint`, no
  bitwise operators, no integer textures, no MRT**.
- **Bit ops go through float math.** `rgba8` stores `n/255` exactly, so a bit is tested with
  `mod(floor(byte*255.0 / exp2(b)), 2.0)` (exact for bits 0–7). This is what the `bitfield-rt` experiment
  proved works end-to-end for a 24-bit RGB bitfield.

## The correction (why this doc first said ES 3.00)

The initial version claimed the client "already runs ES 3.00" because Pixi's GL **template** uses `in`/
`out`/`finalColor`. That was wrong: Pixi **shims** those back to ES 1.00 (`#define in varying`, etc.) at
compile time unless the shader source literally contains `#version 300 es`. Executing `bitfield-rt` E3
settled it — a `uint` in an overlay bit compiled to *"'uint' : undeclared identifier"*
([bitfield-rt I-8](../../../../work/bitfield-rt/issues.md#i-8)). **ES 3.00 is not free.**

## Reaching ES 3.00 (deferred)

ES 3.00 is only reachable by hand-writing a **raw `GlProgram` with `#version 300 es`**, bypassing the
stock ES-1.00-shimmed bits — which means reimplementing the projection / local-uniform / round-pixel
plumbing those bits provide. That's a **real cost**, taken on only when a concrete need justifies it:

- **MRT** (multiple render targets) — the many-separable-lights scale.
- **Integer textures** (`RGBA8UI` / `usampler2D`) or **>32-bit** fields.

Until such a need lands, **stay on ES 1.00 + float-mod**. It is proven and sufficient for the bitfield
work at ≤32 bits.

## Bitfield storage consequence (for the shadow work)

Store bitfields in a **unorm RGBA8** RT (`nearest` — the global `TextureStyle.defaultOptions` — linear,
not sRGB):

- **24 bits** in RGB with **A held at 1** → premultiply is a no-op, exact round-trip, **no special write
  needed** (proven by `bitfield-rt`).
- **32 bits** (reclaim A) → write with the **verbatim, non-premultiply Mesh blit** (the nuked build's
  "linchpin") so A survives as data. This is ES-1.00-compatible — *not* an integer texture.

## Gotchas

- A GLSL compile error draws the mesh **BLACK / nothing** with only a `console.error` — the failure is
  silent on the surface. (This is how the ES-3.00 mistake was caught.)
- A backtick inside a `/* glsl */` template literal closes the literal and breaks the build.

## References

- Proven in [`work/bitfield-rt/`](../../../../work/bitfield-rt/README.md) (E1–E4 + the I-8 finding).
- Consumed by the shadow bitfield work in [`work/shadows/`](../../../../work/shadows/README.md).
