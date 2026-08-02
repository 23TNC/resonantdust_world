# Plan — render performance

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.**

- **Never block a draw on a fetch.** Flat geometry is the resting state; art upgrades into it. Any
  code path where a pixel waits on a network round-trip is the defect this stream exists to remove.
- **Measure the thing that was complained about.** Frame time is not load latency
  ([I1](issues.md#i1)). Each item states which number it moves.
- **Populated, wheel-driven fixtures.** A quiet scene driven by `__zoom()` measured 3.2 ms and told
  us nothing useful; reproduce under load, through the real input path.
- **No geometry changes.** [`subframe-ingest`](../2026-08-02-subframe-ingest/README.md) owns
  placement. If a change here moves a sprite, it is out of scope and wrong.
- **No LOD ladder.** It was deleted deliberately and it made the runtime bbox unstable
  ([subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7)). A cheap PLACEHOLDER tier is not
  a ladder ([F2](forks.md#f2)).

## P0 — Instrument what was actually reported

- [ ] Add a first-paint probe: wall time from navigation to the first non-geo pixel, and per stem to its first real art. Acceptance: one call reports both, so "loads slowly" becomes two numbers instead of an impression.
- [ ] Record the cold-load baseline with the cache cleared. Acceptance: per-stem fetch + decode timings written down, so the await removal is measured against a real starting point rather than a remembered one.
- [ ] Measure frame cost in a POPULATED view under wheel zoom, not `__zoom()`. Acceptance: a number from the path the user actually drives, since the existing 3.2 ms figure came from neither.
- [ ] Record resident bytes: composites, atlas pages, and the IndexedDB cache. Acceptance: a total, because ~306 MiB of composites is the ceiling that decides target hardware and nothing tracks it today.

## P1 — Remove the await; flat geometry is the resting state ([F1](forks.md#f1))

- [ ] Stop awaiting all four maps together in `ensureCoPack`. Acceptance: `albedo` landing alone is enough to draw a stem; `layers` arriving late never holds `albedo` back.
- [ ] Let each map upgrade the co-pack independently as it lands. Acceptance: a stem re-packs on each arrival rather than once at the end, and a missing map leaves its quadrant transparent instead of blocking.
- [ ] Confirm the GEO tier draws with no network at all. Acceptance: with fetches blocked, the world still renders flat tinted geometry — proving the resting state is real and not merely a fallback that never runs.
- [ ] Emit on every arrival, not only on success. Acceptance: a failed or partial pack still schedules a re-bake, closing [I2](issues.md#i2) where `if (ok) emit()` leaves a stem geo forever.
- [ ] Re-measure first paint against P0. Acceptance: time to first non-geo pixel drops, stated as a number and not as "feels faster".

## P2 — Decode off the critical path ([F3](forks.md#f3))

- [ ] Measure `createImageBitmap` cost per map at each master size. Acceptance: the decode share of load latency is known before anything is moved off-thread on a hunch.
- [ ] Cap concurrent decodes so a zone's worth of stems cannot storm the main thread. Acceptance: a bounded queue, with the bound justified by P2's measurement.
- [ ] Decide whether decode moves to a worker ([F3](forks.md#f3)). Acceptance: a stated answer with its cost — `ImageBitmap` is transferable, but the atlas upload must stay on the GL thread.

## P3 — A placeholder tier, which is not a ladder ([F2](forks.md#f2))

- [ ] Fetch one small tier purely as a placeholder, swapped when the master lands. Acceptance: it is never consulted for zoom and never feeds geometry, so [subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7)'s instability cannot return.
- [ ] Serve the placeholder from IndexedDB first. Acceptance: a warm reload paints real art before any network round-trip completes, since `previewCache` survived the ladder's deletion.
- [ ] Re-measure first paint. Acceptance: cold and warm figures both stated against P0.

## P4 — The standing costs

- [ ] Stop destroying `mrtScratch` on every partition change. Acceptance: allocated once at the largest `slotPx` and viewport-scissored, so a zoom cannot stall on a GPU allocation.
- [ ] Cost the 8 composites against a budget. Acceptance: a stated target for resident bytes, since 306 MiB is currently an accident of `SQUARE 128` rather than a decision.
- [ ] Stop re-running the DSL hook per lookup in `node_visual` ([I3](issues.md#i3)). Acceptance: a kind's visual is resolved once and cached; it is currently re-executed per TILE in `zone_tile_prims`.
- [ ] Re-measure frame cost under load against P0. Acceptance: the populated, wheel-driven number moves, or the item is recorded as no-change with its measurement.
