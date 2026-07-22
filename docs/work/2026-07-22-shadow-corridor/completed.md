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

## P4 · Height cull (radial gate) — 2026-07-22 ✓

- `casterHits` gates each candidate on `0 ≤ (dP − dC) ≤ (dP/L.z)·H` (P/caster distance from the light)
  **before** the projection + silhouette fetch — 3 ALU ops, not a "shadow calculation". `H` (full
  height) over-estimates the tilted extent → conservative, never drops a real hit.
- **Verified**: shadows pixel-identical to pre-cull (the cull only skips casters that can't reach the
  texel). Typecheck clean.

## Multi-light verified (partial P5) — 2026-07-22 ✓

- `MAX_LIGHTS` 1 → **6** (the ring around tile (100,50), the plan's debug seed). Verified on
  `/overlayRT`: 6 lights each cast tree-shaped shadows radiating from their own position, per-light
  colours, combining on overlap. The corridor path is per-(texel, light), so it scales with no change.
- This is the multi-light path on the **existing 128-bit-per-light output** (≤128 lights). The full
  **P5** — u16 presence list + 8-bit per-slot output for the 65 536-light address space — is still to
  do; it's only needed to go *past* 128 lights and to make the coverage/penumbra output per-slot.

_Not built yet: **P5** (u16/per-slot rep), **P6** (coverage+dedup), **P7** (penumbra), **P8**
(cold/hot). P1′ = delete the dead `light_data` LUT rows. P3's center-approx sub-check rides with P6/P7._
