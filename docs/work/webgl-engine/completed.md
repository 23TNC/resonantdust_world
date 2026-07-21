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

## Reliable render — content-reload wiring (the intermittent blank world) — 2026-07-21

The primary cause of the intermittent blank viewport was client-side, not the edge race first suspected (see
[I-8](issues.md#i-8) mode B). Boot loads the build-time **embed** corpus; login fires `reloadContent` (an async
fetch of the server's corpus). The W4d `WorldBridge` port had **dropped pixijs's `onContentReloaded →
setContent` wiring**, so a bridge created at scene-enter before the fetch resolved stayed stuck on the embed —
whose biome defs don't cover the server's zones, so `content.zoneTilePrims` returned empty and the delivered
cold rows expanded to ~nothing (instrumented: **52 rows delivered → 3 prims** on a stuck load). And it never
recovered. **Fix:** wire `onContentReloaded(() => { bridge.setContent(c); moverLayer.setContent(c); })` in
`WorldScene` (unsub on exit); `setContent` re-reads stems + **re-expands every stored cold row** through the new
corpus. **Verified in-browser: loads now render reliably — 2962 prims every time, the world fills the viewport.**
This is failure mode **B**; the edge 5s cold-shard `await_ready` timeout (mode **A**, no deliveries at all)
remains open as [W4g](todo.md#w4). Also un-bitrotted `client/npc` to compile against current core (the
`zone_id → macro_position` coord-purge) — it was the core-driver for the login-flow experiment
(`docs/logs/{webgl,pixijs,core}`).

## Frame-cap remainder — maxFPS locks cleanly — 2026-07-21

The `Ticker` frame-cap reset its accumulator to 0 after each dispatch instead of **carrying the sub-interval
remainder** (despite the comment claiming it carried it). On a 120Hz monitor with `maxFPS=60`, `minInterval`
is 16.667ms and two RAF frames (8.333ms) sum right onto that boundary — so pairs that jitter a hair under it
lose their progress and wait a 3rd frame (a 40fps cycle), mixing with 60fps cycles into a reported ~52. Pixi
stayed locked at 60 because it carries the remainder (`_lastFrame = now - delta % _minElapsedMS`). **Fix:**
`this.acc %= this.minIntervalMs` (carry the remainder; `%=` also collapses a post-tab-stall backlog into one
step rather than a catch-up burst). The renderer was never the bottleneck — webgl draws faster than pixijs;
only the cap cadence was off. (Still reads ~56–57 on the box, not a clean 60 — a smaller residual to revisit.)

## W4f (partial) · /overlayRT — G-buffer overlay on the engine — 2026-07-21

Ported the pixijs `overlayShader` to a self-contained engine `Program` (GLSL drop-mode logic verbatim; the
harness is a `uProjection` vertex + a single `uComposite` sampler replacing Pixi's textureBit/localUniform
bits). The Viewport gained the composite API mirroring pixijs — `renderTextures()` (cold+warm albedo/normal/
surface/zdepth), `overlayChannelNames()`/`overlayChannel`/`setOverlay()`/`compositeFor()` — and draws the
overlay over the lit blit each frame in exact register (same display geometry + projection). `/overlayRT`
wired in `WorldScene` (removed from the W4f-pending map). **Verified in-browser:** `/overlayRT surface-cold`
paints opaque cyan aligned to the zone/region grid; `/overlayRT normal-cold` drops the all-flat-up geo
normals (FLAT mode) so the world reads through — the distinct modes confirm the per-channel drop + the
`uMode` upload. `/showRT` + the `/es300`/`/mrttest`/`/inttest` spikes intentionally **not** ported (the
engine techniques they exercised are already proven in W2); `/showRT` deferred (needs a GL→DOM readback).

## W4f (partial) · First lights + billboard shadows (geo tier) — 2026-07-21

The first lights + shadow-casting on the owned engine — basic-functionality cut of the `docs/work/shadows/`
design ([D-2](deviations.md#d-2)). 6 lights seed in a ring around tile (100,50); a `ShadowCaster` casts
billboard-quad shadows off the cold cache's standing prims (things, `zIndex ≥ 1`, culled to a light's reach)
each frame, drawing per-light **coloured** shadows over the world (overlaps combine additively) with light
markers + reach rings. Done **analytically in ONE screen-space fullscreen pass**: per fragment → world pos
(grid-shader mapping) → loop lights × in-range casters → point-in-projected-trapezoid → sum colours. No RTs /
ping-pong / round-robin — recasting each frame is inherently world-stuck + zoom-correct. `/coldlights [tileX
tileY]` (re-seed) + `/shadows` (toggle) wired. **Verified in-browser at `?focus=100,50`:** 6 coloured shadows
fan off the prims, overlaps combine to white, markers + reach rings show; **pan keeps them stuck to the
world** (a brown structure pans in with its own shadows); zero console errors. Cost ~49fps (from ~56 unlit) —
the per-pixel analytic loop; the deferred world-cold persistence is the optimisation that recovers it.
Deferred (D-2): world-space `shadow-cold` (so shadows aren't `/overlayRT`-inspectable yet), per-light bit
packing, round-robin.
