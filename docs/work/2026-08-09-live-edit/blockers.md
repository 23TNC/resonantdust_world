# Blockers — live-edit

## 2026-08-09 — the preview needs a design decision about the renderer (open)

**What blocks.** P5's remaining three items (build the preview, zoom/pan on it, graceful
non-pawn) are all downstream of one question I should not answer alone.

**Why it needs you.** The spike ([F1](forks.md#f1)) established that a second `Viewport` costs
exactly one extra WebGL2 context and that the browser evicts the world viewport at 16 — so one is
safe. Building it then surfaced what the spike missed ([I10](issues.md#i10)): a `Viewport` is a
renderer, not a world. `MoverLayer` and `WorldBridge` each hold **one** viewport and push every
prim into it, so a second instance has a camera, a context, and an empty scene.

That makes the three options genuinely different pieces of work, and the choice is about the
renderer's shape rather than about this panel:

- **A′ — duplicate the content feed.** `MoverLayer`/`WorldBridge` push to a list of viewports
  rather than one. Truest preview (it IS the world, zoom and pan free from `Camera`), but doubles
  per-frame prim work while the panel is open and touches the hottest code in the client.
- **B — a purpose-built object preview.** Draw just the selected object's sprite at a chosen
  scale on a small canvas. Cheap and contained, but re-implements a slice of drawing that will
  drift from the renderer — the thing several streams have worked to eliminate.
- **C — drop the preview.** The four tabs are the panel's substance and all work; the preview is
  the one part of your description that is not information the tabs already carry.

**My recommendation: B**, and deliberately narrow — the preview's job is "which object am I
looking at", not "render the world twice". It cannot evict a context, cannot double prim work,
and the drift risk is bounded because it draws one sprite rather than a scene. A′ is the right
answer only if you want the preview to become a genuine second view of the world later.

**State.** `PreviewViewport.ts` is written and works as far as mounting, zoom and pan wiring go —
it is the A′ half that has no content. The panel currently shows a one-line placeholder in the
preview region rather than a live-but-blank canvas, because a black rectangle reads as a broken
panel. Nothing else in the stream depends on this: `/edit` and all four tabs are complete.
