# Per-cell padding on linked textures, authored in the DSL — 2026-07-29

_Components: [`client/webgl`](../../components/client/webgl/) (the consumer). The authored field lands
in `shared/dsl/src/loader.rs`, which has no component doc yet — see the
[components index](../../components/README.md). Plan in [`todo.md`](todo.md);
decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md). Sibling:
[`2026-07-28-art-128-tiles`](../2026-07-28-art-128-tiles/README.md) owns the WRITER side of the same
number — see [F3](forks.md#f3)._

## What the user asked for

> Update linked textures with per-slot padding. Assign this variable in the object's DSL
> configuration. Then when we need a frame from a linked texture we grab the 128 px frame, apply the
> padding to all 4 sides, and scale up to 128 to fit our texture size. Scale and such applies after
> this. Padding is just telling us how much padding each of the individual textures has in the linked
> texture atlas.

So `padding` describes **the art**: how much guard ring each sub-image carries inside the linked
sheet. The client trims that ring off the cell's sampling rect, and the remaining art stretches to
fill the tile.

## What already exists, and what actually has to be built

The trim itself is already implemented. `TextureResolver.cellFrame`
([`TextureResolver.ts:215`](../../../client/webgl/src/textures/TextureResolver.ts)) insets every cell
on all four sides:

```ts
const u0 = cx / cols + pu;          const uw = 1 / cols - 2 * pu;
const v0 = cy / rows + pv;          const vh = 1 / rows - 2 * pv;
```

And the **scale-up is free** — `cellFrame` returns a `TexFrame`, a rect on the atlas page, which the
consumer stretches across the destination footprint. There is no resample and no second step to
build ([I1](issues.md)).

What is missing is only the **source of `pu`/`pv`**. Today it is `entry.pad` off the texture manifest,
written by the art pipeline. The user wants it authored per object in the corpus. So the real work is
a **plumbing job**, not a rendering job:

```
content/visual/*.rd  →  loader.rs VisualParts  →  def payload  →  TextureResolver.cellFrame
      NEW                     NEW                    NEW              already correct
```

## The two decisions that shape it

**Units.** `cellFrame` wants a fraction of the whole atlas; the user is thinking in **pixels of a
128 px cell**, which is the right authoring unit — it is what someone looking at the art can count,
and it stays meaningful as the grid or sheet size changes. The conversion
`pu = (padPx / nativeCellPx) / cols` is LOD-invariant, so one authored number is correct at every lod
([F2](forks.md#f2)).

**Naming.** The user said "slot", but **`slot` is already the textile slot grid** (32×16 slots, the
toroidal window) — a completely unrelated concept that this codebase's lod bugs have repeatedly turned
on. The sub-images of a linked sheet are called **cells** everywhere in the code already (`cellFrame`,
`cellFrames`, `entry.grid`, the `cell` parameter). Using `cell` here is not a rename of the user's
idea, it is avoiding a collision that would be actively dangerous ([F1](forks.md#f1)).

## The consequence worth deciding with eyes open

Trimming 1 px off each side of a 128 px cell leaves 126 px, which then stretches to 128. **That is a
non-integer scale**, and it breaks the whole-px-per-unit invariant `VARIABLES.md` states for every
other map (`ppu = SQUARE/(16·2^lod)`, an integer at every lod). Under the codebase's NEAREST-everywhere
rule, a 126→128 stretch duplicates two pixel rows and two columns somewhere in the cell.

That may be invisible on a wall texture, or it may read as a subtle irregularity in a tiling material —
exactly where it would be most noticeable. [F5](forks.md#f5) records the options and commits to
deciding it by eye in [P3](todo.md) rather than in advance, because that is the kind of question a
screenshot answers and an argument does not.

## Done when

`&tile.texture_pad` is authored on `wall_smooth`, flows through `VisualParts` to the resolver, and the
wall's cells sample inside their guard ring with no bleed from neighbouring cells — verified against a
before-image, at more than one lod, with the manifest path still working for anything that does not
author the field.
