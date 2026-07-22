# Todo — 2026-07-22-shadow-corridor (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified on
`/overlayRT shadow-cold`. Phases run in order; each is verifiable on its own. See
[`README.md`](README.md) for the architecture + reasoning, [`forks.md`](forks.md) for decisions,
[`issues.md`](issues.md) for accepted limits + deferrals._

---

## P1 · Per-tile caster buckets (replace the per-light LUT)

- [ ] A per-tile **caster list** texture (parallel to `light_presence_cold`): **8× `u16` caster
      refs/tile** into `prim_data` (plain refs — no seq field, [I-6](issues.md)). CPU-built: bucket
      each `cast_shadows` prim into every **bounding-box** tile it covers as a whole ref (not a column
      — no slicing, [F5](forks.md#f5)); footprint from `prim_width`/`prim_height`. Update membership on
      tile-boundary crossings; write the prim's x/y every frame it moves (independent streams).
- [ ] Hot-caster path: on a caster move, **re-bucket** (drop old tile, add new) — O(1), the whole
      point. (Cold casters bucket once.)
- [ ] Delete the `light_data` rows 1–32 LUT + its CPU maintenance once the sweep (P2) reads buckets.

## P2 · The corridor sweep (tile → light), keeping the current 1-bit output

- [ ] Define **`onCorridor(tile, P, L)`** — the **1-tile-wide** ([F9](forks.md#f9)) rasterized path
      from `P` to `L`. **One predicate**, used to drive the sweep enumeration *and* (later) the dedup
      — they must agree, so there is a single source of truth.
- [ ] Gather change: for texel `P`, for each of its lights, **march from `P` toward `L`** along the
      corridor, reading each tile's caster refs, testing point-in-projected-silhouette, **break on
      first hit**, **cap at 64** ([F8](forks.md#f8)). Nearest-`P` first (naturally, marching from `P`).
- [ ] Keep the existing 128-bit per-light output for now — verify the corridor finds the same
      shadows the LUT did (overlay should match pre-change).

## P3 · Wide-caster correctness (whole-caster, no slicing — [F5](forks.md#f5))

- [ ] Verify a **wide** caster (multi-tile tree) casts a **seamless** shadow — the gather tests the
      full silhouette, so it's correct from **any** spanned tile the corridor hits; no column seams
      (the reason slicing was dropped).
- [ ] Confirm the height cull + (later) penumbra using the caster **center** is acceptable for wide
      casters (coarse gate — expected fine).
- [ ] Multi-tile appearance is harmless here (binary is idempotent); the dedup that prevents
      double-*accumulate* lands with coverage in P6 ([I-6](issues.md)).

## P4 · Height cull (radial gate)

- [ ] Precompute `k = d_P / L.z` per (texel, light); per caster **skip if `δ > k·h`** (`δ` = sweep
      distance so far, `h` = caster height from def). Runs **before** the projection + silhouette
      sample.
- [ ] Verify: distant short casters no longer tested (perf), pawn head-shadow reaches farther than
      hand-shadows (correctness of the fan).

## P5 · 8-slot representation (unlimited lights, per-slot output)

- [ ] **Presence → 8× `u16` light indices** per tile (one `RGBA32UI`), nearest-N with **stable slot
      assignment** (keep present lights in-slot, fill freed slots only). Built to **`reach − 1`**
      ([F10](forks.md#f10)) — the light's illumination-range field is **`reach`** (rename `radius`).
      Presence change **dirties** the tile.
- [ ] **Output → per-slot 8-bit** (bit `i` = slot `i` shadowed) instead of 128-bit per-global-light.
- [ ] Update `/overlayRT` decode + the lighting join (slot `i` → light `presence[i]`, skip if
      shadowed). Verify parity with P2 on ≤8-light scenes, then push past 128 total lights.

## P6 · 4-bit coverage (translucency)

- [ ] Output slot → **`u4`** (`0x0` lit … `0xF` opaque); accumulate the caster **alpha** from
      `cover()` (drop the `≥0.5` threshold); **break at `0xF`**.
- [ ] **Dedup multi-tile casters** ([I-6](issues.md)): a caster in >1 corridor tile would add its
      coverage twice. **Cast iff none of the 3 toward-`P` neighbours** (backward along the corridor)
      is in `prim ∩ corridor` (bbox test + the P2 `onCorridor`); else defer. Aim toward `P`, **not**
      the anchor. No stored field, no neighbour-list read; flat 3 checks (corridor is 1-wide,
      [F9](forks.md#f9)). Harmless before P6 (binary idempotent).
- [ ] Verify foliage/glass casters produce partial shadow; opaque casters still `0xF`-and-break; a
      wide tree and a tile-straddling pawn each cast a single (not doubled) shadow.

## P7 · Penumbra (fake area light)

- [ ] Add **`emitter_radius`** to the light record (distinct from `reach`).
- [ ] Soften the `cover()` sample by `penumbra_width = emitter_radius·(tile→occluder)/(light→occluder)`
      — blurred silhouette **mip** preferred (one read). Feeds the same P6 accumulate.
- [ ] Verify: sharp base / soft tip; `emitter_radius = 0` ⇒ hard shadow.

## P8 · Cold / hot split

- [ ] Two fields (`cold-shadow`, `hot-shadow`), same layout; global stable indices + a hot/cold flag
      on lights **and** prims.
- [ ] Dirty routing on change (cold prim ×cold light → cold; anything hot → hot), **symmetric
      promote/settle** ([I-5](issues.md)). Retain hot via dirty (no per-frame clear).
- [ ] **Dirty budget**: measure resolve time → cap tiles/frame; log deferrals.

## Deferred (not this stream unless they bite)

- Shadow-only **opaque bbox** in the def (tighten sample + projection without touching other maps —
  [issues I-3](issues.md)).
- **Smooth (sub-tile) dirtying** for movers vs tile-crossing-only ([issues I-4](issues.md)).
