# Todo — docs migration (remaining components)

Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Migrate each component's scattered
top-level docs into `docs/components/<group>/<name>/{design,intent,current,plan}` — **lazily**, as
we work each. Newest-first. (Physical `git mv` breaks code-comment doc-paths; those are fixed when
that code is next touched, not now — we're only documenting.)

- **2026-07-14** · **shared/codec** — the reference layouts (`docs/references/*`,
  `docs/object-model.md`, `object-model-status.md`) are really codec's design/intent; the shard
  currently links to them in place. Migrate → `components/shared/codec/{design,intent}` and
  relink shard. (Includes the object-model's open decisions — feeds blocker B-1.)
- **2026-07-14** · **server/edge** — `docs/gateway.md` (partly), worldgen notes, `sync.md` (the
  sync model, cross-cutting client/edge/worker) → edge design/intent (+ `docs/intent/` for the
  cross-cutting sync/pathfinding parts).
- **2026-07-14** · **server/gateway** — `docs/gateway.md` → gateway design/intent.
- **2026-07-14** · **client/{pixijs,core}** — `docs/client.md`, `zones-to-screen.md`,
  `lighting.md`, `de-lighting.md` → pixijs design/intent; `sync.md` client half.
- **2026-07-14** · **dev/** — `docs/art-style.md`, `sprite-gen-plan.md`, `texture-layout/`,
  `texture-paths.md` → `components/dev/{textures,scripts,dsl}`; the `bin/` → `dev/scripts/` reorg.
- **2026-07-14** · **server/spacetime/pipeline** — `docs/pipeline-generalization.md`,
  `world-on-pipeline.md` → pipeline design/intent.
- **2026-07-14** · **legacy sweep** — `docs/data-shards.md` (dead cold_tiles/things design),
  `object-shard.md`, `simulation*.md`, `gaps.md` — classify as retired / current / plan or delete.
