# Completed — webgl-engine

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## W1 · Scaffold `client/webgl` + a WebGL2 hello — 2026-07-20

New vite + TS project at `client/webgl` (working name — [F1](forks.md#f1)), mirroring the pixijs build shape
(same tsconfig, the wasm/`@shared` + `@content` aliases, repo-root fs allow) but on **port 5174** so it runs
alongside `client/pixijs` (5173) during the migration ([I-3](issues.md#i-3)). `src/main.ts` owns the
`<canvas>` + `getContext("webgl2")` + a `requestAnimationFrame` loop and draws a per-vertex-coloured triangle
via hand-written ES 3.00 shaders — **no PixiJS**. Verified in-browser: the triangle renders, no client
errors. The context, loop, and build are ours end-to-end.

## W2 · The engine core — Program / Geometry / Texture / RenderTarget / Renderer — 2026-07-20

`client/webgl/src/gl/`: `Program` (raw ES 3.00 compile/link + typed uniform/attr cache), `Geometry` (VAO +
per-instance divisors + integer attrs + indexed/instanced draw), `Texture` (rgba8unorm / rgba32float /
**rgba8uint / rgba32uint**, nearest), `RenderTarget` (FBO, **MRT**, integer + float clears, **always calls
`drawBuffers`**), `Renderer` (context + blend/viewport state + the draw primitive). A W2 test scene
(`main.ts`) exercises all six techniques the shadow work needs — **the exact set that failed under Pixi**:
textured quad, render-to-target, MRT (2 attachments), integer target (`RGBA8UI` write + `usampler2D` uint
read), instancing, and vertex-texture-fetch. Verified in-browser: the checker, the two MRT ramps, and the
five VTF-positioned colour-quads all render, **zero engine errors** — no `GL_INVALID_OPERATION`, no state
corruption. Everything Pixi couldn't do works cleanly on the owned engine.
