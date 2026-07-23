# Issues — def frame/anchor rework

_Problems hit + candidate solutions + which we chose + why. Chronological._

---

## Grid (linked-atlas) stems can't scale-at-ingest — 2026-07-23

One blit scales the whole packed grid image about its centre, which would slide cells off their
grid slots; per-cell re-centring needs per-cell blits. No gridded stem casts shadows or authors a
scale today → **packed unscaled + a one-shot warn**. Revisit with per-cell blits if a gridded stem
ever needs a scale.

## Hot-swap sprite_scale change leaves stale defs — 2026-07-23

`setSpriteScale` change evicts the stem's packed LODs (repack under the new transform), but defs
are IMMUTABLE and keyed `(stem, cell, lod)` — a repack that relocates the frame leaves the old
def's frame origin stale until reload. Dev-time only (scale changes ride content hot-swap);
fix = def invalidation by stem when C5/eviction lands.

## shared/dsl unit test not run locally — 2026-07-23

`cargo` lives in docker; `rd build shared` compiles (green) but doesn't run the updated
`thing_stem_and_layout_tables` test. Run it with the next dockerised test pass.
