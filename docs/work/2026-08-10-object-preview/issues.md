# Issues — object-preview (anticipated inventory)

Written 2026-08-10 at planning, before any code.

## I1 — the resolver's frame data is private, and must stay narrow when it opens

`packedHash`, `subframe`, `spriteBBox`, `spriteScale` and the manifest are all `private` on
`TextureResolver` — not by design, but because nothing outside has needed them. [F2](forks.md#f2)
opens a read-only slice.

The risk is the slice widening later into "the preview can drive the resolver". It must expose
data and no verbs: no `resolve`, no `ensureCoPack`, nothing that fetches, packs or uploads. If the
preview can make the resolver do GL work, GL work is back on the path this stream exists to keep
it off.

## I2 — the atlas is a CO-PACK; the preview wants the albedo map only

Textures pack as one co-pack per stem across twin pages (albedo / normal / depth / …). A preview
drawing whatever page it lands on would show a normal map — vivid lilac, unmistakably wrong, and
the kind of bug that looks like a corrupted texture rather than a wrong index. The map is selected
explicitly (`albedo`), and the URL builder already takes it as a parameter.

## I3 — placeholder art and unresolved textures are the COMMON case

Much of the corpus is still flat tinted squares (shrub / cactus / reed / rock / logs / torches),
and a texture that has not resolved yet has no bitmap at all. So "no bitmap" is not an edge case
to guard, it is a normal state the preview will be in constantly during early play.

The fallback is the geo tier's own answer — the kind's colour with its glyph drawn dark-on-light,
which the `Viewport` already rasterizes for exactly this purpose. Getting this wrong makes the
preview look broken precisely for the objects whose art is unfinished, i.e. the ones a corpus
tuner is most likely to be inspecting.

## I4 — pixel art needs NEAREST, and the canvas smooths by default

`CanvasRenderingContext2D` has `imageSmoothingEnabled = true` by default, which will blur the art
at any zoom above 1× — and zooming in is the preview's main verb. It must be turned off, and it
must be turned off again after any canvas resize (the flag lives on the context; a context re-made
by a resize returns to the default).

Note also that the world renders NEAREST as its universal rescale rule for encoded maps — matching
it here is consistency, not preference.

## I5 — the panel polls twice a second; the preview must not fetch or decode on that cadence

The live-edit panel refreshes at 500 ms. Fetching a texture, or re-decoding an `ImageBitmap`, per
refresh would be absurd — and worse, would make the preview flicker as bitmaps swapped.

The cache is keyed by `(stem, size, hash)` and outlives panel opens, exactly like the preview
viewport singleton it replaces. A selection change looks up; only an unseen stem fetches. This is
a correctness requirement for the panel's cadence, not an optimisation.

## I6 — parts carry tint and flipX, and ignoring them makes the preview LIE

Each `MoverPart` has a tint and a `flipX` (east is the mirrored west texture — the texture-serving
model serves per-facing masters with west mirrored). A preview that draws the raw sprite shows an
untinted, wrongly-facing object while claiming to be a preview *of that object*. Both are cheap to
honour on a 2D canvas (`globalCompositeOperation` for tint, a negative x-scale for flip) and both
must be.

## I7 — the composed bounding box drives the fit, and parts stick out of it

[F4](forks.md#f4)'s fit needs the bounding box of the WHOLE composition, not the primary part's:
a head sits above the body's box, and `offsetZ` shifts it further. Computing the fit from one part
crops the object in its own preview. The box is the union of each part's placed rect, which the
compositor already has to compute in order to draw them.

## I8 — the live-edit panel currently shows a placeholder in this region

`LiveEditPanel` renders a one-line "preview — pending a content feed" note where this goes, and
`PreviewViewport.ts` (the A-shaped attempt: singleton `Viewport`, wheel-zoom, drag-pan, `follow`
that yields to the user) is still in the tree. Landing this stream means removing both — the
placeholder because it is replaced, and `PreviewViewport` because leaving a second, GL-based
preview implementation beside the working one is precisely the kind of dead alternative that gets
resurrected by mistake.
