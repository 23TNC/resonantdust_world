# Forks — light-data-texture

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · Texture format — float32 (recommended) vs u8 packing

- **`RGBA32F` float (recommended).** 4× full-precision `f32` per pixel, sampled via `texture2D` in ES 1.00
  (WebGL2 core; `nearest` avoids the float-linear extension). Store real world-px / radius / intensity /
  colour with no packing or quantization — the practical form of the "u32 would be wonderful" wish. Cost:
  needs `precision highp float` ([I-2](issues.md#i-2)) and a float-texture-capable context (WebGL2 gives
  it; guard anyway — [I-4](issues.md#i-4)).
- **`RGBA8` unorm (fallback).** The 16-field hierarchical u8 packing (README table); each channel 0–255,
  decode `v = texel.c * 255.0`. Positions must split region→zone→tile→anchor to fit a byte. Use only if the
  float texture fails to create/sample on this context.
- **True `u32` integer texture — NOT available.** `usampler2D` / `texelFetch` / `uint` need ES 3.00; Pixi's
  high-shader is ES 1.00 ([I-1](issues.md#i-1)). Out of scope until we hand-write a raw ES 3.00 shader.

**Pick:** start float32; fall back to u8 only on a demonstrated failure. _(pending execution)_

## F2 · What reads the texture — colour only (core) vs also position (stretch)

- **Colour only (core).** The display samples row 3 for each light's colour. Minimal, and the per-frame
  colour randomisation makes the live data path unmistakable. This is the gate.
- **Position too (stretch).** The cast / markers read `anchor_*` + `radius` from the texture instead of the
  JS `lights[]`. Unifies the source of truth toward the real engine, but the cast is JS `Graphics` today, so
  it's a larger change. Do it only if cheap ([L5](todo.md)).

**Pick:** colour-only gates the experiment; position is a stretch. _(pending execution)_

## F3 · Per-frame upload — rewrite the whole 5×5 vs partial update

- **Rewrite the whole array + `source.update()`** each frame. 100 floats = 400 bytes; trivially cheap at
  this size. Simplest.
- **Partial / dirty-only upload.** Premature at 5 lights; revisit only if a real light count makes the full
  re-upload measurable.

**Pick:** full rewrite. _(pending execution)_
