# Completed — 2026-07-22-shadow-corridor

_Items move here from [`todo.md`](todo.md) when done **and** verified on `/overlayRT shadow-cold`._

---

## P1 · Per-tile caster buckets — 2026-07-22 ✓

- `ShadowGather.buildCasters()` builds a per-tile **caster-bucket** texture (`cols×rows` `RGBA32UI` =
  8× `u16` prim indices), bucketing each standing caster's `prim_data` index into its **base-line**
  tiles (anchor row × width cols) that the window covers, ≤8/tile. Allocates def+prim via the existing
  `ColdShadowData` (idempotent/cached) and flushes.
- Rebuilt each frame for now (cheap for the static scene); the O(1) re-bucket-on-move lands with the
  hot tier (P8). Bucket entry is a **plain `u16`** (no seq/slice field, per [F5](forks.md#f5)/[I-6](issues.md)).

## P2 · Corridor sweep (tile → light) — 2026-07-22 ✓

- Gather rewritten: the per-light **LUT loop is gone**; instead a **1-tile Bresenham march** from the
  texel's tile toward the light reads each corridor tile's ≤8 caster refs, tests
  `casterHits`/`inShadow`, **breaks on first hit**, caps total tests at **64** ([F8](forks.md#f8)).
  `light_data` row 0 (records) still drives the light list + presence cull; the LUT rows are now unread.
- **Verified** at `/overlayRT shadow-cold`, zoom 2, tile (100,50): tree-shaped shadows radiate from
  the light exactly as the LUT produced — parity confirmed. Typecheck clean, no shader-compile errors.
- Whole-caster (not sliced): 2-tile-wide conifers cast **seamless** shadows found from any base tile
  the corridor hits ([F5](forks.md#f5)) — the binary-level half of P3.

_Not built yet: P3 (center-approx check waits on P4), P4 height cull, P5 8-slot, P6 coverage+dedup,
P7 penumbra, P8 cold/hot. The `light_data` LUT rows are dead but not yet deleted (a P1 cleanup)._
