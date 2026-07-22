# Todo — 2026-07-22-shadow-corridor (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified on
`/overlayRT shadow-cold`. Phases run in order; each is verifiable on its own. See
[`README.md`](README.md) for the architecture + reasoning, [`forks.md`](forks.md) for decisions,
[`issues.md`](issues.md) for accepted limits + deferrals._

---

_**Done + verified** 2026-07-22 → [`completed.md`](completed.md): P1, P2, P4, multi-light, P6
(coverage), P7 (penumbra), **I-7** (base-row-bucketing fix), **P1′** (LUT deleted), **P7′** (per-light
`emitter_radius` + `radius`→`reach`). Remaining below._

## P3 · Wide-caster correctness (whole-caster, no slicing — [F5](forks.md#f5))

- [ ] Verify a **wide** caster (multi-tile tree) casts a **seamless** shadow — the gather tests the
      full silhouette, so it's correct from **any** spanned tile the corridor hits; no column seams
      (the reason slicing was dropped).
- [ ] Confirm the height cull + (later) penumbra using the caster **center** is acceptable for wide
      casters (coarse gate — expected fine).
- [ ] Multi-tile appearance is harmless here (binary is idempotent); the dedup that prevents
      double-*accumulate* lands with coverage in P6 ([I-6](issues.md)).

_**P5 done + verified** 2026-07-22 → [`completed.md`](completed.md): 8× u16 nearest-light presence
(reach−1) + per-slot 4-bit output. Light count is now a free dial. Remaining: **P8** + **P6′**._

_P6 (4-bit coverage) + P7 (penumbra) **core done + verified** → [`completed.md`](completed.md).
Remaining refinements below._

## P6′ · Toward-P dedup (deferred — only if a seam shows)

- [ ] Wire the **3 toward-`P` neighbour** dedup ([I-6](issues.md)) into the coverage accumulate: cast
      iff none of the 3 toward-`P` neighbours is in `prim ∩ corridor` (bbox test + a shared
      `onCorridor` predicate — currently the march is inline Bresenham; factor the predicate so both
      agree). Not yet needed — base-line bucketing + break-at-full hides any double-count so far.
- [ ] Verify foliage/glass casters produce partial shadow (needs translucent test content).

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
