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

## W3 · The shell + non-Pixi substrate + login boot — 2026-07-20

The bespoke app shell (`app/App` + `app/Ticker`) replaces Pixi's `Application`/`stage`/`Ticker` — DOM-composited,
**no global canvas, no scene-graph** ([F6](forks.md#f6)). Rewrote `Scene` (dropped `root: Container`),
`SceneManager` (App ticker/resize; scenes self-mount DOM), `GameContext` (`app: App`), `assets/fonts` (kept
`FontFace`, dropped `BitmapFont`). Copied the Pixi-free substrate verbatim — the WASM client-core seam
(`client/*`), content runtime, DOM panel framework (`ui/dom/*` minus `PixiPanel`), taskbars, popups, debug
overlay, chat + titlebar panels, `squareMath`, texture URL/LOD helpers, locale + panel-default JSON, and the
`public/` assets. Five render-layer files are W4 stubs (see [D-1](deviations.md#d-1)). Fixed the import seams
(paths, `@content` tsconfig path). **Verified in-browser on `client/webgl` (5174), zero Pixi, zero console
errors:** the login DOM form renders (DEV badge + ⚙/📊 title-bar tools + both taskbars), `initWasm` +
`loadContent` succeed, and **clicking Login runs the full gateway round-trip → world-server WebSocket →
`claim_or_login` auth → scene handoff** to the (stub) world scene. The entire non-render half of the client
now runs on the owned engine.

## W4a · Viewport host + camera — 2026-07-20

First slice of the render port. The real `WorldScene` now hosts a `ViewportPanel` — a `DomPanel` whose body
holds the viewport's **own `<canvas>` + engine `Renderer`** (F6: the viewport is the one WebGL surface, it
self-canvases; no shared Pixi canvas). `Camera` (lifted from the pixijs `Viewport`: anchor/zoom,
`screenToWorld`/`worldToScreen`, `zoomAt`) drives a per-frame render, and `WorldScene` wires drag-to-pan +
scroll-to-zoom (about the cursor) + the `?grid` overlay, with the URL `x`/`y`/`focus` framing the initial
anchor. Until the G-buffer lands (W4c/W4d) the viewport draws a **procedural world-grid** (tiles/zones/regions,
antialiased in a fullscreen fragment) so the camera is visible + verifiable. **Verified in-browser on
`client/webgl`:** auto-login → world, the grid renders, and pan + zoom both track correctly, zero console
errors. (Hit + fixed the recurring GLSL-backtick-in-template-literal foot-gun in the grid shader.)

## W4b (partial) · The essential shaders → engine Program — 2026-07-20

Ported the two shaders on the critical path to "world renders" from the pixijs high-shader system to
self-contained engine `Program`s (GLSL copied verbatim; only the harness changes): **`albedoBlitShader`**
(the warm-over-cold display blit — a `uProjection` world→clip vertex replaces Pixi's transform/roundPixels
boilerplate) and **`mrtBakeShader`** (the merged 4-output G-buffer bake — a `uModel` unit-quad→slot vertex,
and the four outs declared `layout(location=0..3)` directly, so the `finalColor`-patch hack is gone). Each is
a class holding texture + uniform state with an `apply(program)` + `textures(empty)` pair the SquareCache /
Viewport will drive. Copied the Pixi-free `material.ts`. Both typecheck; runtime-verified when wired in
W4c/W4d. Deferred: `overlayShader` → W4f (with the other debug `/commands`), `noiseAtlas` → W4c (with the
material-bake wiring; the bake reads a null noise atlas as flat until then).

## W4c · SquareCache — the world renders on the engine — 2026-07-20

Ported the SquareCache (toroidal G-buffer) onto the engine core: `Channel` buffers = engine
`RenderTarget`s (rgba8unorm), the MRT scratch = one `RenderTarget({formats:[×4]})`, `bakeSquare` = one
merged MRT draw per prim (placed by a `uModel` mat3) + four apron blits (a small blit `Program`), and the
window / dirty / markStale / fillDisplay logic copied verbatim. Wired the per-frame pipeline into the
Viewport (resize→recenter→bakeDirty→fillDisplay→albedo blit with a world→clip `uProjection`), geo tier only
(every prim a solid `geoColor` box — white fill × tint, no atlas). **Verified in-browser: a 16×16 checker of
solid tiles renders through the full merged MRT bake + toroidal composite + display blit on `client/webgl`,
zero errors.** SIMPLIFIED vs pixijs (deferred follow-ups): single buffer per channel (no ping-pong/reproject
→ a re-bake flash on zoom-LOD), and the real `TextureResolver`/atlas (master→preview textures) — the geo
tier is the floor both start from. Fixed a `uSrc` vertex/fragment type collision in the blit program.

## W4d · Viewport pipeline + WorldBridge — THE WORLD RENDERS — 2026-07-20

Ported `WorldBridge` (613 lines) + `thingPlacement` onto the engine — its only Pixi seam was `type Texture`
(swapped); it streams subscribed-zone cold tiles/things (via the WASM client + `content.zone*Prims`) into the
viewport's SquareCache and drives the client anchor as the camera pans. Added the Viewport's `setAnchor`/
`setMaterialRegistry`/`setNoiseAtlas` + exposed `white`; stubbed `noiseAtlas` (geo tier is flat). Rewired
`WorldScene` to create the bridge and route drag-pan + scroll-zoom THROUGH it (so zone subscriptions follow
the view). **Verified in-browser on `client/webgl`: the real world renders — the gray stone floor, the tan
tile, the green/brown biome terrain, scattered things — matching the pixijs client's terrain byte-for-byte in
layout + colour, correct Y-orientation, zero console errors.** Geo tier (things render as solid `geoColor`
boxes, not sprites yet). Remaining W4 texture work (deferred): the real `TextureResolver`/atlas
(master→preview→geo), so things become textured sprites; plus the ping-pong/reproject smooth-LOD (W4c).

## W4e · Warm layer + movers (wired) — 2026-07-20

Added a second (WARM) `SquareCache` to the Viewport, driven with the identical window/slot geometry as the
cold cache, plus `warmAddPrim`/`warmGetPrim`/`warmRefreshPrim`/`warmRemovePrim`. The per-frame tick bakes warm
first (priority — movers are few) then cold with the remaining budget (floored), and binds the warm
albedo/surface composites into the display blit's warm samplers — so `AlbedoBlitShader` composites warm OVER
cold by warm coverage (the path built in W4b). Ported `MoverLayer` (only Pixi seam: `Texture.EMPTY` → the
viewport's white) and wired it into `WorldScene`. **Verified: typechecks, and the world still renders with the
warm cache active + bound (pure cold where no mover sits).** The mover VISUAL is unverified-live — there are no
pawns in the world right now (the wolves need the npc driver running server-side; the pixijs client showed none
either). The warm pipeline is a faithful port and will render pawns as geo boxes when they exist.

## Input + URL fixes — wheel zoom, cursor readout, focus parsing — 2026-07-21

Three real bugs the port introduced, found while diagnosing the world-render issues, now fixed + verified:
- **Wheel zoom** was a discrete accumulator (only zoomed once `deltaY` summed past a threshold, so most ticks
  did nothing). Restored pixijs's **continuous per-event** zoom (`factor = 2^(-deltaY/500)`).
- **Cursor readout** (the debug HUD's region/zone/tile x,y rows — copied verbatim but never fed): re-wired the
  `ViewportPanel` window-`pointermove` → `debugPanel.setCursorCoords(screenToWorld(...))`, exactly as pixijs did.
- **`?focus=x,y` parsing** read `args[0].split(",")` (→ tileY always 0) but `parseUrl` already splits on
  `[\s,]+`, so it arrives as `args ["x","y"]`; now reads `args[0]`/`args[1]` (matching pixijs). Dropped the
  ad-hoc `?x=`/`?y=` (old syntax). Verified: `focus=100,50` now anchors the window on tile (100,50).

## Chat panel + slash-command console — 2026-07-21

Wired the `ChatPanel` (copied Pixi-free in W3b, unwired) into `WorldScene` + the command registry, so debug
functionality works again. `ctx.panels` (PanelManager) + `ctx.logs` (LogManager) installed; the chat opens on
world entry, and `registerCommand`/`execCommand` back both typed `/commands` and the URL-param replay
(`?focus=x,y`, `?grid`, `?zoom` run once through the same handlers). Added `Viewport.setZoom`/`zoom`. Working
commands: **`/grid [0-3]`**, **`/focus x y`**, **`/zoom level`** (drive the camera/grid live, through the
bridge so subscriptions follow), plus `/pause`/`/unpause` stubs. The RT-preview / overlay / shadow / spike
commands (`/showRT`, `/overlayRT`, `/shadowcast`, `/es300`, `/mrttest`, `/inttest`) are registered but report
their W4f-pending status (they need the render-texture debug infra). **Verified in-browser: chat opens, typed
`/grid 1` prints its response + toggles the grid, and `?focus=100,50` replays as "Camera focused on tile
(100, 50)." — zero console errors.**
