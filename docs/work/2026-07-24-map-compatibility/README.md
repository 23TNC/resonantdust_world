# Map compatibility — which maps can look each other up, and why on-prim casting failed twice — 2026-07-24

_Component: [`client/webgl`](../../components/client/) · `game/viewport/`. An INVESTIGATION stream (no
feature): establish, from the code, which maps may be cross-referenced by world coordinate and which may
not — so a feature never again reaches across an incompatible pair. Phases in [`todo.md`](todo.md);
decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md)._

## Why this stream exists
Shadows-on-prims was implemented and **reverted twice** for the same class of bug: sampling one map from a
shader that renders into a *different* map, by world position, and getting the wrong texel once the camera
**zooms**. The second revert is [`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md)
(the gather sampled the **zdepth composite** to learn where a prim is drawn; it drifted on zoom and
corrupted even the ground shadows). Before a third attempt we must know, precisely, **what is and isn't a
safe cross-map lookup in this codebase.**

## The user's hypothesis (and it holds)
> textile_square and textile_unit are both formed from tiles and should resolve to units — but because the
> textile_square textures are **fixed size**, on zoom they are **not grid-aligned**, so a coordinate lookup
> into them fails.

Grounded in the code, this is correct — and it exposes a map family that `map-model.md` never named. There
are really **two kinds of grid**, distinguished by whether a world tile lands at a **fixed** texel stride or
a **variable, zoom-chosen** one:

| family | maps | sized | textiles / tile | layout | world-coord addressable across zoom? |
|---|---|---|---|---|---|
| **`textile_tile`** (positions in TILES+anchor) | the data texture: lights / prims / presence / **caster buckets** | `1024×1024` region torus, fixed | **1** | contiguous, region-torus fold | **YES** |
| **`textile_unit`** (positions in UNITS) | `shadow-cold`, the baked lightmap | **`cols·TEXTILE_UNIT × rows·TEXTILE_UNIT`** — exact, from the tile window | `TEXTILE_UNIT` = **16**, constant | contiguous toroidal (`pmod(tile,cols)` + sub-tile), no pad | **YES** — obeys `map-model.md` |
| **`textile_slot`** *(NEW name — the honest one)* | the `SquareCache` composites: `albedo` / `normal` / `surface` / **`zdepth`** | **`fixedCW × fixedCH`** — a FIXED reserve (`ceil(screen + RESERVE_CSS)`), independent of the tile count | **`slotPx = floor(fixedCW/(cols+2))`** — variable, floor-quantised, **changes with zoom** | padded **slot atlas**: **one slot per tile** at slot `(mod(wc,cols), mod(wr,rows))`, interior at `(slot+1)·slotPx`, a 1-slot apron ring | **NO** |

## `textile_slot` — the family we were missing (user, 2026-07-24)
The composites were labelled `textile_square` (a contiguous `SQUARE` px/tile grid). They are **not** that.
They are a **`textile_slot`** map: **each slot is one tile**, packed into a fixed-size atlas, and the slot's
pixel size **`slotPx` is chosen by zoom** (so a zoomed-out window still fits its tiles in the fixed texture,
and a zoomed-in one gets sharper tiles). Naming it makes the compatibility rule fall out of the family:

- A **contiguous** map (`textile_tile` / `textile_unit` / a *true* `textile_square`) has a **fixed** world→
  texel relation, so any pass can address it by recomputing from a world tile — at any zoom.
- A **`textile_slot`** map's world→texel relation runs through `slotPx`, which is a zoom-dependent `floor`
  in a fixed atlas with an apron. There is **no** stable world→texel formula. It is addressable **only with
  a UV the cache itself emits** (per-vertex, in `fillDisplay`, which knows `slotPx`+apron) — the display
  blit does exactly this and is safe. A *recomputed* world-coord lookup (what the gather did for `zdepth`)
  drifts on zoom ([`issues.md#i1`](issues.md#i1); observed `slotPx` **48 → 57**, same texture width).

So the incompatibility is now a one-line rule: **never address a `textile_slot` map by recomputed world
coordinate.** On-prim casting broke because it treated `zdepth` (a `textile_slot` map) as if it were
contiguous.

## The core tension
`map-model.md` asserts every map is `cols·R × rows·R`, contiguous, tile-toroidal, and calls the composites
`textile_square` = `SQUARE` (64) textiles/tile. The `SquareCache` composites implement a `textile_slot`
atlas instead. **The authoritative model names a family the code doesn't build, and omits the one it does**
([`issues.md#i2`](issues.md#i2)) — precisely where cross-family lookups fail. P2 must add `textile_slot` to
the model (and decide whether the composites stay `textile_slot` or become a real contiguous `textile_square`
— [forks F1](forks.md#f1)).

## What this stream produces (not a feature)
1. A **map inventory + compatibility matrix**: every live map, its true on-texture layout, and which
   coordinate systems can address it safely.
2. A **root-cause writeup** of the zoom drift (numeric reproduction), so the rule is understood, not just
   asserted.
3. A **decision** ([forks F1](forks.md#f1)): conform the `SquareCache` composites to `map-model.md`
   (contiguous, un-padded, `cols·SQUARE × rows·SQUARE`) so cross-family lookups become legal — **or** keep
   the atlas and make it a RULE that no shader ever cross-references a composite by world coordinate (a
   feature needing that data derives it *within its own family* instead — e.g. bake a `textile_unit`
   zdepth, or read the caster buckets already in the data texture).
4. A **re-plan** of on-prim casting on the chosen basis — the third attempt does not start until the
   compatibility rule is settled.

## What we already know is SAFE (from the working build)
- `textile_unit` intra-family: the gather, the lighting bake, and the overlay all address `shadow-cold` /
  the lightmap by the same `pmod(tile,cols)`+sub-tile mapping — stable across zoom (the shipped lighting).
- The **data texture** (`textile_tile`: lights / prims / presence / caster buckets) is region-torus folded
  and zoom-independent (positions are tiles+anchor, not px) — the gather reads it safely at any zoom.
- The **display blit** samples the `textile_square` composites by `vUV` — safe because the vUV is handed to
  it **per-vertex** by `SquareCache.fillDisplay` (which knows `slotPx`/apron), NOT recomputed from a world
  coordinate. That is the tell: composites are safe to read only with a UV the cache itself produced.
</content>
