# Subframe at ingest — crop once, into the atlas — 2026-08-02

_Components: [`client/webgl`](../../components/client/webgl/), [`dev/textures`](../../components/dev/textures/),
plus `shared/dsl` (no component folder yet — create it lazily if this stream is the first to need one).
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); what is suspect in
[`issues.md`](issues.md)._

## The user's call

> "I suspect we 'could' handle this with subframes … instead we are going to handle this on ingest.
> When we ingest a texture, we receive all four maps, and load them into our combine frame. We will
> place our subframe variables into our dsl instead. We will then ingest each of the maps, copying
> the subframe into the atlas instead, using the same subframe and offsets for each map. Using the
> anchor to align the ingested subframe into our atlas." — 2026-08-02

> "Our dsl will define subframes for each direction individually as each direction may have
> different specifications as they are different textures." — 2026-08-02

## Why the subframe moved, rather than being repaired

The subframe was not wrong as a *concept*. It was wrong as a **second copy**.

[normal-frames I8](../2026-08-02-normal-frames/issues.md#i8): the def carried the art's opaque bbox
quantised to whole units (`subX/subY/subW/subH`), and the card was placed and silhouetted through it,
bottom-aligned to the prim's anchor — while that anchor was the *continuous* opaque bottom,
`p.y + (bb.fy + bb.fh) · p.height`. **Two derivations of one edge, one rounded and one not.** On the
`100,48` bush that put the sampled texture half a unit north of the drawn pixels and stretched it by
`13/12.5`, painting a halo around every sprite.

The repair shipped that day deleted the subframe outright and put placement *and* sampling on the
frame. That closed the drift, and it paid for it: the anchor became the frame box's bottom, so a
letterboxed master's bottom margin puts the plan line south of the feet again
([lighting-visual I1](../2026-07-31-lighting-visual/issues.md), knowingly re-opened).

**This stream is the real answer, and it is a better one than either.** Crop at ingest and the
duplication has nowhere to live: the atlas holds only the art, the frame *is* the art, and the
anchor is an authored point rather than a number recovered from pixels. Placement, sampling and all
four maps are registered **by construction**, not by two derivations agreeing.

## The seam already exists

This is not new machinery. `TextureResolver.packCoPack` already takes one `AtlasDraw`
(`dx/dy/dw/dh`, `sx/sy/sw/sh`) and applies it to **every** source:

```ts
draws = srcs.map((s) => (s ? draw : null));   // the SAME rect for all four maps
```

That is verbatim "using the same subframe and offsets for each map". Today the rect comes from
`sprite_scale` (a DSL-authored pre-atlas scale about the `sprite_anchor` pivot, registered through
`WorldBridge` → `resolver.setSpriteScale`). The subframe is the same kind of number arriving through
the same door — which is also why [F5](forks.md#f5) has to settle how the two compose rather than
letting them both re-centre the art.

## Per direction, because a facing is a different texture

The resolver keys by stem **including the facing** — `biome-thing/default/flora/e` — and
`setSpriteScale`/`setLinkedPad` are already per-stem. A def BLOCK is already one rotation px per
facing (r0 south, r1 east, r2 north, r3 east **mirrored** = west). So a per-direction subframe needs
no new indexing anywhere; it needs the DSL to author it per direction and the resolver to hold it
per stem.

The one direction that is **not** its own texture is west ([F4](forks.md#f4)): west is the east
master mirrored, so its subframe must be east's mirrored about the frame centre. Authoring it
separately would let two numbers describing one image disagree — the exact failure this stream
exists to end.

## What this closes

- **[normal-frames I8](../2026-08-02-normal-frames/issues.md#i8)** — properly, at the source.
- **[lighting-visual I1](../2026-07-31-lighting-visual/issues.md)** — the plan line lands on the
  authored anchor, not on a letterbox margin. The def's GREEN word was left free for exactly this.
- **Atlas waste** — a letterboxed master spends texels on transparent margin at every lod. Cropping
  is a resolution win as well as a correctness one ([I1](issues.md#i1) sizes it).

## The number this stream moves

**No halo, and the plan line on the feet.** Measured, not eyeballed: for a sprite whose art fills a
known part of its master, the receiver map's covered texels must match the drawn silhouette's texels
with **zero** fringe rows, and the prim's stored ground row must equal its authored anchor. Secondary:
the fraction of each atlas frame that is opaque art goes **up** (I1's table, re-measured).
