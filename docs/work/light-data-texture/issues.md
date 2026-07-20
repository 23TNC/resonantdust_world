# Issues — light-data-texture

_Gotchas to respect while building. The mechanism is standard; these are the stack-specific sharp edges._

---

## I-1 · ES 1.00 rules out integer textures — sample as float/unorm via `texture2D`

Pixi's high-shader compiles **GLSL ES 1.00** even on WebGL2 (the `webgl2-es300` finding — `uint` fails to
compile, proven by `bitfield-rt`). So `usampler2D`, `texelFetch`, and any `uint` bit-twiddling are **out**.
The data texture must be sampled with `texture2D` and read as **floats** — either a real `RGBA32F` texture
(full precision) or `RGBA8` unorm decoded `× 255.0`. True `u32` per texel would need a hand-written raw ES
3.00 shader (deferred).

## I-2 · `highp` is mandatory for world-px in the float texture

`mediump float` (~10-bit mantissa) cannot represent world-pixel coordinates (thousands) without stepping.
The sampler and the fragment math that consumes positions must be `highp`. Declare `precision highp float`
in the shader (and a `highp sampler2D` for the data texture). Colour/radius/intensity are small and fine at
any precision, but positions are not — this bit the merge math already and would silently corrupt
texture-sourced positions ([F2](forks.md#f2)/[L5](todo.md)).

## I-3 · Colour must come ONLY from the texture — or the proof is vacuous

The whole point is proving data flows texture→shader. If the display keeps any hardcoded-colour fallback,
a passing screenshot proves nothing. Remove the `LIGHT_COLORS` constant path from the display; the only
colour source is the sampled row 3. The **per-frame randomisation** is the live-ness proof: if the shadow
colours flicker, the texture is genuinely being read each frame.

## I-4 · Guard float-texture support; fall back to u8 on failure

`RGBA32F` sample + `nearest` filter is WebGL2 core, so it should just work here (`preferWebGLVersion: 2`).
But a context quirk or a Pixi format-string mismatch can yield an incomplete texture (samples black) or a
console error. Watch the console on first run; if it fails, switch to the `RGBA8` packing
([F1](forks.md#f1)) rather than fighting the float path.

## I-5 · Texel-centre sampling — `((k + 0.5)/5, (r + 0.5)/5)`

With `nearest` on a 5×5 texture, sample at texel **centres**, not edges, or a rounding wobble reads the
neighbouring light/row. Light `k` row `r` → uv `((k + 0.5) / 5, (r + 0.5) / 5)`.

## I-4 · RESOLVED — float texture worked, no fallback needed — 2026-07-20

`RGBA32F` sample + `nearest` created and sampled cleanly on this WebGL2 context (`preferWebGLVersion: 2`);
no incomplete-texture or format errors in the console. The `RGBA8` packing was not needed. Keep it
documented ([F1](forks.md#f1)) as the fallback if a future context lacks float-texture support.
