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
- **2026-07-14** · **dev/** — `docs/art-style.md`, `sprite-gen-plan.md`, `texture-layout/`,
  `texture-paths.md`, `prompts/*` → `components/dev/{textures,scripts}`; the `bin/` → `dev/scripts/`
  reorg. Fuzzy; docs vs assets (`prompts` are template *assets*, not docs) needs a call first.
- **2026-07-14** · **legacy sweep** — `docs/data-shards.md` (dead cold_tiles/things design),
  `object-shard.md`, `simulation.md`, `simulation-plan.md`, `gaps.md`, `repo-layout.md` — these are
  pre-rewrite / superseded designs. Decide: retire to an archive, fold into a component's
  `current`, or delete. Needs a call — don't force them into a component's `design/`.
