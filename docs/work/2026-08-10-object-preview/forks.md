# Forks — object-preview

## F1 — a 2D canvas, and NO WebGL context

2026-08-10. The preview draws with `CanvasRenderingContext2D.drawImage` onto its own 2D canvas.
It creates no GL context, shares none, and cannot lose one.

This is the decision the whole stream rests on, and it is available only because of two facts
established while surveying: textures arrive as ordinary HTTP fetches decoded by
`createImageBitmap`, and `ImageBitmap` is a first-class `drawImage` source. Nothing about putting
one sprite on screen needs a renderer.

What it buys, measured against the alternative the live-edit spike priced: no context (so no
eviction — the browser was measured evicting the world viewport at the 16th extra context, with
every request granted and nothing thrown), no duplicate atlas residency, and no per-frame prim
work while the panel is open. What it forecloses: lighting, shadows, materials, anything shader.
That is the correct trade, because the preview answers *which object am I looking at*, and a lit
preview would answer a question nobody asked at the cost of the renderer's shape.

Rejected: a second `Viewport` (priced by live-edit F1 — one context is affordable, but the content
feed is not: `MoverLayer` and `WorldBridge` each push every prim into ONE viewport, so a second
instance renders an empty scene); an offscreen GL context shared with the world (a
`TextureResolver` is bound to its own context, so the sharing that would make it cheap is exactly
what is unavailable).

## F2 — ask the resolver for its numbers; never re-derive them

2026-08-10. The preview reads the frame geometry it needs through a **narrow, read-only accessor
surface** added to `TextureResolver` — the packed hash + size for a stem, its sub-rect, its
bounding box, its sprite scale — rather than rebuilding URL construction, atlas packing or
sub-rect maths of its own.

This, not the choice of drawing API, is what actually stops the preview drifting from the world.
Two draw paths that read the SAME numbers disagree only in pixels; two draw paths that each
compute their own numbers disagree in meaning, and the second kind of disagreement is the one that
survives review and ships. The resolver already holds all of it (`packedHash`, `subframe`,
`spriteBBox`, `spriteScale`, the manifest); it is private today purely because nothing outside
needed it.

The accessors are read-only and return plain data. The preview must never be able to make the
resolver fetch, pack, or upload — those are the renderer's business and doing them from a panel
would put GL work back on a path this stream exists to keep off it.

Rejected: duplicating `texUrl` + manifest lookup in the preview (a second source of truth for
where a texture lives, which breaks the day the layout changes — and it changed twice already);
passing the resolver in and reaching through `as any` (same coupling, no contract, and it hides
from the compiler what the preview depends on).

## F3 — composite the object's PARTS, in order

2026-08-10. An object is not one sprite. A pawn is multi-part (body + head — human-pawns-redux),
each part carrying its own stem, `offsetX`/`offsetY`/`offsetZ`, `scale`, tint, `flipX` and a
draw-order offset. The preview composites them in the same `zIndex` order the world draws them.

Drawing only the primary stem would produce a headless pawn that looks like a rendering bug in the
*world*, sending someone to debug the wrong system. Honouring tint and `flipX` matters for the
same reason: a preview that shows an untinted, unflipped sprite is quietly telling you the object
is something it is not, which is worse than showing nothing.

Rejected: previewing the primary part only (produces a visibly wrong object); re-using the
world's prim list for the selection (that list lives in the viewport the preview deliberately does
not have — and reaching for it is how A′ starts).

## F4 — fit to the object first; zoom and pan are adjustments

2026-08-10. The preview opens framed on the object — its composed bounding box scaled to fit the
region with a small margin — and the wheel/drag adjust from there. A control returns to the fit.

The default view is what answers the user's actual question, and an arbitrary starting scale makes
every selection change a small chore. Fit-first also makes the preview *self-scaling* across wildly
different objects: a tile, a bunny and a tree do not share a sensible fixed zoom.

Following the live-edit precedent: once the user pans or zooms, the preview stops re-fitting on
selection change until they ask for it — silently yanking their view back is the behaviour that
made `follow` yield in `PreviewViewport`.

Rejected: a fixed pixel scale (unusable across object sizes); re-fitting on every poll (fights the
user twice a second).

## F5 — draw the object's CURRENT facing

2026-08-10. The preview shows the facing the object is actually in, not a canonical portrait
angle. It is a *live* preview in a *live-edit* panel; a pawn that turns while you watch it should
turn in the preview, and a fixed south-facing portrait would silently disagree with the world.

Noted as a real trade: a wandering pawn's preview will flip about, which is visually busier than a
portrait. If that proves annoying the fix is a pin, not a canonical angle — pinning is a user
choice, a canonical angle is a lie.

Rejected: always-south portraits (stable and prettier, but disagrees with the thing it previews);
cycling all facings as an animation (an animation nobody asked for, and it would obscure the
current one).

## F6 — non-pawn selections preview too, through the geo path

2026-08-10. Things and tiles are selectable, so they get a preview: their texture when one has
resolved, and their authored colour + glyph when it has not — which is exactly what the world's
geo tier draws, and reuses `thing_color` / `tile_color_bg`, both of which already exist.

This falls out of F1 rather than costing extra: once the preview is "draw a bitmap or a coloured
box with a letter", the non-pawn case is the same code with a different source. Refusing to
preview non-pawns would leave the panel's top third blank for two of the three things a user can
click.

Rejected: pawns only (leaves the region empty for most selections); a bespoke tile renderer (the
geo colour + glyph IS the tile's honest appearance when unresolved).
