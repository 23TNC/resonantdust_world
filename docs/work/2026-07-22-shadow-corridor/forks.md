# Forks — 2026-07-22-shadow-corridor

_Decision points + options + which we chose + why. All resolved in the 2026-07-22 design
conversation; the user authored the design, the assessment is the review feedback._

---

## F1 · Caster lookup: per-light LUT vs per-tile buckets — 2026-07-22 (RESOLVED)

- **(a) Per-light LUT** (shadow-bitfield): each light stores its in-range casters. Duplicates casters
  across lights, ~16 MB pre-allocated, and a **hot caster must update every light's list**.
- **(b) Per-tile buckets:** casters bucket into the tile they occupy; the gather finds them by
  walking tiles.

**Decided: (b).** Hot casters become **O(1) re-bucket**, no LUT, no duplication. GPU pays a corridor
walk instead of a flat read — bounded and cheap (see [issues](issues.md)). The keystone of the stream.

## F2 · Sweep direction + shape — 2026-07-22 (RESOLVED)

Sweeping from the **light outward** with a cap is **buggy**: it burns the cap on near-light casters
and can drop a distant tile's real shadower. Sweeping **from the tile toward the light** makes the
64-cap capture the **nearest-tile** casters (the ones that *can* shadow it) and makes break-on-first
optimal. And the region is a **1-tile-wide corridor** along `P→L`, not a 90° cone — casters must be
within ~one caster-width of the line, *constant* width the whole way. **Decided: sweep tile→light,
corridor width 1 tile, nearest-first, break-on-first.**

## F3 · Lights per tile + address space — 2026-07-22 (RESOLVED)

**8 lights/tile** as **8× u16** in one `RGBA32UI` presence texel. u16 ⇒ **65 536-light** address
space, but **O(8) per-texel work** regardless of total. **Decided: 8/tile** (extend presence to 2 px
= 16/tile only if a scene ever needs it). Overflow → nearest-N evict. This is the *saturation cap*
too: a texel can't be shadowed by more lights than reach it, and >8 overlapping shadows are visually
unreadable anyway — so 8 is both the perf bound and the readability bound.

## F4 · Output representation: per-global-light bit vs per-slot value — 2026-07-22 (RESOLVED)

128-bit-per-global-light can't scale past 128 lights. With 8 lights/tile the output is **per presence
slot**: **`u4` per slot** (`0x0` lit … `0xF` opaque), 8 slots = 32 bits. **Decided: per-slot 4-bit.**
Removes the light ceiling **and** enables translucency + penumbra in the same accumulate. Cost: the
value is slot-keyed, so presence changes must dirty + slot assignment must be stable ([I-1](issues.md)).

## F5 · Wide casters: slice into columns vs whole-caster — 2026-07-22 (RESOLVED, revised same day)

- **(a) Slice into 1-tile columns** (initial plan): bucket a wide prim into every tile it spans, each
  tile casting only its column (`slice = tile_col − anchor_col`, sub-frame `frame_x + slice·w`).
- **(b) Whole caster in every spanned tile:** each per-tile entry is the whole caster ref; the gather
  tests the full silhouette; a caster in multiple corridor tiles is cast **once**.

**Initially chose (a); reverted to (b) same day** when the user noted slicing **is clipping-by-tile
and therefore seams** — adjacent column-shadows abut, so coverage double-counts every internal
boundary (dark line) and penumbra softens invented internal edges (false soft seams). (b) never
partitions → no seam, and testing the full silhouette from any one of the caster's tiles is correct.

**Decided: (b) whole-caster.** Multi-tile appearance is handled by **idempotence** in binary (no
work) and a **toward-`P` neighbour check** in coverage/P6+ (see [I-6](issues.md) — 3 tests, stateless,
no stored field). Also simpler (no slice index / `width_in_tiles`) and cheaper (one test per caster,
not per column). Cost: height cull + penumbra distance use the caster center as an approximation.
**Retires the straddler sub-case** — [I-6](issues.md) is now one general rule, not a special case.

## F6 · Height cull — 2026-07-22 (RESOLVED)

A caster shadows `P` only if `0 ≤ δ ≤ (d_P/L.z)·h`. **Decided: gate each candidate on `δ > k·h`
(k precomputed) before the projection + silhouette fetch.** 2 ALU ops, not a "shadow calculation" —
saves the expensive test for radially-implausible casters. Per-part heights make a pawn's shadow fan
for free.

## F7 · Area lights (penumbra) via a second radius — 2026-07-22 (RESOLVED)

Torches are area lights; softness is driven by **physical emitter size**, not reach. **Decided: add
`emitter_radius`** (distinct from `reach`), fake penumbra by softening the silhouette sample with
`penumbra_width = emitter_radius·(tile→occluder)/(light→occluder)` (both distances free from the
sweep). `emitter_radius = 0` ⇒ hard shadow. Doubles as the mood/softness dial.

## F8 · Caster cap: 128 vs 64 — 2026-07-22 (RESOLVED)

With the 1-tile corridor + height cull, few casters survive to a full test. **Decided: cap at 64.**
It's a *guarantee*, not an expected load — it converts the inner loop into a bounded one so the dirty
budget can make honest promises. Only affects the rare failure ([I-2](issues.md)), so it's a safe
quality dial (raise/lower with hardware).

## F10 · Shadow radius = `reach − 1`; field named `reach` — 2026-07-22 (RESOLVED)

The shadow-relevant radius is **`reach − 1` tile**, not `reach`. The outer 1-tile ring
(`[reach−1, reach]`) is dead weight both ways: **receiving** tiles there are falloff-dark (shadows
imperceptible), and **casters** there project their shadows *beyond* reach onto unlit tiles. Because
disc area is quadratic, trimming one tile removes `(2·reach − 1)/reach²` of it — **~44% at reach 4,
~36% at reach 5** — from every light's presence + sweep + culls, for free. **Decided (user):
`reach − 1`.** Caveat: falloff-curve dependent (at `reach−1` a light may still be ~`1/reach`
intensity) — a good aggressive default; dial if a ring ever shows. Applies to `light_presence_cold`
(which tiles a light reaches *for shadow*), the corridor sweep bound, and the range culls.

**Naming:** the light's illumination-range field is **`reach`** (renames the shadow-bitfield layout's
`radius`). `emitter_radius` (F7) is unaffected — it's a genuinely different quantity (physical source
size for penumbra, not a range).

## F9 · Corridor width: 1 tile — 2026-07-22 (RESOLVED)

The tile→light sweep is a **1-tile-wide** rasterized path (thin, one tile per step), defined by a
single `onCorridor(tile, P, L)` predicate that **both** the sweep enumeration and the dedup query.
**Decided (user): 1 wide.** Consequences: `S = prim ∩ corridor` is a 1-wide connected path with a
**unique `P`-most tile**, so the dedup ([I-6](issues.md)) is a flat **3-neighbour** toward-`P` check
with **no tie-break** (the ~5-check thick-corridor case is moot). Accepted cost: a thin path can
graze past a caster whose base tile it clips only at a corner → a rare missed shadow (same tier as the
64-cap miss, [I-2](issues.md)); widen only if it ever shows.
