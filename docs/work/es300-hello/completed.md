# Completed — es300-hello

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## E1 · Raw ES 3.00 `GlProgram` — Pixi handles it natively — 2026-07-20

`GlProgram.from({ vertex, fragment })` with `#version 300 es` in both stages. **Pixi v8 detects ES 3.00
itself** (`isES300 = fragment.includes("#version 300 es")`): it strips the directive, re-inserts it as line
1 (`insertVersion`), and **skips** the WebGL1 `#define in varying` shims (`addProgramDefines` no-ops for
ES3). So the [I-1](issues.md#i-1) "version must be line 1" risk is handled by Pixi — no manual GL, no
preamble-before-`#version`. `es300Hello.ts` holds the source + program.

## E2 · Full-viewport quad + the ES-3.00-only fragment — 2026-07-20

`Es300Hello`: a clip-space quad `Geometry` (`aPosition` at `[-1,1]`) + a `Mesh` over a `Es300Shader`
(a `Shader` with a no-op `texture` accessor — the mesh pipe wants a `TextureShader`, but the program
samples nothing). Vertex = clip-space passthrough (`gl_Position = vec4(aPosition,0,1)`, camera-independent);
fragment = a cyan checkerboard from `uvec2(gl_FragCoord)/24u`, `(p.x ^ p.y) & 1u`, `out vec4 fragColor`.
Wired into `Viewport.toggleEs300()` (added to `overlayContainer`) + the `/es300` chat command.

## E3 · Verified in-browser — 2026-07-20

`/es300` on: the cyan checkerboard renders full-viewport, the game shows through the transparent gaps
(the ES 3.00 mesh composites into the scene), and **the rest of the ES 1.00 scene still renders** — the two
dialects coexist in the one WebGL2 context ([I-4](issues.md#i-4)). No new compile/link error (a failure
would name `es300-hello` at the current time; only the stale `9:35:54` `viewport-overlay` `uint` errors
remain — those are ES 1.00 *rejecting* `uint`, the perfect negative control for [I-6](issues.md#i-6)).
`/es300` off cleanly removes it. **ES 3.00 is proven live.**

---

**E4 (`texelFetch` / integer sampler) not run** — deferred to [`caster-lut` C5](../caster-lut/todo.md),
where the sampler read is actually needed. The arithmetic/compile/render path (the point of this
experiment) is proven; the sampler path is proved in situ there.
