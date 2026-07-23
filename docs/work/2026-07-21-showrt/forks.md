# Forks — 2026-07-21-showrt

_Decision points + options + which we chose + why. Chronological._

## F1 · How does a GL composite reach the DOM panel? — 2026-07-21

The composites are `Texture`s in the viewport's WebGL2 context; the panel is a separate DOM `<canvas>`.
Textures don't cross contexts, and the viewport context renders only to its own canvas — so pixijs's
zero-copy "`Sprite` → same-context `RenderTexture`" is not available. Options:

- **(A) One grid RenderTarget in the viewport ctx + a single `gl.readPixels` → 2D `<canvas>` paint.**
  Blit every channel into one panel-sized target, read it back **once**, paint. **← CHOSEN.**
- **(B) Per-channel readback.** One `gl.readPixels` per tile straight from each composite. Simpler to
  wire (no grid target) but **N pipeline stalls per repaint** instead of one, and N `putImageData`s.
- **(C) A second WebGL2 context on the panel canvas.** Still can't sample the viewport's textures —
  you'd have to readback + re-upload into the second context anyway (all of A's cost **plus** a whole
  second renderer). No benefit.
- **(D) `drawImage(viewportCanvas, …)` canvas-to-canvas.** Works cross-context, but only copies what's
  **presented** on the viewport's default framebuffer — i.e. the live world — not the off-screen
  G-buffer composites. Would require rendering the grid onto the world's own canvas, fighting the world
  render. Rejected.

**Why A:** fewest GPU→CPU stalls (one readback of a small grid, not N), reuses the engine's existing
`Blitter` + `RenderTarget`, and keeps the panel a dumb 2D-canvas painter. The readback cost is real but
bounded and throttled ([I-1](issues.md#i-1)) — acceptable for a dev panel. It also naturally hosts the
future **bitfield decode** (route those channels through a decode program into the same grid target)
without changing the transport.

## F2 · Panel shell — port the pixijs `RtPanel` scroll machinery, or lean on DOM? — 2026-07-21

pixijs `RtPanel` is a **Pixi** panel: it hand-rolls a scrollbar (Graphics track+thumb), `window`
wheel/pointer listeners gated to the body rect, and a drag model — all because a Pixi body is
`pointer-events:none` and has no native scroll.

- **(A) Faithful port** — reproduce the wheel/thumb/drag + Graphics scrollbar on the DOM canvas.
- **(B) DOM-native shell** — a `DomPanel` whose body is `overflow:auto`; the browser gives wheel +
  drag-scroll + a real scrollbar for free. **← CHOSEN.** Delete the ~90 lines of scroll plumbing.

**Why B:** the entire scroll apparatus in pixijs exists only to work around Pixi's non-DOM body; on
`DomPanel` it's dead weight. Keep the **portable** parts (tile grid math, aspect sizing, `rtBytes` /
`fmtBytes`, labels, memory total, the relayout-only-when-changed tick guard) and drop the shell
workarounds. Net: a smaller, more idiomatic panel than the source.

_Sub-decision — one grid canvas vs a DOM `<canvas>` per tile:_ **one canvas** painted from the single
P2 readback (matches F1-A: one target, one readback, one paint). A per-tile canvas grid would re-invite
per-channel readback (F1-B) purely for layout convenience — not worth it. Labels draw into the canvas
(or a thin absolutely-positioned DOM overlay if crisper text is wanted).
