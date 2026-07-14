# Todo — docs migration (remaining components)

Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Migrate each component's scattered
top-level docs into `docs/components/<group>/<name>/{design,intent,current,plan}` — **lazily**, as
we work each. Newest-first. (Physical `git mv` breaks code-comment doc-paths; those are fixed when
that code is next touched, not now — we're only documenting.)

- **2026-07-14** · **server/edge** — no edge-specific top-level doc remains (worldgen is inline in
  `edge/src`); create `components/server/edge/{intent,current}` from the map's edge entry + the
  worldgen/`world-on-pipeline` relationship when edge is worked. Low priority.
- **2026-07-14** · **client/core** — split the client half out of pixijs's `intent/client.md` if
  it turns out to describe `client/core` rather than the browser client.

- **2026-07-14** · **minor remainders** — `docs/repo-layout.md` (current orientation; overlaps the
  component map — keep, merge into map, or note); `docs/issues/00X` detail files (already indexed
  by `work/spacetime-rewrite/issues.md`+`forks.md` — optionally relocate into that work folder);
  `docs/prompts/*` (template assets — move with the bin→dev reorg).
