# object-preview — the live-edit preview, drawn without a renderer

_User (2026-08-09, after the live-edit stream stopped on a blocker): "Please write up a folder of
work for B. We will implement it later."_

Option **B** from [`2026-08-09-live-edit`'s blockers](../2026-08-09-live-edit/blockers.md): a
purpose-built preview of the SELECTED OBJECT for the live-edit panel's top region, instead of a
second world `Viewport`.

## Why B, and why it is smaller than it sounded

The live-edit stream stopped here for a good reason. Its spike proved a second `Viewport` costs
exactly one extra WebGL2 context (the browser evicts the world viewport at the 16th, granting
every request and throwing nothing) — and then building it surfaced the real cost: **a `Viewport`
is a renderer, not a world.** `MoverLayer` takes ONE viewport and pushes every pawn prim into it,
`WorldBridge` does the same for terrain and things, so a second instance has a camera, a context
and an empty scene. Candidate A's true price is duplicating the content feed — every prim to two
viewports, every frame.

B was described in that blocker as "re-implements a slice of drawing, which will drift". Surveying
it properly for this plan, that is **too pessimistic**, and the reason is worth stating up front
because it changes the whole shape of the work:

- Textures are fetched as ordinary HTTP (`{root}/lod/{hash}/{size}/{map}/{stem}`) and decoded with
  `createImageBitmap`. Nothing about drawing one sprite requires GL.
- `TextureResolver` **already knows** every frame's sub-rect, bounding box and sprite scale
  (`subframe`, `spriteBBox`, `spriteScale`, `packedHash`, the manifest). The preview does not have
  to work any of that out — it has to *ask*.

So B is: fetch the image the resolver already knows the URL of, and `drawImage` the sub-rect the
resolver already computed, onto a **2D canvas**. Zoom and pan become a canvas transform. No GL
context, so no eviction risk. No prim duplication, so no per-frame cost while the panel is open.
The "drift" risk shrinks from "a second renderer" to "which rectangle do I crop", which is a
lookup rather than an algorithm.

## The stance

- **A 2D canvas, no GL** (F1). This is the decision that makes everything else small. It also
  means the preview cannot participate in lighting or shaders — correct, because its job is
  *which object am I looking at*, not *what does the world look like*.
- **Ask the resolver, do not re-derive** (F2). The preview gets a narrow read-only accessor
  surface on `TextureResolver` rather than a second copy of the URL/atlas logic. Every number it
  draws with comes from the same place the real renderer's numbers come from — which is what
  actually prevents drift, far more than avoiding a second draw path.
- **Composite the object's PARTS** (F3). A pawn is multi-part (body + head, human-pawns-redux),
  each with its own stem, offset, scale, tint and flip, drawn in `zIndex` order. Drawing only the
  first part would silently show a headless pawn.
- **Fit first, then let the user zoom** (F4): the preview opens framed on the object rather than
  at some arbitrary scale, because "which object is this" is answered by the default view.

## Watch

The corpus is still largely placeholder art — several kinds are FLAT TINTED SQUARES, and a
texture that has not resolved yet has no bitmap at all. The preview therefore needs an honest geo
fallback (the kind's colour + glyph, exactly as the world's geo tier does) rather than a blank
box, or it will read as broken precisely for the objects whose art is not done
([I3](issues.md#i3)). And the panel polls twice a second: fetching or re-decoding per poll would
be absurd, so the bitmap cache is not an optimisation but a correctness requirement for the
panel's cadence ([I5](issues.md#i5)).

## Not in this stream (named, not built)

Lighting, shadows or any shader effect in the preview (F1 forecloses it deliberately); animation
or facing cycling beyond drawing the object's current facing; previewing anything the selection
model cannot select; and the A′ option (feeding two viewports) — if the preview should ever become
a genuine second view of the world, that is a different stream against the renderer, not this one.

## Exit

Selecting an object with the live-edit panel open shows it in the preview region, framed to fit,
composited from all its parts with their tints and flips. The mouse wheel zooms and drag pans, and
a control returns to the fitted view. An object whose texture has not resolved shows its geo
colour and glyph rather than nothing. No WebGL context is created, and the world viewport is
provably unaffected. The user's eyes close the stream.
