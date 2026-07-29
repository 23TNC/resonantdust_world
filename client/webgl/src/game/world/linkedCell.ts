//! The linked-atlas cell formula (build-walls P0, D1) — which cell of a 4×4 autotile atlas a
//! tile draws, from its four cardinal same-kind neighbors. Derived from the user's 16-row
//! table (2026-07-28) and pinned by {@link LINKED_CELL_TABLE_16} below:
//!
//!     atlas x = N + 2·E            (0..3)
//!     atlas y = 3 − (S + 2·W)      (0..3 — row 3 is the BOTTOM of the atlas)
//!
//! The resolver's cell index is row-major from the TOP (`cell = cy·cols + cx`,
//! `TextureResolver.cellFrame`), and the user's y already counts down from the top, so the
//! index is simply `y·4 + x`. Consumers: tile expansion (P1 — the world's walls), the
//! blueprint preview (P2 — the same formula against the preview shape).

/** Neighbor mask bits — 1 where the SAME wall kind stands in that cardinal neighbor. */
export const N = 1, E = 2, S = 4, W = 8;

/** The 4×4 linked-atlas cell (resolver index, row-major from the top) for a neighbor mask. */
export function linkedCell(mask: number): number {
  const x = (mask & N ? 1 : 0) + (mask & E ? 2 : 0);
  const y = 3 - ((mask & S ? 1 : 0) + (mask & W ? 2 : 0));
  return y * 4 + x;
}

/** The user's authored table, verbatim — (x, y) per neighbor set. `assertLinkedCellTable`
 *  checks the formula against every row; it runs once in dev (contentBoot) and throws on
 *  the first mismatch, so a drifted formula can never ship silently. */
export const LINKED_CELL_TABLE_16: ReadonlyArray<{ mask: number; x: number; y: number }> = [
  { mask: 0, x: 0, y: 3 },             // lone wall
  { mask: S, x: 0, y: 2 },             // south neighbor
  { mask: W, x: 0, y: 1 },             // west
  { mask: S | W, x: 0, y: 0 },         // south + west
  { mask: N, x: 1, y: 3 },             // north
  { mask: N | S, x: 1, y: 2 },         // north + south
  { mask: N | W, x: 1, y: 1 },         // north + west
  { mask: N | W | S, x: 1, y: 0 },     // north + west + south
  { mask: E, x: 2, y: 3 },             // east
  { mask: S | E, x: 2, y: 2 },         // south + east
  { mask: W | E, x: 2, y: 1 },         // west + east
  { mask: E | W | S, x: 2, y: 0 },     // east + west + south
  { mask: E | N, x: 3, y: 3 },         // east + north
  { mask: E | N | S, x: 3, y: 2 },     // east + north + south
  { mask: E | N | W, x: 3, y: 1 },     // east + north + west
  { mask: N | E | S | W, x: 3, y: 0 }, // all four
];

/** Verify the formula against the authored table (all 16 rows). Throws on mismatch. */
export function assertLinkedCellTable(): void {
  for (const row of LINKED_CELL_TABLE_16) {
    const got = linkedCell(row.mask);
    const want = row.y * 4 + row.x;
    if (got !== want) {
      throw new Error(`linkedCell(${row.mask}) = ${got}, table says (${row.x},${row.y}) = ${want}`);
    }
  }
}
