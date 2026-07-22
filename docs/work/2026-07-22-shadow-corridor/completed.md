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

## P6 · 4-bit coverage — 2026-07-22 ✓ (partial — dedup pending)

- `inShadow` → **`shadowCover`**: returns the silhouette **alpha** (0..1), not a hard `≥0.5` bool.
  `casterCover` carries it through the height cull. The gather **accumulates** `cov += casterCover`,
  clamps, **breaks at full (1.0)**, and packs a **4-bit nibble per light** (bit `(k&7)*4` of channel
  `k>>3`, ≤32 lights). Overlay decodes the nibbles → per-light colour × coverage, `alpha = coverage`.
- **Verified**: 6-light coverage renders correctly, no seams at this scale. The **toward-P dedup**
  ([I-6](issues.md)) is **not yet wired** — deferred until a double-count seam actually appears (base-
  line bucketing + break-at-full keeps it invisible so far). This is 4-bit-per-light on the existing
  light-bitfield presence (≤32 lights); the full u16 **P5** (65 536-light space) rides separately.

## P7 · Penumbra (fake area light) — 2026-07-22 ✓ (constant emitter)

- `shadowCover` now blurs the silhouette sample by a **PCSS-style** radius
  `EMITTER_R·|P−A|/|A−L|` (occluder→receiver / light→occluder) — a **5-tap cross** in card-UV space.
  Near occluders stay sharp, far ones feather; `EMITTER_R = 0` ⇒ hard.
- **Verified**: shadow edges are visibly soft and widen with distance — the firelight look. `EMITTER_R`
  is a **constant** for now; the per-light `emitter_radius` field (distinct from `reach`,
  [F7](forks.md#f7)/[F10](forks.md#f10)) is the remaining refinement.
- **Fix (2026-07-22): clamped the penumbra blur** to ≤0.25 UV. It was unbounded
  (`EMITTER_R·|P−A|/|A−L|`) → exploded at far shadow tips, sampling ~the whole silhouette and smearing
  coverage far past the real tip (the "shadow too far" the user caught). Clamp bounds it to a fraction
  of the sprite.

## Debug overlay: shadow colours now match the light gizmos — 2026-07-22 ✓

- Overlay `hueColour(k)` → **`lightColour(k)`** = the same `LIGHT_COLORS` the gizmo rings use, so a
  light's shadow reads as the *same* colour as its ring (attribution was scrambled before — golden-
  ratio hue ≠ gizmo colour, which made per-light bug-hunting confusing). Debug: first 6 lights.
- Also: `MAX_LIGHTS` is now `number`-typed so `MAX_LIGHTS = 1` (isolate one light) doesn't trip TS's
  constant-comparison check; the seed isolates the k=4 (teal) ring position when set to 1.

_Not built yet: **P5** (u16/per-slot for >128 lights), **P6 dedup** (toward-P, if a seam shows), **P8**
(cold/hot split + dirty budget). P1′ = delete the dead `light_data` LUT rows. Per-light
`emitter_radius`, and the `radius`→`reach` field rename, ride with P5's layout rewrite._
