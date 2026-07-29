# Forks — linked cell padding

_Decision points, options, which we chose and why._

## F1 — `cell`, not `slot` {#f1}

The user asked for "per-slot padding". **`slot` is already taken, by something adjacent enough to be
dangerous:** the textile SLOT grid — 32×16 fixed slots, the toroidal window every map rides, with
`win.slotPx` as its per-tile texel size. That is the concept whose mis-addressing caused
[`2026-07-24-map-compatibility`](../2026-07-24-map-compatibility/README.md), and which the
[square-128](../2026-07-28-square-128/README.md) stream had to untangle from the lightmap only
yesterday.

- (a) Use the user's word, `slot`.
- **(b) Use `cell`.**

**Chosen: (b).** The sub-images of a linked sheet are *already* called cells everywhere in the code —
`cellFrame`, `cellFrames`, the `cell` parameter on `resolve()`, `entry.grid`. So (b) is not renaming
the user's idea, it is spelling it the way the codebase already spells it, and it keeps `slotPx` from
ever appearing three lines from a `slotPad` that means something unrelated.

This is a decision about the identifier only. The user's model — a guard ring around each sub-image —
is what gets built, unchanged.

## F2 — What unit is the authored number in? {#f2}

`cellFrame` consumes a fraction of the WHOLE ATLAS (`u0 = cx/cols + pu`). Nobody can author that.

- (a) Normalized atlas fraction — what the code wants.
- **(b) Pixels per cell edge, at the art's native cell size.**
- (c) A fraction of one cell.

**Chosen: (b).** It is the only unit someone looking at the art can *count*, which matters because
this number describes the art rather than the object. It also survives changes (b) does not obviously
survive: if the grid goes 4×4 → 8×8 or the sheet size changes, the guard ring in the art is still the
same number of pixels, and the conversion `pu = (padPx / nativeCellPx) / cols` absorbs the difference.

(a) makes the authored value silently wrong the moment the grid changes, and couples the corpus to the
atlas layout. (c) is LOD-invariant like (b) but is a number nobody can measure off the source PNG.

**The conversion must use the NATIVE cell size, not the loaded one.** Dividing by the current lod's
cell px would make the authored value mean a different physical inset at every zoom — the exact bug
class this repo keeps re-learning. P2's last item exists to check this.

## F3 — DSL or the manifest? Both exist. {#f3}

[`2026-07-28-art-128-tiles`](../2026-07-28-art-128-tiles/README.md) is *concurrently* fixing the art
pipeline to emit a per-cell `padU`/`padV` into `atlas.json` → the manifest — `bin/lib/pad_maps.py`
already documents it as "the per-cell guard for an atlas is the GRID_INSET_FRAC sampling inset …
trimmed off each cell's UV rect by the client". So after that stream lands, the manifest carries a
correct pad on its own.

- (a) DSL only — ignore the manifest's.
- (b) Manifest only — decline the request, point at the sibling stream.
- **(c) DSL overrides; manifest is the fallback.**

**Chosen: (c).** Two independent sources of one number is a genuine smell, and (a) would be the clean
answer if the pipeline never emitted it — but it does, and deleting a working path to satisfy a
preference would break every stem that has a pad and no authored one. (b) refuses work the user asked
for on the grounds that another stream might eventually cover it, which is not this session's call to
make.

(c) is honest about which is which: **the manifest states what the art HAS; the DSL states what we
want used.** They agree in the normal case, and the override exists for art whose manifest is wrong or
not yet re-emitted — which is exactly today's situation while the sibling stream is mid-flight.

**Ordering, explicitly:** authored value if the def sets one, else `entry.pad`, else zero. A def
authoring `0` is a real instruction — "trim nothing" — and must NOT fall through to the manifest. That
means the field needs an unset/zero distinction; simplest is to treat the DSL default as "unset" and
require an explicit author to override, which is what P1 sets up by authoring an explicit `0`.

## F4 — How does the pad reach the resolver? {#f4}

`resolve(stem, map, cell)` has no idea which def is asking.

- **(a) Thread the pad through `resolve()` from the caller.**
- (b) Register a per-stem pad table on the resolver at corpus load, keyed like the manifest.

**Chosen: (a).** It matches what the value now *is* — a property of the def's use of the texture, per
the user's framing — so the def that knows it is the thing that passes it. (b) is tidier at the call
sites but re-introduces the ambiguity F3 just resolved: a stem shared by two kinds with different pads
would silently take whichever registered last, and it puts corpus state inside a component whose job
is texture streaming.

Cost of (a) is touching the call sites. There are few, and they already pass `cell`.

## F5 — The 126 → 128 stretch breaks whole-px-per-unit {#f5}

Trim 1 px from each side of a 128 px cell and 126 px stretch to 128. Every other map in this engine
holds an integer px-per-unit (`VARIABLES.md`: `ppu = SQUARE/(16·2^lod)` → 8/4/2/1), and the universal
rescale rule is NEAREST. A 126→128 NEAREST stretch duplicates two rows and two columns somewhere
inside each cell.

- **(a) Accept it. Decide by eye in [P3](todo.md).**
- (b) Constrain the pad so the trimmed region divides evenly.
- (c) Sample linked cells bilinear, breaking the NEAREST rule for this family only.
- (d) Change the art so the cell PITCH carries the pad (130 px cells holding 128 px of art), making the
  trim exact and the stretch identity.

**Chosen: (a) for now, with (d) named as the real answer if it bites.** The artifact is two duplicated
lines in a wall texture; it may be entirely invisible, and this is a question a zoomed screenshot
settles in a minute where an argument could run all day. Deciding it in advance would be inventing a
requirement.

(b) constrains the art to serve the sampler, backwards. (c) breaks a rule the whole engine leans on and
would soften the very edges the pad exists to protect. **(d) is the principled fix** — it is what the
user's own phrasing ("how much padding each of the individual textures HAS") describes if the pad sits
*outside* the nominal cell rather than inside it — but it is an art-pipeline change, which is
[art-128-tiles](../2026-07-28-art-128-tiles/README.md)'s territory, not this stream's. If P3 says the
artifact is real, the finding is filed here and the fix belongs there.
