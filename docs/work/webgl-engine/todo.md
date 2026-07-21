# Todo — webgl-engine (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. New component
`client/webgl` (working name — [F1](forks.md#f1)). See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md). The pixijs client stays working throughout ([I-3](issues.md#i-3))._

---

## W1 · Scaffold — DONE (completed.md)

- [ ] New vite + TS project adjacent to `client/pixijs` (same tsconfig shape, dev/build, container-relative
      paths per [repo-layout](../../repo-layout.md)). Own the `<canvas>` + `getContext('webgl2', …)` + a
      `requestAnimationFrame` loop; render a cleared canvas → then a single triangle. Proves the context,
      loop, and build are ours end-to-end.

## W2 · The engine core — DONE (completed.md)

- [ ] `Program` (raw ES 3.00 compile/link/cache; uniform/attr reflection; sampler binding incl. VERTEX
      stage + `usampler2D` + `uvec4` out), `Geometry` (VAO, per-instance divisors, indexed/instanced draw),
      `Texture` (unorm / `RGBA32F` / `RGBA8UI` / `RGBA32UI`; nearest; atlas frame UVs), `RenderTarget` (FBO,
      MRT, integer + float + depth, `clearColor` **and** `clearBufferuiv`), `Renderer`/state (blend/scissor/
      viewport/bind — owned; [I-1](issues.md#i-1)), camera transform.
- [ ] Unit-prove each with tiny tests (a `/command` or a test page): textured quad, render-to-target, MRT
      (all attachments), integer target write + `usampler2D` read, instancing, **VTF** — the exact things
      that failed under Pixi ([caster-lut I-10/I-11](../caster-lut/issues.md)) must WORK here.

## W3 · The shell + non-Pixi substrate + login boot — DONE (completed.md)

Survey ([forks F2](forks.md#f2)): 34/62 files Pixi-free (copy verbatim); 28 touch Pixi. The shell shape is
[F6](forks.md#f6) — DOM-composited, no global scene-graph; viewport self-canvases; chrome → CSS. Ordered:

- [x] **W3a · Shell.** `app/Ticker` (RAF, deltaMS, add/remove) + `app/App` (ticker + resize dispatch +
      SceneManager + mount host; **no global canvas**). Rewrite `Scene` (drop `root: Container`; self-mount
      DOM/canvas in `onEnter`/`onExit`), `SceneManager` (App ticker/resize; no `stage.addChild`),
      `GameContext` (`app: App`). `assets/fonts` keeps the `FontFace` registration, drops `BitmapFont`.
- [x] **W3b · Pure substrate (verbatim).** `client/*` (WasmClient/wasm/environments — the WASM seam),
      `debug/{urlParams,EnvOverlay,index}`, `game/definitions/contentBoot`, `game/lighting/oklab`,
      `game/panels/{panelStrings,titlebar/SettingsMenu,titlebar/syncHistory,titlebar/DebugPanel,chat/*}`,
      `game/viewport/squareMath`, `textures/{lod,textureManifest,previewCache,MaxRectsPacker}` +
      `content/panels/defaults.json`. Fix import paths only.
- [x] **W3c · Panel framework (DOM copy).** Copied `ui/dom/*` (minus `PixiPanel`) + `ui/panels/PanelManager`
      + `pointerInteractions` verbatim; `LayoutNode` is a structural stub ([D-1](deviations.md#d-1)) so
      `PanelManager` type-checks. **Deferred to [W5](#w5):** collapsing the `LayoutNode`/`PixiPanel` canvas
      chrome into CSS on `DomPanel` — no chrome renders until the world-scene panels/cards return, so it moves
      to the UI port ([F6](forks.md#f6)).
- [x] **W3d · Login boot.** `scenes/{Scene,SceneManager,login/LoginScene,login/FormOverlay}` +
      `main.ts` on the shell. `DrawCallCounter` → engine hook (or stub). `TextureResolver` constructed against
      the engine `Renderer` (atlas GPU parts stubbed until [W4](#w4)); login needs no textures. Milestone:
      **login DOM form renders, WASM inits, connect to the gateway works** on `client/webgl` — verified in
      browser. `VideoPanel` (touches `app`) adapts here or stubs.

## W4 · Port the viewport renderer — 2026-07-20

The pixijs render layer (~2,500 lines: `SquareCache` 922, `Viewport` 708, `WorldBridge` 613, the shaders,
`MoverLayer`) maps onto the engine core 1:1 — `RenderTexture`→`RenderTarget`, the 4-attachment MRT scratch →
our `RenderTarget({formats:[×4]})` (drawBuffers built in), `Mesh`/`Shader`→`Program`/`Geometry`+`draw`,
`Matrix`→a `uMat3`. The toroidal window / dirty / reproject / apron logic is pure math (copies ~verbatim);
only the Pixi draw calls + the shader harnesses change. Shader GLSL copies verbatim ([I-5](issues.md#i-5)).
Ordered vertical slices, each verifiable in-browser:

- [x] **W4a · Viewport host + camera.** DONE (completed.md). Real `WorldScene` hosting a `ViewportPanel` = a `DomPanel` body with
      its **own `<canvas>` + `Renderer`** (F6), driven by the ticker. The camera (anchor/zoom,
      `screenToWorld`/`worldToScreen`, scroll-zoom + drag-pan input, the `?grid` debug grid). First render: a
      cleared viewport + the grid, pannable/zoomable. No world content yet.
- [ ] **W4b · Shaders → engine `Program`.** Port `mrtBakeShader` (the merged 4-out bake), `albedoBlitShader`
      (warm-over-cold display), `overlayShader` (`/overlayRT`) — copy GLSL verbatim, rewrite each harness to
      an engine `Program` + typed uniform/texture setters. Copy `material.ts` (Pixi-free) + `noiseAtlas`.
- [ ] **W4c · SquareCache.** Port onto the engine: `Channel` ping-pong = two `RenderTarget`s; the MRT scratch
      = one `RenderTarget({formats:[×4]})`; `bakeSquare` = one MRT `draw` + four apron blits; `reproject`/
      `resize`/`recenter`/`markStale`/`fillDisplay` = the same math over engine draws. The real
      `TextureResolver` + atlas (`LodPool`/`TextureAtlas`/`MaxRectsPacker`) return here, sharing the viewport
      `Renderer`'s GL context (retire the W3 stub, [D-1](deviations.md#d-1)).
- [ ] **W4d · Viewport pipeline + WorldBridge.** Wire the per-frame resize→recenter→bakeDirty→fillDisplay→
      blit into the viewport; port `WorldBridge` (world state → cold prims) + `thingPlacement`. **Milestone:
      the world renders — G-buffer + albedo display, zoom/LOD, pan — matching pixijs.**
- [ ] **W4e · Warm layer + movers.** Port `MoverLayer` + the warm `SquareCache`; pawns bake + composite
      warm-over-cold.
- [ ] **W4f · Shadows + debug.** Port `shadowCast`/`shadowCastShaders` (the bitfield cast) and the debug
      `/commands` (`/showRT`, `/overlayRT`, `/shadowcast`). Milestone: `/shadowcast` casts, matching pixijs.

## W5 · Port the UI — 2026-07-20

- [ ] DOM panels already copied (W3c). The canvas-hosting panel hosts our `<canvas>`. **Collapse the
      `LayoutNode`/`PixiPanel` canvas chrome into CSS on `DomPanel`** (borders/outline/resize-grip via
      `::before`/box-shadow) — deferred here from W3c ([F6](forks.md#f6)); retire the `LayoutNode` stub
      ([D-1](deviations.md#d-1)). `Text` → DOM overlays; `Graphics`/cards → the engine's line/rect/circle
      helper or CSS ([F4](forks.md#f4)).

## W6 · Parity sweep + cutover — 2026-07-20

- [ ] Feature-for-feature vs `client/pixijs`: login, world render, zoom/LOD/pan, every panel, `/shadowcast`,
      `/overlayRT`, `/showRT`, `/es300`, all debug commands ([I-4](issues.md#i-4)). When it matches, make
      `client/webgl` the primary client and **retire `client/pixijs`** (archive out-of-repo).

## W7 · The payoff — `caster-lut` C5 on the owned engine — 2026-07-20

- [ ] Implement the GPU cast (instanced, VTF reading light/LUT/caster), the integer bitfield (`RGBA8UI`/
      `RGBA32UI`, real `uint`, retire float-mod), MRT where useful — all clean, no Pixi walls. Resume
      [`caster-lut`](../caster-lut/todo.md) C5 here.
