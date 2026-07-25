# Todo — 2026-07-21-showrt

_Component: `client/webgl`. Design + crux in [`README.md`](README.md); the transport + shell decisions
in [`forks.md`](forks.md) (F1 readback, F2 DOM shell); the readback-cost handling in
[`issues.md`](issues.md). Phased P1→P5; complete when `/showRT` opens a live G-buffer preview grid and
matches the pixijs panel feature-for-feature (minus the deferred bitfield decode)._

## P1 · Viewport-side preview compositor — 2026-07-21

- [ ] Add `Viewport.renderRtPreview(layout)` (or a small `RtPreview` helper the viewport owns): given the
  panel's tile rects, `Blitter.blit` each **live** channel from `renderTextures()` into its rect of a
  panel-sized `RGBA8` `RenderTarget` (lazily allocated / resized to the body). Clear tiles for null
  channels. Returns the target (no readback yet). Verify by pointing `/overlayRT` at nothing and
  eyeballing the target via a temporary blit-to-screen, or defer visual check to P3.

## P2 · Single readback → ImageData — 2026-07-21

- [ ] One `gl.readPixels` of the whole grid target into a reused `Uint8Array`; wrap as `ImageData`. **One**
  readback for the grid, never one-per-channel ([F1](forks.md#f1)). Handle the GL bottom-left origin —
  y-flip on paint (P4), not in the blit. Throttle the readback to ~10 Hz / open-only ([I-1](issues.md#i-1)).

## P3 · DOM `RtPanel` shell — 2026-07-21

- [ ] New `client/webgl/src/game/panels/rt/RtPanel.ts` extending `DomPanel`: a `<canvas>` body (2-up grid,
  each tile's image area aspect-matched to `view.aspect()`), `overflow:auto` for **native** scroll
  (drop pixijs's wheel/thumb/drag entirely — [F2](forks.md#f2)). Port the portable layout math from
  pixijs `RtPanel` (`COLS`/`GAP`/`LABEL_H`, `rtBytes`, `fmtBytes`, per-tile name+dims+bytes label,
  title-bar memory total, empty-channel placeholder). Relayout only when a texture identity moved / the
  aspect or body size changed (same cheap-tick guard as pixijs).

## P4 · Paint + wire the command — 2026-07-21

- [ ] Paint the P2 `ImageData` into the panel canvas y-flipped; draw the labels. Register the real
  `showRT` handler: `WorldScene.showRenderTextures()` → `ctx.panels.ensure("showRT", () => new RtPanel({
  source: () => ({ aspect: vp.aspect(), channels: vp.renderTextures() }), … }))`; **remove `showRT`
  from `PENDING_W4F`**. Tick the panel from `WorldScene.update()`. Verify in-browser: `/showRT` opens a
  grid of the 8 G-buffer channels updating live as the camera pans/zooms; re-issuing re-focuses; close
  works; memory total is sane.

## P5 · URL-param + parity pass — 2026-07-21

- [ ] Confirm `?showRT` opens the panel on load (runs through the same `execCommand` replay as `/grid`
  etc.). Side-by-side against the pixijs `/showRT`: same channels, same labels, live update, empty
  placeholders. Log any intentional delta (e.g. throttled vs free liveness, no bitfield decode yet) in
  a `deviations.md` (created lazily at the first deviation) and confirm nothing else on the
  webgl-engine parity bar regressed.

---
_Deferred (out of scope, see [`README.md`](README.md)): the `shadow-cold` **decoded** preview (needs a
decode program — lands with the webgl shadow tier); the `/shadowcast` `/es300` `/mrttest` `/inttest`
spikes (separate W4f deferrals)._
