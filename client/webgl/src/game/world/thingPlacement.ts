//! Thing/pawn spatial placement — the one place that turns a content LAYOUT
//! (`Content.thingLayout`, authored via `&thing.footprint` / `.anchor` / `.size` /
//! `.sprite_anchor`) into a world-px sprite box + a z-row. Shared by the cold-thing
//! painter ({@link WorldBridge}) and the pawn layer ({@link MoverLayer}) so both place
//! identically.
//!
//! The model (see `VisualParts` in `shared/dsl`): an object's position is its TOP-LEFT
//! tile. `footprint` (tiles) is the logical rectangle it occupies; `anchor` (0..1) is a
//! point WITHIN that footprint; `sprite_anchor` (0..1) is the pivot on the (square) art
//! that gets pinned to the anchor; `size` (tiles) is how big the square sprite draws.
//! Bottom-anchoring is just `anchor.y = sprite_anchor.y = 1` — no special case.
//!
//! **Footprint is visual-only today**: its server-side occupancy (movement, hit-testing)
//! and the e/w↔n/s dimension swap on rotation are deferred with the object-model
//! storage (`docs/object-model.md`). Here it only scales where the anchor lands and
//! which tile row drives z.

import { SQUARE } from "../viewport/squareMath";

/** Floats per def in the flat `Content.thingLayout()` table (mirrors the Rust
 *  `Bundle::thing_layout` stride): `[fw, fh, ax, ay, size, sx, sy, span, scw, sch]`. */
export const LAYOUT_STRIDE = 10;

/** One def's decoded layout — footprint (tiles), logical anchor (0..1), legacy sprite
 *  size (tiles — superseded by span), sprite pivot (0..1), frame span (pow2 tiles) and
 *  pre-atlas sprite scale (def-frame-anchors P5). */
export interface ThingLayout {
  fw: number;
  fh: number;
  ax: number;
  ay: number;
  size: number;
  sx: number;
  sy: number;
  span: number;
  scw: number;
  sch: number;
}

/** The all-default layout the Rust side also emits for a prim-less def — used when a
 *  `defId` falls outside the table (e.g. before content has loaded). */
const DEFAULT_LAYOUT: ThingLayout = { fw: 1, fh: 1, ax: 0.5, ay: 0.5, size: 1, sx: 0.5, sy: 0.5, span: 1, scw: 1, sch: 1 };

/** Slice a def's layout out of the stride-10 table by its 1-based `defId`. */
export function readLayout(table: Float64Array, defId: number): ThingLayout {
  const base = (defId - 1) * LAYOUT_STRIDE;
  if (base < 0 || base + LAYOUT_STRIDE > table.length) return DEFAULT_LAYOUT;
  return {
    fw: table[base],
    fh: table[base + 1],
    ax: table[base + 2],
    ay: table[base + 3],
    size: table[base + 4],
    sx: table[base + 5],
    sy: table[base + 6],
    span: table[base + 7],
    scw: table[base + 8],
    sch: table[base + 9],
  };
}

/** A placed thing: its sprite's world-px box (`size × size`, a square) and the tile ROW
 *  that drives its zIndex. */
export interface ThingPlacement {
  x: number;
  y: number;
  width: number;
  height: number;
  zRow: number;
}

/** Resolve a thing's world-px box + z-row from its top-left tile, layout, and horizontal
 *  flip (a west/mirrored-east facing).
 *
 *  - anchor (world tiles) = `topleft + anchor·footprint` — the logical point the art pins to.
 *  - a flip mirrors the sprite pivot `sx → 1 − sx`, so the same art point stays pinned when
 *    east art is drawn mirrored as west.
 *  - the drawn box is `span × span` tiles (def-frame-anchors P5: the pow2 frame maps onto its
 *    world span; sprite proportions are baked at atlas INGEST via `sprite_scale`). A def with
 *    no real texture (`white`/geo) never passes ingest, so `scaleAtDraw` applies its
 *    `sprite_scale` to the drawn quad instead — same semantic, applied at the only stage the
 *    "sprite" exists.
 *  - sprite top-left = `anchor − pivot·drawn`, → px via {@link SQUARE}.
 *  - z-row = the tile the anchor resides in (a `floor` already rounds a value on a row
 *    boundary toward the larger, bottom-of-screen row), CLAMPED into the footprint's rows.
 *    So a centred anchor on a 2-tall footprint z-sorts as its bottom tile. */
export function placeThing(tileX: number, tileY: number, l: ThingLayout, flipX: boolean, scaleAtDraw = false): ThingPlacement {
  const anchorX = tileX + l.ax * l.fw;
  const anchorY = tileY + l.ay * l.fh;
  const sx = flipX ? 1 - l.sx : l.sx;
  const drawW = l.span * (scaleAtDraw ? l.scw : 1);
  const drawH = l.span * (scaleAtDraw ? l.sch : 1);
  const bottomRow = tileY + Math.max(1, Math.ceil(l.fh)) - 1;
  const zRow = Math.min(bottomRow, Math.max(tileY, Math.floor(anchorY)));
  return {
    x: (anchorX - sx * drawW) * SQUARE,
    y: (anchorY - l.sy * drawH) * SQUARE,
    width: drawW * SQUARE,
    height: drawH * SQUARE,
    zRow,
  };
}
