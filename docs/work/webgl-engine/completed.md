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
