//! The N×N SQUARE grid the viewport caches + dirty-tracks on. Pure geometry — no
//! PIXI, no game state. Squares are an axis-aligned lattice in WORLD PIXELS,
//! `N × N` (N = {@link SQUARE}). Every primitive's world AABB maps deterministically
//! to the small set of squares it overlaps — the unit the albedo map bakes, caches
//! and copies.
//!
//! This is the rect-grid descendant of the old quadtree: the render texture is a
//! FIXED grid of square slots that never pans; as the anchor moves, world squares
//! wrap onto physical slots via {@link mod} (a torus), and only the freshly-exposed
//! strip re-bakes. See `SquareCache`.

/** Square edge in world px. The grid unit — all map squares are this size, and the MAXIMUM art size:
 *  a slot cannot show more than {@link SQUARE} px of a tile, so there is never a reason to author
 *  larger (work `2026-07-26-textile-slot`). Raised 64 → 128 with the fixed slot grid.
 *
 *  **This is the ART dial and ONLY the art dial** (work `2026-07-28-square-128`). It used to be the
 *  lighting dial too, via `TEXTILE_SQUARE = SQUARE`, and that identity is what forced the 2026-07-27 A/B
 *  to buy a cheaper lighting pass by paying with art resolution. The lighting now sits on its own pinned
 *  {@link TEXTILE_LIGHT}, so moving this changes how sharp the world looks and nothing else:
 *
 *    SQUARE  ->  UNIT = SQUARE/16 (px per world unit; units-per-tile is FIXED at 16, so every tile+unit
 *                position on the wire and in the data records is untouched)
 *            ->  TEXTILE_SQUARE   (albedo/normal/surface/zdepth, cold AND warm — 8 maps)
 *            ->  BASE_LOD_PX in TextureResolver — the cap on how much art a sprite may fetch
 *            ->  REFERENCE_W/H, SLOT_PW/PH, the lod ladder
 *        NOT ->  the lightmap, the receiver map, the shadow map, or any tile map
 *
 *  At 128 the art maps cost ~321 MiB against ~76 at 64 (measured P0). That is the real price, and it
 *  buys back the resolution: `BASE_LOD_PX` goes 64 -> 128, so every sprite steps up one LOD. The corpus
 *  already carries it — 0 of 27 manifest entries are short of 128 and the wolf's master is 512, which is
 *  why it read as mush at 64. */
export const SQUARE = 128;

/** World UNIT in px — `UNIT = SQUARE/16`; 16 units per tile edge. Shadow math is in units.
 *  Units-per-tile is FIXED at 16, so this tracks `SQUARE` and every tile+unit position — which is
 *  what the wire and the data records store — is unaffected by the `SQUARE` change. */
/** Units per tile — a WORLD invariant, not a dial. The rework names it explicitly rather than
 *  leaving 16 as a bare literal in a dozen derivations. */
export const UNITS_PER_TILE = 16;
export const UNIT = SQUARE / UNITS_PER_TILE;

// ── Textile resolutions — the shared toroidal TILE grid (map-model.md, lighting rebuild) ──
// A TEXTILE is one texel of a map. EVERY map — data textures and full-res — shares the ONE
// `cols × rows` TILE window and wraps by TILES (`mod(worldTile, cols/rows)`); each map picks one
// of these per-tile-edge resolutions R and is sized `cols·R × rows·R`. `SQUARE` was once the ONLY
// dial — that identity is what made the 2b1025a A/B pay for a lighting win with art resolution —
// but the light map now has its own pinned {@link TEXTILE_LIGHT}. So raising `SQUARE` sharpens the
// textile_square (art) maps ALONE: textile_unit stays 16 textiles/tile and the lightmap does not move.
/** 1 textile / tile — lights, caster buckets, presence, dirty. Positions in TILES (+ anchor). */
export const TEXTILE_TILE = 1;
/** `SQUARE/UNIT` = 16 textiles / tile (one per unit) — the shadow map. Positions in UNITS. */
export const TEXTILE_UNIT = SQUARE / UNIT;
/** `SQUARE` textiles / tile (one per px) — albedo, normal, surface, zdepth. Positions in PX. */
export const TEXTILE_SQUARE = SQUARE;

/** LIGHTING textiles / tile — the fine lightmap pair and the fine receiver map. **PINNED at 64; it does
 *  NOT track {@link SQUARE}.**
 *
 *  It used to, because these maps were sized from `TEXTILE_SQUARE`, and that identity made ONE constant
 *  steer two unrelated things: how sharp the ART is, and how many texels the LIGHTING pass shades. That
 *  is why the 2026-07-27 A/B could only buy a cheaper lighting pass by paying with art resolution
 *  (`2b1025a`) — the two effects were never coupled, just co-located behind one name.
 *
 *  Split, both are free to sit where they belong. MEASURED before splitting (work `2026-07-28-square-128`
 *  P0), at 64 vs 128 with `SQUARE` held at 64 so only the lighting resolution moved:
 *
 *    lighting draw   reach 4: 0.124 -> 0.344 ms   reach 8: 0.286 -> 0.620   reach 12: 0.253 -> 0.790
 *    gather draw     unchanged within the harness's spread; art maps not resized at all
 *
 *  So 4x the lighting texels costs 2.2-3.1x the time and buys nothing the display can show: the blit
 *  samples this NEAREST and the detail the eye reads comes from the albedo. Three maps ride it —
 *  `coldLightRT` + `hotLightRT` (rgba32float) and `receiverFineRT` (r32uint) — so the pin is worth
 *  **216 MiB** at `SQUARE = 128`, which is most of what that A/B actually freed.
 *
 *  Raising it is a deliberate quality/cost decision, never a side effect of moving `SQUARE`. Keep it a
 *  power-of-two multiple of {@link TEXTILE_UNIT} so `FINE_RATIO` stays an integer. */
export const TEXTILE_LIGHT = 64;

/** Tiles per zone edge — mirrors `resonantdust_codec::packed::ZONE_DIM`. A zone is
 *  `ZONE_DIM × ZONE_DIM` tiles. */
export const ZONE_DIM = 16;

/** Zones per region edge — mirrors `resonantdust_codec::packed::REGION_DIM`. A region is
 *  `REGION_DIM × REGION_DIM` zones, i.e. `REGION_DIM · ZONE_DIM` tiles per edge. */
export const REGION_DIM = 16;

/** Slack SLOTS kept baked off each edge of the visible area, so a pan reaches finished content
 *  before the screen edge does (and a small jiggle never re-bakes). */
export const OVERSCAN = 2;

// ── The fixed slot grid (work `2026-07-26-textile-slot`; VARIABLES is authoritative) ──
// Every textile map is sized in TILES, never in screen resolution, and NEVER CHANGES SIZE. A slot
// holds 1 tile at PARTITION LEVEL 0 and `2^level × 2^level` tiles at level k, so the texture is
// constant while the world it covers grows 4× per step. That is what stops the lightmap tracking
// zoom (it was 176 MB at zoom 0.25) and what makes gameplay identical on every monitor.
// (one-resolution P4: this concept USED to share the name "lod" with the deleted atlas ladder —
// the PARTITION LEVEL is the survivor, renamed.)
/** Visible slots — the world every player sees at a given partition level, regardless of monitor. */
export const VISIBLE_X = 28;
export const VISIBLE_Y = 12;
/** Total slots including {@link OVERSCAN} on each side. Chosen so BOTH are POWERS OF TWO: the toroidal
 *  wrap modulus is `SLOTS << level`, which stays pow2 at every level, so `mod(wc, cols)` compiles to a
 *  bitmask instead of an integer division. Purely a performance property — the grid is correct at any
 *  size, since a slot subdivides into `2^level` tiles regardless. */
export const SLOTS_X = VISIBLE_X + 2 * OVERSCAN; // 32
export const SLOTS_Y = VISIBLE_Y + 2 * OVERSCAN; // 16
/** Partition levels 0..2 — `SQUARE` (128) down to 32 px per tile, matching `ZOOM_MIN` 0.25. Fits
 *  `u2`, which leaves room to restore level 3 without a layout change. */
export const PARTITION_LEVELS = 3;
export const PARTITION_MAX = PARTITION_LEVELS - 1;
/** The reference render target: the visible slots at level 0. Standardising on this (rather than the
 *  player's panel) is what equalises gameplay across monitors — 4K magnifies ~1.5×, 1080p minifies. */
export const REFERENCE_W = VISIBLE_X * SQUARE; // 3584
export const REFERENCE_H = VISIBLE_Y * SQUARE; // 1536

/** Tiles per slot edge at `level` — 1, 2, 4, 8. */
export function tilesPerSlot(level: number): number {
  return 1 << level;
}
/** Texels per TILE edge at `level` for a map with `texelsPerSlot` (SQUARE / TEXTILE_UNIT / 1). */
export function tileTexels(texelsPerSlot: number, level: number): number {
  return texelsPerSlot >> level;
}
/** The COVER fit — the scale that keeps the viewport entirely inside the visible slots.
 *  `max`, not `min`: `min` would fit the whole grid and expose overscan at the edges. */
export function coverScale(screenW: number, screenH: number): number {
  return Math.max(screenW / REFERENCE_W, screenH / REFERENCE_H);
}
/** The partition level a zoom sits in: level 0 covers `[1, 2)`, level 1 `[0.5, 1)`, … Clamped. */
export function partitionForZoom(zoom: number): number {
  if (!(zoom > 0)) return 0;
  return Math.min(PARTITION_MAX, Math.max(0, Math.ceil(-Math.log2(zoom))));
}

/** Gutter (px) baked around every slot's interior. The bake over-renders neighbour
 *  content into it; the display samples only the gutter-protected interior, so
 *  bilinear never reaches across a slot boundary (no seam). 2 is the safe default. */
export const PAD = 2;

/** Slot PITCH in the padded atlas — interior + a {@link PAD} gutter on each side.
 *  INTEGER, so slot `sx` sits at the whole-texel atlas position `sx·PW` (no
 *  fractional blit, no drift). */
export const SLOT_PW = SQUARE + 2 * PAD;
export const SLOT_PH = SQUARE + 2 * PAD;

/** An inclusive range of square cells `[col0..col1] × [row0..row1]`. */
export interface SquareRange {
  col0: number;
  row0: number;
  col1: number;
  row1: number;
}

/** True modulo (always non-negative) — the torus wrap maps a world square col/row
 *  to its physical composite slot via `mod(worldCol, COLS)`. JS `%` keeps the sign,
 *  which would index out of the composite for negative world coords. */
export function mod(n: number, m: number): number {
  return ((n % m) + m) % m;
}

/** World-px top-left of square cell `(col, row)` — the lattice the composite bakes
 *  from and the display places quads at. */
export function squareWorldX(col: number): number {
  return col * SQUARE;
}
export function squareWorldY(row: number): number {
  return row * SQUARE;
}

/** The square cell a world point falls in (floor division). */
export function worldToSquare(x: number, y: number): { col: number; row: number } {
  return { col: Math.floor(x / SQUARE), row: Math.floor(y / SQUARE) };
}

/** The inclusive square range a world-space AABB `[x0,y0]–[x1,y1]` covers. The
 *  `-ε` on the far edges keeps an AABB whose right/bottom sits exactly on a square
 *  boundary from claiming the next (empty) square. */
export function squaresForAABB(x0: number, y0: number, x1: number, y1: number): SquareRange {
  const eps = 1e-4;
  return {
    col0: Math.floor(x0 / SQUARE),
    row0: Math.floor(y0 / SQUARE),
    col1: Math.floor((x1 - eps) / SQUARE),
    row1: Math.floor((y1 - eps) / SQUARE),
  };
}

/** Whether two ranges are identical (footprint unchanged → no relink). */
export function sameRange(a: SquareRange, b: SquareRange): boolean {
  return a.col0 === b.col0 && a.row0 === b.row0 && a.col1 === b.col1 && a.row1 === b.row1;
}
