# Forks — strip the lighting + shadow system

_Decision points, options, which we chose and why._

## F1 — What does "stripped" render like? {#f1}

The blit is `albedo × (ambient + cold + hot)`. Removing the accumulators leaves the ambient term, and
its value is a choice with consequences for every screenshot taken until the replacement lands.

- **(a) `ambient = 1.0` — full-brightness unlit albedo.**
- (b) Keep today's ambient floor (~0.1), so the world stays dim.
- (c) Keep one static directional term so form is still readable.

**Chosen: (a).** The point of the strip is a **clean seam**, and (b) leaves the world in a state that
looks like a bug — a successor loading the client would reasonably think the lighting is broken
rather than absent. (c) is worse: it is a lighting system, small enough to feel free and permanent
enough to constrain the re-think, which is exactly the pattern being escaped.

Full-brightness albedo also makes the art honestly visible for the first time in a while, which is
worth something on its own: the [albedo-outlines](../../../.claude/) note records that near-black
albedo masters are normal and can only be judged on screen.

**The cost, stated plainly:** the world will look flat, and normal/depth maps will render as unused
files. That is what a strip looks like, and hiding it under a token light would make the seam harder
to find, not easier.

## F2 — Delete the design docs, or archive them? {#f2}

`design/{lighting,shadows}.md` and `intent/{shadows,tiered-lighting}.md` describe a system that will
not exist.

- (a) Delete outright — git holds the history.
- **(b) Move them out of the repo to `../archive/`.**
- (c) Leave them with a "superseded" banner.

**Chosen: (b)**, which is the convention's own answer: _"a superseded / pre-rewrite design … move it
out of the repo to `../archive/`"_. (c) is banned for good reason — an in-repo doc gets read as
current, and a banner is exactly the deprecation marker the tree-wide rule rejects.

(a) is tempting given `delete, don't deprecate`, but these documents are not stale *notes*, they are
the reasoning behind a system the re-think will want to argue with. The distinction the convention
draws is between a doc that misleads (delete) and a design that is genuinely superseded (archive),
and this is the second.

## F3 — Strip in one pass, or cut consumers before deleting producers? {#f3}

- (a) One pass: delete the files and fix what breaks.
- **(b) P1 cuts consumers and dims the passes; P2 deletes.**

**Chosen: (b).** The extra phase costs an hour and buys a **reversible checkpoint**: after P1 the
lighting code still exists and still compiles, so if the unlit render exposes something unexpected —
a z-order artefact the shadow pass was masking, a sprite that only reads correctly when lit — the
diagnosis happens against working code rather than against 4 300 deleted lines.

It also separates the two questions that a single pass would fuse: *does anything still need the
lighting output?* (P1) and *did the deletion miss something?* (P2). Fusing them is how a strip turns
into a debugging session.

## F4 — Do the normal and depth maps stay in the art pipeline? {#f4}

Marigold generates albedo, normals and a 16-bit depth field per master. After the strip **nothing
consumes normals or depth.**

- (a) Strip the generation too — dead assets are debt.
- **(b) Keep generating them; drop only the consumers.**

**Chosen: (b).** They are already generated, already gated by `atlas_check`, and cost only disk. Any
lighting model worth re-thinking toward will want a surface normal, and regenerating the corpus is
hours of GPU time plus the re-tuning that `2026-07-29-marigold-linked-normals` is still working
through.

The honest risk is that unconsumed maps **drift** — nothing renders them, so nothing catches a
regression. That is why the seam doc (P5) names them explicitly as present-but-unread, rather than
letting a successor discover them.

## F5 — Which parts of `coldShadowData.ts` are lighting, and which are the world? {#f5}

The file is named for shadows and is 1 348 lines, but it is really **the unified data-texture
writer** — the primitive graph lives in it. Deleting it wholesale would take out how the world
addresses its objects.

- (a) Delete the file; re-home the graph writer afterwards.
- **(b) Split it in place: keep the graph, delete the lighting paths.**

**Chosen: (b).** The split is clean because the bands are clean:

| stays | goes |
|---|---|
| `definition_data`, `prim_data`, `billboard_data` — sprites, z-depth, selection all read these | `light_data` (set 2) |
| the command-buffer scatter + the CPU mirror | `light_presence_lo` (3), `light_presence_hi` (5) |
| `encodePosition` / `resolveCarried` / the free-list | the caster bucket build |

**`prim_presence` (set 4) is the one genuine unknown**, which is why P3 proves its readers before
touching it. It is an *occupancy* relation — an object registering on the tiles its footprint covers
— and occupancy is a world fact, not a lighting fact. If the shadow walks were its only readers it
goes; if placement, selection or collision read it, it stays. That is a five-minute grep and it is
written as an item rather than assumed either way.

**The file should be renamed** once it holds no shadow code — `coldShadowData` would then describe
nothing it does. Left to P2 rather than hoisted here, because renaming during a delete makes the
diff unreadable.
