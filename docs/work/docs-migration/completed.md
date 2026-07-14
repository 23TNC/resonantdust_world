# Completed — docs migration to the components/work convention

Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md) across all components. Done work,
chronological.

- **2026-07-14** · **Convention + trees established** — `docs/CONVENTIONS.md` (the how);
  `docs/components/README.md` (the component map); `docs/intent/` staging (+ pathfinding);
  `docs/work/spacetime-rewrite/` (completed/remaining/todo/issues/forks/blockers).
- **2026-07-14** · **`shard` component migrated** — `docs/spacetime-tables/` and
  `docs/spacetime-implementation/` → `docs/components/server/spacetime/modules/shard/`:
  design (`tables`, `event-dsl`, design README), intent (`events`, `lifecycle`, `hot-cold`,
  `risks`, `event-reference`), current (`README` + `divergences`), plan (`stages/` S0–S7 +
  `cleanup` + plan README) + component README. 100 relative links rewritten to the new depth; old
  folders removed. Dead modules kept OUT of design, placed in current (reality) + plan (deletion).
- **2026-07-14** · **codec / gateway / pipeline / pixijs migrated + sync staged** — codec:
  `docs/references/*` + `object-model*.md` → `components/shared/codec/{design,current}` (flags
  the object-model open decisions → blocker B-1). gateway: `gateway.md` → intent. pipeline:
  `pipeline-generalization.md` (design) + `world-on-pipeline.md` (intent). pixijs: `lighting.md`,
  `de-lighting.md` (design) + `client.md`, `zones-to-screen.md` (intent). Cross-cutting
  `sync.md` → `docs/intent/`. 33 outgoing + 12 incoming link-sets rewritten; `docs/references/`
  removed. Component READMEs written.
