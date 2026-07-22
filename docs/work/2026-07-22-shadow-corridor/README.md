# Work — 2026-07-22-shadow-corridor (per-tile casters + tile→light corridor sweep)

_Opened 2026-07-22. Evolve the shadow gather off the **per-light caster LUT** onto **per-tile caster
buckets** swept as a **1-tile-wide corridor from the shadowed tile back toward the light**. Adds a
**radial height cull**, a **4-bit coverage** output (translucency + fake penumbra in one accumulate),
an **8-lights-per-tile** presence list (u16 → unlimited lights), and a **cold/hot split by dirty**.
Reuses the `2026-07-21-shadow-bitfield` gather shell, `/overlayRT` decode, and cold data textures.
Component: `client/webgl`._

## Why this supersedes the LUT (the core shift)

`2026-07-21-shadow-bitfield` finds a texel's shadow casters through a **per-light LUT**: each light
stores a list (up to 256) of the casters in its reach, and the gather loops that list. That LUT
**duplicates** casters across every light that sees them, costs **~16 MB pre-allocated**, and — the
real pain — a **moving (hot) caster must update every light's list**.

The shift: **bucket casters by the tile they sit in** (like lights already are), and find a texel's
casters by **walking from the texel toward the light**, reading each tile's ≤8 casters as you go.
Consequences:

- **Hot casters become O(1)** — a mover just re-buckets (drop from old tile, add to new); no light
  lists to touch. This is the whole reason the shift is worth it.
- **No LUT** — kills the 16 MB and the per-light duplication.
- The GPU trades a flat LUT read for a **corridor walk** — more per-fragment work, but bounded and
  well within budget (see [issues.md](issues.md) for the numbers).

## The architecture (all of it, with the reasoning — this is the "don't forget how it works")

### 1 · Per-tile lights, per-tile casters (both ≤ 8)

- **Presence** holds, per tile, up to **8 lights** as **8× `u16` global light indices** (one
  `RGBA32UI` texel = 128 bits = 8× u16). u16 ⇒ a **65 536-light address space**, but you only ever
  process **8 per tile**, so **per-texel shader work is O(8) regardless of total light count**
  ([F3](forks.md#f3)). >8 lights want a tile → keep the nearest, evict the rest (nearest-N). A light
  reaches a tile *for shadow* only within **`reach − 1`** ([F10](forks.md#f10)) — the outer ring is
  falloff-dark, so trimming it drops ~40% of every light's shadow tiles for free.
- **Casters** get a parallel per-tile list (**double the presence texture** → 8× `u16` caster
  refs/tile). 8 covers the common tile: 1 primary object + 1 secondary + 1 pawn (**head/body/2
  hands = 4 sub-casters**) = 6, and the `cast_shadows` bit keeps most secondaries out entirely.

### 2 · The corridor sweep — from the **tile**, toward the **light** (not light→tile)

To shadow texel `P` from light `L`, march from `P` **toward** `L`, reading each tile's casters,
testing each, **break on the first that fully occludes** (`0xF`), cap at **64**.

- **Direction matters and fixes a real bug.** Sweeping from the *light* outward burns the cap on
  near-`L` casters (irrelevant to a distant `P`) and can exhaust it before reaching the caster next
  to `P` that actually shadows it → dropped shadows for far tiles. Sweeping **from `P`** makes the
  cap capture the **nearest-`P`** casters — exactly the ones that can shadow it — and makes
  break-on-first optimal (most-likely tested first) ([F2](forks.md#f2)).
- **It's a corridor, not a 90° cone.** The casters that can shadow `P` sit within ~one caster-width
  of the `P→L` line, *constant* width the whole way (a near caster subtends a wide *angle* but a
  tiny lateral offset). So sweep a **1-tile-wide** corridor, not a fan — tighter, and it doesn't
  waste the cap on side-casters ([F2](forks.md#f2)).
- **Asymmetric cost (keep in mind):** a **shadowed** texel breaks on the 1st caster → nearly free;
  a **lit** texel must exhaust the corridor to *prove* nothing occludes it → that's the expensive
  case the corridor bound + height cull exist to bound.

### 3 · Whole-caster buckets — no slicing, no seams ([F5](forks.md#f5))

Casters wider than a tile are **bucketed whole into every tile they span** (footprint from
`prim_width`); each per-tile entry is the **whole caster ref**, not a column. The gather tests the
**full silhouette** — correct from *any* of the caster's tiles (P is shadowed iff under any part of
the full projection), and the corridor reaches the caster because its tiles lie on the P→L line.

**Why not slice into columns** (the earlier plan, reverted 2026-07-22): slicing is *clipping-by-
tile*, so it seams. In coverage, adjacent column-shadows abut and the boundary double-counts (dark
line at *every* internal boundary); with penumbra it invents internal edges that each get softened
(false soft seams through solid interior). Whole-caster **never partitions**, so there is no seam.

Handling a caster that appears in **multiple corridor tiles** (a wide object along the corridor, or a
≤1-tile straddler mid-move — [I-6](issues.md)):

- **Binary (P1–P5): nothing to do** — testing the whole caster twice is idempotent (same bit).
- **Coverage/penumbra (P6+): a stateless dedup, no stored field** — a tile **casts iff none of its
  ≤3 neighbours in the toward-`P` octant** (backward along the corridor) is in `prim ∩ corridor`
  (bbox test + `onCorridor`), else defers. `S = prim ∩ corridor` is a connected path *along* the
  corridor, so the predecessor is always a toward-`P` neighbour and the unique `P`-most tile casts —
  any prim shape. Plain `u16` bucket entry, **no neighbour-list read**. Bound: **3** — corridors are
  1-tile wide ([F9](forks.md#f9)), so no tie-break, never N−1. Aim toward `P`, *not* the anchor (which
  fails on a diagonal corridor). See [I-6](issues.md) for the failure of the rejected attempts + CPU flow.

Cost: the height cull + penumbra distance use the caster **center** as an approximation instead of a
per-column position — fine for coarse gates. (`prim_width`/`prim_height` already carry the footprint;
no slice index, no `width_in_tiles`.)

### 4 · The height cull — a cheap radial gate, **not** a shadow calculation

A caster shadows `P` only if `P`'s distance from the light falls in the caster's shadow band. With
`δ = d_P − d_c` (how far the caster sits toward the light from `P`, = how far you've swept) and
`k = d_P / L.z` **precomputed per (texel, light)**:

```
caster can shadow P  ⇔  0 ≤ δ ≤ k · h        // h = caster height
skip if δ > k·h      // one multiply, one compare
```

Short caster far from `P` → skipped; tall caster reaches from farther → tested. **This is not a
shadow calculation** — it's 2 ALU ops on values already in hand, run **before** the expensive
projection-inversion + silhouette texture fetch, so it spends the 64-budget only on casters that can
plausibly reach `P`. Order: read def (for `h`) → height cull → (survivors only) project + sample.
Nice free consequence: a pawn's **head casts farther than its hands** (per-part height) → the figure
shadow **fans** with no special code ([F6](forks.md#f6)).

### 5 · Output = 4-bit coverage per light **slot** (not 1 bit per global light)

8 lights/tile ⇒ the output is **per presence-slot**, not per-global-light: **`0x0` = lit, `0xF` =
fully occluded**, 4 bits × 8 slots = **32 bits** (one `u32`). This drops the per-light 128-bit
ceiling *and* buys **translucency + penumbra** for free ([F4](forks.md#f4)):

- **Translucency** (glass, foliage): accumulate the caster's **alpha** (already in the surface/albedo
  map `cover()` samples) instead of a hard threshold; `occlusion += coverage`, clamp, **break at
  `0xF`**. Multiplicative transmittance is order-independent, so nearest-first is fine.
- **Penumbra** (fake area light): soften the `cover()` sample by a **blur radius = penumbra_width**
  (a few taps, or a **blurred silhouette mip** — one read). Both effects feed the **same 4-bit
  accumulate**, so one loop does both.

> **Slot stability caveat:** the 4-bit value is keyed by **presence slot (0–7)**, not global light.
> Any change to a tile's presence set must **dirty** that tile, and slot assignment must be **stable**
> (keep a present light in its slot; fill only freed slots) or persistence silently corrupts. See
> [issues.md](issues.md).

### 6 · Area lights = a second radius (softness), not reach

Torches are physically area lights (a finite flame). Penumbra width is driven by the **emitter's
physical size**, a **new field distinct from `reach`**:

```
penumbra_width ≈ emitter_radius × (tile→occluder dist) / (light→occluder dist)
```

Both distances fall out of the tile→light sweep. `emitter_radius = 0` ⇒ penumbra 0 ⇒ hard shadow
(a true point), which is the proof the term is *size*, not reach (a point light is sharp at any
reach). So each light carries **two radii**: `reach` (how far) + `emitter_radius` (how soft) — the
latter doubles as the mood/softness dial ([F7](forks.md#f7)).

### 7 · Cold / hot split by dirty

Two fields (`cold-shadow`, `hot-shadow`), same layout. A (light, caster) contribution is **cold iff
both are cold**, else **hot**. Global light/prim indices are **stable across hot↔cold migration**
(the flag lives in the record, not presence), so presence never rewrites on settle. Dirty routing on
change; **hot is retained + dirty-gated, not cleared every frame** — full-tile recompute writes the
whole mask, so **no ping-pong**. A **dirty budget** (measured resolve time → max tiles/frame) makes
the worst case schedulable. Migration + **settle must be symmetric** (see [issues.md](issues.md)).

## Relationship to other streams

- **Reuses** `2026-07-21-shadow-bitfield`: the single-pass **gather shell** + `discard`-on-clean
  dirty gating, the `/overlayRT` decode, `prim_data` / `prim_definition_data`, and the world-space
  toroidal RT geometry. **Replaces** its per-light **LUT** (`light_data` rows 1–32) with per-tile
  caster buckets, and its 128-bit per-light presence/output with the 8-slot u16 / 4-bit scheme.
  When this lands, shadow-bitfield's LUT half is superseded (mark it then, not now).
- **Feeds** the lighting pass (the per-light mask it consumes) — unchanged consumer, richer value
  (4-bit coverage instead of 1 bit).
- Layout changes land in **`docs/VARIABLES.md` §Cold shadow data textures** during implementation
  (P5/P6 change presence width, output width, prim_data width field) — not edited yet; the live
  layout is still the shadow-bitfield one.

## Build order

Each phase is verifiable on `/overlayRT shadow-cold` before the next. See [todo.md](todo.md). The
natural spine: prove the **cold path end-to-end at 1 bit/light** (per-tile whole-caster buckets +
corridor + height cull, keeping the current output), **then** layer the 8-slot representation, then
4-bit coverage, then penumbra, then the hot/dirty split — each an independent upgrade on a working
base.
