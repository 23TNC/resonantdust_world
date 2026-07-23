# Work — 2026-07-21-showrt (`/showRT` render-texture preview on the bespoke engine)

_Opened 2026-07-21. Port the `/showRT` debug panel — a live grid of miniature previews of the
viewport's G-buffer render textures — from `client/pixijs` onto `client/webgl`. It is the **one
remaining W4f debug command** ([`webgl-engine`](../webgl-engine/README.md) descoped it: "needs a
GL→DOM readback") and sits on the parity/cutover bar. Component: `client/webgl`._

## What `/showRT` is

A dev panel showing miniature **live previews of the viewport's render textures** — the whole
G-buffer for both tiers (`albedo` / `normal` / `surface` / `zdepth-world`, each × `cold` + `warm`),
each tile labelled with its name, pixel dimensions, and GPU byte size, with a running memory total in
the title bar. A channel that isn't produced yet draws an empty placeholder, so the panel doubles as a
checklist of what's live. Opened / re-focused by the `/showRT` chat command against the world
viewport; a singleton keyed `showRT` in the `PanelManager`.

It is the read-only companion to `/overlayRT` (already ported): `/overlayRT` draws **one** channel
*over* the lit world; `/showRT` shows **all** channels side-by-side in a panel. Together they're the
G-buffer debug surface.

## Current state — what already exists on the engine

- **The data source is done.** `client/webgl` `Viewport.renderTextures()`
  ([`Viewport.ts`](../../../client/webgl/src/game/viewport/Viewport.ts)) already returns the eight
  `{ name, texture: Texture | null }` channels (cold+warm × albedo/normal/surface/zdepth-world),
  and `aspect()` exists. `/overlayRT` consumes the same list and is wired + verified.
- **The command is a discoverable stub.** `WorldScene.PENDING_W4F` registers `showRT` →
  _"The render-texture preview (/showRT) isn't ported yet — lands with the RT debug panel (W4f)."_
  This stream **removes `showRT` from that map** and registers the real handler.
- **The reference implementation is complete** in `client/pixijs`: the panel
  [`RtPanel.ts`](../../../client/pixijs/src/game/panels/rt/RtPanel.ts), the command wiring
  `WorldScene.showRenderTextures()` → `panels.ensure("showRT", …)`, and the source shape
  `RtChannel`/`RtView`. The **layout math is portable** (2-up tiles, aspect-matched height, byte
  labels, memory total). What is **not** portable is the shell + the preview transport (below).

## The crux — why this was deferred: GL textures don't reach a DOM panel for free

In `client/pixijs` the preview was **free**: the panel was a Pixi panel and each tile a Pixi `Sprite`
pointing straight at the source `RenderTexture` in the **same** Pixi context — it updated every frame
with no pixel readback.

`client/webgl` has neither half of that. Panels are **DOM** (`DomPanel`, an HTML `<canvas>`/`<div>`
body), and the viewport **self-canvases** ([`ViewportPanel`](../../../client/webgl/src/game/viewport/ViewportPanel.ts)):
its composites are `Texture`s living in the viewport's **own** WebGL2 context. A WebGL2 context is
bound to one canvas and its textures don't cross into another context, so a second panel canvas can't
sample them directly. **Moving a composite into the panel requires a readback** (`gl.readPixels`) out
of the viewport context — the exact cost pixijs never paid. This is the whole reason W4f deferred it,
and it's the one real design decision here (see [F1](forks.md#f1)).

## Approach (recommended — resolved in [forks.md](forks.md))

1. **Composite the requested channels into one small grid RenderTarget in the viewport's context.**
   The viewport already owns a [`Blitter`](../../../client/webgl/src/gl/blitter.ts)
   (`blit(target, src, dx,dy,dw,dh)`) and `RenderTarget`; a `renderRtPreview(layout)` helper blits
   each live channel into its tile rect of a panel-sized `RGBA8` target (empty tiles cleared). Bitfield
   channels (`shadow-cold`, later) go through a decode program rather than a verbatim blit — deferred.
2. **One readback per painted frame.** `gl.readPixels` the whole grid target **once** into a
   `Uint8Array` → `ImageData`, not one readback per channel — fewest pipeline stalls ([F1](forks.md#f1)).
3. **Paint it into a single DOM `<canvas>` in the panel body**, y-flipped (GL origin is bottom-left).
   Labels draw into the same canvas (or a DOM overlay). The panel body is `overflow:auto`, so **native
   DOM scroll replaces all of pixijs `RtPanel`'s hand-rolled wheel/thumb/drag code** — a real
   simplification, not a port ([F2](forks.md#f2)).
4. **Throttle.** Readback is a GPU→CPU stall; a debug panel does not need 60 Hz. Repaint at ~10 Hz (or
   only while open + not minimized). pixijs got liveness free; here it costs a stall, so we rate-limit
   ([I-1](issues.md#i-1)).

## Scope

**In:** removing `showRT` from `PENDING_W4F` + registering the real handler; a `showRT` singleton via
`panels.ensure`; a DOM `RtPanel` (canvas body, aspect-matched 2-up grid, per-tile name/dims/bytes
labels, title-bar memory total, empty-channel placeholders, native scroll); the viewport-side
`renderRtPreview` grid composite + single throttled readback + y-flip paint; ticking it from
`WorldScene.update`; `?showRT` URL-param parity (opens on load).

**Out (deferred):** decoded previews of bitfield channels (`shadow-cold` decode program) — land when
the webgl shadow tier does; the other W4f spikes (`/shadowcast`, `/es300`, `/mrttest`, `/inttest`) —
separate deferrals, engine techniques already proven in W2; any pixel-export / "save RT to PNG".

## Relationship

Executes the deferred `/showRT` on the [`webgl-engine`](../webgl-engine/README.md) parity bar
(W4f descoped it — [webgl-engine todo W4f](../webgl-engine/todo.md), [completed W4f](../webgl-engine/completed.md)).
Reuses the already-ported `Viewport.renderTextures()` / `aspect()` data source and the engine's
`Blitter` / `RenderTarget` / `Texture`. Ports layout logic from
[`client/pixijs` `RtPanel.ts`](../../../client/pixijs/src/game/panels/rt/RtPanel.ts) but rebuilds the
shell on `DomPanel`. No cross-component variable/table shapes change (no `VARIABLES.md`/`TABLES.md`
touch).
