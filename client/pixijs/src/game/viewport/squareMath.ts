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

/** Square edge in world px. The grid unit — all map squares are this size. */
export const SQUARE = 64;

/** Tiles per zone edge — mirrors `resonantdust_codec::packed::ZONE_DIM`. A zone is
 *  `ZONE_DIM × ZONE_DIM` tiles. */
export const ZONE_DIM = 16;

/** Zones per region edge — mirrors `resonantdust_codec::packed::REGION_DIM`. A region is
 *  `REGION_DIM × REGION_DIM` zones, i.e. `REGION_DIM · ZONE_DIM` tiles per edge. */
export const REGION_DIM = 16;

/** Slack squares kept baked OFF each edge of the viewport, so a pan reaches
 *  finished content before the screen edge does (and a small jiggle never
 *  re-bakes). "A number of these squares off each edge." */
export const OVERSCAN = 2;

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
