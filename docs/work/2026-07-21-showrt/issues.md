# Issues — 2026-07-21-showrt

_Problems hit + candidate solutions + which we chose + why. Chronological._

## I-1 · Readback is a GPU→CPU stall — liveness isn't free here — 2026-07-21

**Problem.** pixijs got a live preview for free (same-context `Sprite` on the `RenderTexture`).
The engine port must `gl.readPixels` the grid out of the viewport context every repaint, which
**synchronises the pipeline** (CPU waits for the GPU). At 60 Hz over a full-body grid that's a
per-frame stall on the debug path.

**Candidates.** (a) Repaint every frame — simplest, but pays the stall continuously. (b) **Throttle**
to ~10 Hz and only while the panel is open + un-minimized — a debug preview doesn't need frame-perfect
liveness. (c) Async readback via a PBO + fence (`gl.fenceSync`) to avoid the stall entirely —
correct-est, but real complexity for a dev panel.

**Chosen: (b), with (c) noted as a future upgrade if it ever bites.** Throttle + open-gating removes
the cost from the steady state (panel closed = zero work, the common case), and 10 Hz is plenty to
watch the G-buffer while panning. Record the "not frame-live like pixijs" behaviour as an intentional
delta in `deviations.md` at P5 if it's user-visible.

## I-2 · GL origin is bottom-left; the panel canvas is top-left — 2026-07-21

**Problem.** `gl.readPixels` returns rows bottom-up; a 2D canvas paints top-down, so a naive paint is
vertically flipped. **Chosen:** y-flip at **paint** time (row-reverse the `ImageData`, or
`ctx.scale(1,-1)` + translate), not in the blit — keeps the grid RenderTarget in natural GL space so
the same target can later feed a decode program unflipped. Watch this per-tile too (each tile's image
must not end up individually mirrored).

## I-3 · Empty/dormant channels must still lay out — 2026-07-21

**Problem.** `renderTextures()` can return `texture: null` for a channel not yet produced (e.g. warm
before any mover bakes). The panel must draw a labelled placeholder tile (pixijs shows `name —`), not
collapse the grid — the panel doubles as a "what's live" checklist. **Chosen:** clear that tile's rect
in the grid target and paint the placeholder box + `name —` label, mirroring pixijs `RtPanel`'s
null-texture branch. No readback change (the tile is just cleared).
