# Work — billboard-depth (size the sprite-less billboards + z-order via zdepth)

_Opened 2026-07-21. Runs **before continuing [`webgl-engine`](../webgl-engine/README.md)** (interleaves in
the W4 render port, on the geo tier — no textures yet). Component: `client/webgl` (its docs live in the
[`webgl-engine`](../webgl-engine/README.md) stream; design source [`client/pixijs`](../../components/client/pixijs/)).
Two coupled problems the geo tier surfaced now that the first lights + shadows are on
([webgl-engine W4f](../webgl-engine/completed.md)): sprite-less things bake as the wrong shape, and nothing
z-orders the shadow pass against the things._

## Why now

The world renders on the geo tier (every prim a solid `geoColor` box) and casts billboard shadows off the
standing prims. Two things read wrong:

1. **Conifers are square boxes.** A thing bakes as a **`size × size` square** (`placeThing`: `side =
   l.size · SQUARE`), so a conifer — tall and narrow — draws as a squat square and casts a square-based
   shadow. Sprite-less, the box IS the billboard; it should be the caster's real **W × H** (tiles), so a
   conifer reads as a tall billboard and throws a proportionate shadow. (When real sprites land in
   [webgl-engine W4h](../webgl-engine/todo.md), the same W × H frames the sprite — this is not throwaway.)

2. **Shadows draw over everything.** The shadow cast is a screen-space pass drawn **after** the whole world
   display, so a shadow paints **over the conifers too**, not just the ground. The intended order is:

   > **conifers on top of tiles and shadows · shadows on top of tiles.**

## The idea — order through `zdepth-world`, with the toroidal wrap

The G-buffer already has a `zdepth-world` channel (per-square, toroidal, pans + scales with the other
composites). Today it's near-empty: **things** write a tile-depth (`uTileDepth`), **ground writes black**.
Fill it properly — **write each fragment's world-tile depth (with the toroidal wrap-around), for tiles AND
things** — and it becomes the shared ordering key:

- **Conifers over tiles**: already true within the cold composite (things bake after tiles, higher
  `zIndex`), and stays true.
- **Shadows over tiles, under conifers**: the shadow pass samples `zdepth-world` at each screen fragment and
  **suppresses the shadow where a thing occludes that ground point** (the conifer's depth is nearer), so the
  conifer shows through on top while the shadow still lands on bare ground.

The wrap-around matters because `zdepth-world` lives in the **toroidal** cache buffer — the depth must be a
**wrapped world value** so the comparison is valid across the buffer seam exactly like the other channels.

## Shadows stay displayed (provisional)

Per the request: the direct coloured-shadow display is **not** the end state — `shadow-cold` becomes a
**lighting input** (a bitfield the lit render consumes), gated behind an overlay/lighting toggle. But **while
we're still building shadows, keep the shadow display ON by default** (the current always-on `ShadowCaster`).
This stream keeps that default; it does not build the lit consumer. Framed as provisional so it isn't
mistaken for final.

## Scope

In: billboard **W × H** sizing for sprite-less things; a real `zdepth-world` write (tiles + things, wrapped);
the shadow pass reading `zdepth-world` to order under things. Out: real sprites (W4h), the lit shadow
consumer, the `shadow-cold` world-space bitfield persistence ([webgl-engine D-2](../webgl-engine/deviations.md#d-2)).

## Open decisions

See [`forks.md`](forks.md): **F1** — where the billboard W × H comes from (a new content field vs derived
from `footprint`/`size`); **F2** — how the shadow pass respects thing-occlusion (sample `zdepth` in the cast
shader vs reorder the passes vs a display-time depth test).
