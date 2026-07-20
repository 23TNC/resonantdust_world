# Rendering platform — WebGL2 / GLSL ES 3.00 (design / shape)

The client's rendering floor, decided 2026-07-20. **The pixijs client requires WebGL2 and authors its
shaders as GLSL ES 3.00.** This is a deliberate stance, not an accident of defaults — record it here so
new shaders are written to it and no one re-litigates a WebGL1 fallback.

## The decision

- **Context:** WebGL2. Pixi v8 already defaults `preferWebGLVersion: 2`, and `main.ts` forces
  `preference: "webgl"` (over WebGPU) — so the app runs on a WebGL2 context wherever one exists. We make
  it **required**: the renderer init should pin WebGL2 (don't silently fall back to WebGL1, which would
  break the ES 3.00 shaders). In 2026 WebGL2 is effectively universal, so this drops nothing real.
- **Shader dialect:** GLSL **ES 3.00**. Pixi's high-shader programs default to the ES 1.00 dialect and
  treat `#version 300 es` in the source as the opt-in (`isES300 = fragment.indexOf("#version 300 es")`).
  **New client shaders declare `#version 300 es`.** Existing ES-1.00-dialect shaders (the G-buffer bakes,
  the albedo blit) keep working and can migrate opportunistically — the stance is forward-looking, not a
  mandate to rewrite what already runs.

## Why — what ES 3.00 buys (and why the old ES 1.00 default was only inertia)

The prior "ES 1.00" was never a platform requirement — it was Pixi's authoring default plus a now-moot
wish to preserve a WebGL1 fallback. ES 3.00 unlocks capabilities the renderer (especially the shadow
bitfield work) genuinely wants:

- **Real integer + bitwise ops** (`uint`, `& | ^ ~ << >>`) — no more emulating bits with
  `mod(floor(v*255/2^i),2.0)` on values that must stay exactly `n/255`. Removes a standing footgun.
- **`texelFetch`** — exact, unfiltered texel reads by integer coord (ideal for reading a bitfield).
- **Integer textures** (`RGBA8UI` / `usampler2D`) — store a bitfield as actual integers. Not blendable
  and nearest-only, which is exactly what a bitfield wants; the pack is an explicit read-modify-write
  (`texelFetch` prev → OR bits → write `uvec4`), no blending. **Consequence: no premultiply applies to an
  integer target, so all 4 bytes — A included — are usable data (32 bits/texel), not 24.**
- **MRT** (multiple `out` targets, core in WebGL2) — write several render targets in one pass; the path
  to storing many separable lights (e.g. 4 targets × 32 bits = 128) without multi-pass gymnastics.

## Gotchas that survive the version bump

- **Integer render targets don't support fixed-function blending.** So a target that needs `max`/`add`
  blend (e.g. a scatter/union pass across primitives) stays a **float RGBA8**; only read-modify-write
  bitfield targets become integer. (For the shadow work: `shadow-hot` stays float+blend, `shadow-cold`
  can be integer.)
- **The backtick-in-GLSL-comment footgun is unchanged** — a backtick inside a `/* glsl */` template
  literal still closes the literal and breaks the build, ES 3.00 or not.
- **A GLSL compile error still draws BLACK** with only a `console.error` — same silent failure mode.

## References

- Applied first in [`work/shadows/`](../../../../work/shadows/README.md) (F14) — the shadow bitfield is
  the first ES-3.00 shader set.
- The tiered-lighting engine ([`intent/tiered-lighting.md`](../intent/tiered-lighting.md)) assumes this
  floor; its bit-extract notes are ES 3.00 now.
