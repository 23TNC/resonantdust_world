# Todo — 2026-07-22-shadow-corridor (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified on
`/overlayRT shadow-cold`. Phases run in order; each is verifiable on its own. See
[`README.md`](README.md) for the architecture + reasoning, [`forks.md`](forks.md) for decisions,
[`issues.md`](issues.md) for accepted limits + deferrals._

---

_P1 + P2 **done + verified** 2026-07-22 → [`completed.md`](completed.md). Below: cleanup + P3 onward._

## P1′ · LUT cleanup (deferred from P1)

- [ ] Delete the `light_data` rows 1–32 LUT + its CPU build in `ColdShadowData.buildLights` (now dead
      — the corridor reads buckets). Shrink `light_data` to the record row; keep `buildLights` for the
      records only. (`onCorridor` was inlined as the Bresenham march in P2; factor it into a shared
      predicate when the dedup lands in P6, since both must agree.)

## P3 · Wide-caster correctness (whole-caster, no slicing — [F5](forks.md#f5))

- [ ] Verify a **wide** caster (multi-tile tree) casts a **seamless** shadow — the gather tests the
      full silhouette, so it's correct from **any** spanned tile the corridor hits; no column seams
      (the reason slicing was dropped).
- [ ] Confirm the height cull + (later) penumbra using the caster **center** is acceptable for wide
      casters (coarse gate — expected fine).
- [ ] Multi-tile appearance is harmless here (binary is idempotent); the dedup that prevents
      double-*accumulate* lands with coverage in P6 ([I-6](issues.md)).

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
