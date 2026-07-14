# Component — `server/spacetime/server/pipeline` (`resonantdust-pipeline`)

_Path: `server/spacetime/server/pipeline`. **Shared** (ST-side) — the `decl_tick_pipeline!` macro
that generates a module's tables + lifecycle reducers. A change here changes every module built on
it (today: shard). Last updated: 2026-07-14._

- **[`design/pipeline-generalization.md`](design/pipeline-generalization.md)** — one engine over
  any payload: the generalized tick pipeline.
- **[`intent/world-on-pipeline.md`](intent/world-on-pipeline.md)** — how the tile world (biomes,
  terrain, trees) maps onto the pipeline.
- `current/`, `plan/` — lazy.
