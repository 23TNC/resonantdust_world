# Todo — webgl-engine (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. New component
`client/webgl` (working name — [F1](forks.md#f1)). See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md). The pixijs client stays working throughout ([I-3](issues.md#i-3))._

---

## W1 · Scaffold `client/webgl` + a WebGL2 hello — 2026-07-20

- [ ] New vite + TS project adjacent to `client/pixijs` (same tsconfig shape, dev/build, container-relative
      paths per [repo-layout](../../repo-layout.md)). Own the `<canvas>` + `getContext('webgl2', …)` + a
      `requestAnimationFrame` loop; render a cleared canvas → then a single triangle. Proves the context,
      loop, and build are ours end-to-end.

## W2 · The engine core — the renderer that replaces Pixi's backend — 2026-07-20

- [ ] `Program` (raw ES 3.00 compile/link/cache; uniform/attr reflection; sampler binding incl. VERTEX
      stage + `usampler2D` + `uvec4` out), `Geometry` (VAO, per-instance divisors, indexed/instanced draw),
      `Texture` (unorm / `RGBA32F` / `RGBA8UI` / `RGBA32UI`; nearest; atlas frame UVs), `RenderTarget` (FBO,
      MRT, integer + float + depth, `clearColor` **and** `clearBufferuiv`), `Renderer`/state (blend/scissor/
      viewport/bind — owned; [I-1](issues.md#i-1)), camera transform.
- [ ] Unit-prove each with tiny tests (a `/command` or a test page): textured quad, render-to-target, MRT
      (all attachments), integer target write + `usampler2D` read, instancing, **VTF** — the exact things
      that failed under Pixi ([caster-lut I-10/I-11](../caster-lut/issues.md)) must WORK here.

## W3 · Copy the non-Pixi code + adapt the seams — 2026-07-20

- [ ] Copy across: DOM panels + input, atlas/LOD/resolver logic, DSL, sync clock, login, content fetch, the
      WASM client-core seam, `squareMath` + the viewport camera/LOD/shadow math. Adapt the thin Pixi seams
      (`Texture`/`RenderTexture` types → the engine's) — [I-2](issues.md#i-2), [I-6](issues.md#i-6).

## W4 · Port the viewport renderer — 2026-07-20

- [ ] Re-implement `SquareCache` (the bakes + apron + toroidal window + reproject) and the display + shadows
      on the engine core. **Copy the shader GLSL bodies verbatim** (already ES 3.00); rewrite only the
      harness (`Program`) — [I-5](issues.md#i-5). Milestone: the world renders (G-buffer + albedo display,
      zoom/LOD, pan) and `/shadowcast` casts, matching pixijs.

## W5 · Port the UI — 2026-07-20

- [ ] DOM panels copy across (they're DOM). The canvas-hosting panel hosts our `<canvas>`. `Text` → DOM
      overlays; `Graphics` → the engine's line/rect/circle helper or CSS ([F4](forks.md#f4)). Wire the scene
      graph (flat containers of draw items) + the frame loop.

## W6 · Parity sweep + cutover — 2026-07-20

- [ ] Feature-for-feature vs `client/pixijs`: login, world render, zoom/LOD/pan, every panel, `/shadowcast`,
      `/overlayRT`, `/showRT`, `/es300`, all debug commands ([I-4](issues.md#i-4)). When it matches, make
      `client/webgl` the primary client and **retire `client/pixijs`** (archive out-of-repo).

## W7 · The payoff — `caster-lut` C5 on the owned engine — 2026-07-20

- [ ] Implement the GPU cast (instanced, VTF reading light/LUT/caster), the integer bitfield (`RGBA8UI`/
      `RGBA32UI`, real `uint`, retire float-mod), MRT where useful — all clean, no Pixi walls. Resume
      [`caster-lut`](../caster-lut/todo.md) C5 here.
