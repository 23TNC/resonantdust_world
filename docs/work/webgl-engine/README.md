# Work — webgl-engine (retire PixiJS; own the WebGL2 renderer)

_Opened 2026-07-20. Stand up a **new adjacent client** (`client/webgl`) that re-implements the browser
client on a **bespoke WebGL2 renderer** — no PixiJS. Component: a new `client/webgl` (parallel to
[`client/pixijs`](../../components/client/pixijs/)). This is essentially building the rest of the game
engine ourselves; it won't be easy, but it buys the clean GPU control the shadow/lighting work needs._

## Why — we use a thin slice of Pixi, and it's exactly the slice that blocks us

The `caster-lut` C5 work (GPU cast + integer bitfield) keeps hitting **Pixi-abstraction walls**: raw-program
vertex-texture-fetch doesn't bind ([caster-lut I-10](../caster-lut/issues.md)), integer render targets error
through the mesh pipe ([I-11](../caster-lut/issues.md)), and raw-GL interop corrupts Pixi's state
([D-2](../caster-lut/deviations.md)). Meanwhile an inventory of actual usage shows we use **very little** of
Pixi:

- **Already ours (no Pixi):** the DOM panel framework + DOM input, the SquareCache G-buffer, atlas packing
  (MaxRects/LodPool), the texture resolver, the DSL, the sync clock, login, the WASM client core, and — since
  `es300-migration` — **all the shaders (GLSL ES 3.00)**.
- **What Pixi is for us:** a WebGL render backend (`renderer.render`, the high-shader transform/uniform
  plumbing, `Geometry`/`Texture`/`RenderTarget`) plus three convenience primitives (`Sprite`=a quad,
  `Graphics`=vector shapes, `Text`=labels). And its state/batcher — which we barely lean on and which is
  precisely what breaks raw-GL interop.

So Pixi is a render backend we're outgrowing. Owning a WebGL2 layer gives **first-class integer textures,
MRT, VTF, UBOs, and transform feedback** — no fighting — and a smaller bundle.

## Strategy — new folder, copy the non-Pixi code, rewrite the renderer, reach parity, then the payoff

1. **Scaffold** `client/webgl` (vite + TS), a WebGL2 "hello" via our own context.
2. **Build the engine core** — the ~renderer that replaces Pixi's backend (below).
3. **Copy the non-Pixi code** across (game logic, textures/atlas/LOD, DSL, sync, login, DOM panels, npc,
   content, the WASM seam) — adapting only the thin Pixi seams (`Texture`/`RenderTexture` → our types).
4. **Port the viewport renderer** (SquareCache + G-buffer bakes + display + shadows) onto the engine core.
5. **Port the UI** (DOM panels copy; the canvas-hosting panel hosts our canvas; `Text`→DOM; `Graphics`→our
   shapes/CSS).
6. **Parity + cutover** — match the pixijs client feature-for-feature, then make `client/webgl` primary.
7. **The payoff** — implement `caster-lut` C5 (GPU cast + integer bitfield + MRT + VTF) cleanly on the owned
   engine.

The separate folder is deliberate: the **pixijs client stays alive and working** until the new one reaches
parity, so we never have a broken game during the migration.

## The engine core — designed for ES 3.00 from the start

The bespoke layer (replacing Pixi's backend). **Integer textures / MRT / VTF / UBOs are first-class, not
bolted on** — the whole reason for the exercise:

- **Context + canvas** — `getContext('webgl2', …)` with our attributes; the frame loop (`requestAnimationFrame`).
- **Program** — raw ES 3.00 compile/link/cache; reflected + cached uniform/attribute locations; typed uniform
  setters; sampler→unit binding that works in the **vertex** stage (VTF) and for **`usampler2D`** (integer) +
  **`uvec4`** outputs.
- **Geometry** — VAO abstraction: per-vertex + per-instance attributes (divisors), index buffer, draw
  (arrays/elements, instanced).
- **Texture** — upload (buffer/image/canvas); formats unorm / `RGBA32F` / `RGBA8UI` / `RGBA32UI`; nearest &
  wrap; **atlas frame UVs** (we already compute these — the packing is ours).
- **RenderTarget** — FBO with **N colour attachments (MRT)**, integer or float or depth; float `clearColor`
  **and** integer `clearBufferuiv`; bind/resize.
- **Renderer / State** — explicit draw orchestration + a small tracked-state model (blend/scissor/viewport/
  bind), owned end-to-end so there's **no interop problem** (the raw-GL-in-Pixi corruption vanishes when we
  own the whole loop).
- **Camera / transform** — world→clip (pan/zoom) as uniforms; we already compute this in the viewport.

The three convenience losses resolve cheaply: **`Sprite`→a quad mesh**; **`Text`→DOM overlays** (UI is
already DOM); **`Graphics`→** our own line/rect/circle helper (the only hard tessellation, the shadow-cast
polys, is going to the GPU anyway, and panel chrome can be CSS).

## Copy vs rewrite

| Copy ~verbatim (pure / DOM / logic) | Rewrite on the engine core (Pixi-dependent) |
|---|---|
| DOM panels (`ui/dom/*`, most `game/panels/*`), DOM input | `main.ts` app/renderer setup → our context + loop |
| textures: atlas packing, LOD, resolver logic (frame UVs) | the `Texture`/`RenderTexture` wrapper → our `Texture`/`RenderTarget` |
| DSL, sync clock, login, content fetch, WASM client core seam | SquareCache render path (`Mesh`/`Geometry`/`Shader`/RT) |
| **shader GLSL bodies** (already ES 3.00 — copy the logic) | the shader **harness** (compile/uniform/transform — `es3HighShader`/high-shader → our `Program`) |
| the viewport's camera/LOD math, `squareMath`, shadow math | `Sprite`/`Graphics`/`Text` usages |

## Parity target (the cutover bar)

Login → world renders (G-buffer bakes, albedo display, zoom/LOD, pan) → DOM panels (chat/debug/settings/RT) →
`/shadowcast` casts → every debug command (`/overlayRT`, `/showRT`, `/es300`, `/zoom`, …). Verified against
`client/pixijs` before `client/webgl` becomes primary; then Pixi is retired.

## State

Phased in [`todo.md`](todo.md) (W1 scaffold → W7 the payoff); the naming/scope/cutover calls in
[`forks.md`](forks.md); the state-management / seam / parity risks in [`issues.md`](issues.md). Big, but
bounded — most of the client is already ours; only the render backend + three primitives are rebuilt.
