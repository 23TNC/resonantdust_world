# Map model — the shared toroidal TILE grid

_The load-bearing decision for the lighting rebuild (and every map we hold). Referenced from
[`README.md`](README.md); the phases in [`todo.md`](todo.md) build on it._

Every map — the **data textures** (lights, prims, presence, shadow) **and** the full-resolution
textures (albedo, normal, …) — is held on **ONE shared grid: `cols × rows` TILES**, toroidal,
wrapping **by TILES**. Because the shape *and* the wrap are identical across every map, a world tile
`(wc, wr)` lands in the **same toroidal cell in every map** — `(pmod(wc, cols), pmod(wr, rows))` —
so their x/y coordinates relate directly, and **panning never moves data**: only the window's
tile-origin advances.

## The three resolutions — `textile_tile` / `textile_unit` / `textile_square`

A **textile** is one texel of a map. Every map shares the tile grid; each picks how many textiles
it packs per tile. In code: `TEXTILE_TILE` / `TEXTILE_UNIT` / `TEXTILE_SQUARE` (+ `UNIT`) in
`client/webgl/src/game/viewport/squareMath.ts` — the maps size and index off these, never a
re-derived literal.

| name | textiles / tile | one textile covers | position stored in | used for |
|---|---|---|---|---|
| **`textile_tile`** | `1` | 1 tile | **tiles** (+ sub-tile anchor) | lights, caster buckets (prims), presence |
| **`textile_unit`** | `SQUARE/UNIT` = **16** | 1 unit (`UNIT` px) | **units** | the shadow map |
| **`textile_square`** | `SQUARE` = **64** | 1 px | **px** | albedo, normal, … |

Fixed relations (`SQUARE` = px per tile, the one dial):
`UNIT = SQUARE/16` (px per unit) · `16 units per tile` (`= SQUARE/UNIT`) · `SQUARE px per tile`.
So `textile_unit` = **16 textiles/tile** (one per unit) — that count is `SQUARE/UNIT`, **not** the
unit's pixel size `SQUARE/16`.

## Scaling resolution — bump `SQUARE`

The viewport is measured in **tiles**; `SQUARE` is the only resolution dial. Raise it and the
`textile_square` maps (albedo/normal) get sharper, while `textile_unit` **stays 16 textiles/tile** —
each just covers more px:

| `SQUARE` | `UNIT = SQUARE/16` | `textile_unit` (per tile) | each `textile_unit` covers |
|---|---|---|---|
| 64  | 4 px | 16 | 4 px |
| 128 | 8 px | 16 | 8 px |

A map at resolution `R` is a texture of size **`cols·R × rows·R`**.

## Rules

1. **One `(cols, rows)` for everything.** Every map is `cols·R × rows·R`; they allocate/resize
   together from the single tile window. No map invents its own column/row count.
2. **One toroidal wrap, at the TILE level.** Indexing is *always* `pmod(worldTile, cols/rows)` to
   pick the cell; the sub-tile offset (unit or px) is added *within* the cell. There is **no
   per-map window offset** that the others don't also apply — every map resolves a world tile the
   same way.
3. **Coordinates by resolution.** Prim/light positions are **tiles** (+ sub-tile anchor); shadow
   texels are **units**; albedo sampling is **px**. Convert by the fixed factors above; never store
   the same quantity in two unit systems.

## Why this is the unblock

The shadow bug: the reach-walk indexed the **caster buckets** on a different toroidal basis than the
**shadow-cold RT** used, so it walked the wrong tiles and never read the caster it needed — even
though the bucket held it (a *direct* `pmod(tile, cols)` read returned the right prim; the walk's
`lc + (dx,dy)` did not). Forcing every map onto this one shared TILE grid — same `cols/rows`, same
`pmod` wrap, tile-level indexing — makes the walk's tile coordinates line up with the buckets **by
construction**, so the class of bug can't recur.
