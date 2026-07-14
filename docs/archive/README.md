# Archive — superseded designs

_Pre-rewrite / superseded design docs, kept for history. Not the design/intent/current/plan of
any live component; consult the replacement. Per [`../CONVENTIONS.md`](../CONVENTIONS.md). Last
updated: 2026-07-14._

| Archived | Was | Superseded by |
|---|---|---|
| `simulation.md` | the original server-simulation pipeline (event/state split, tic frontier) | the `shard` component ([`design/`](../components/server/spacetime/modules/shard/design/) + [`intent/`](../components/server/spacetime/modules/shard/intent/)) |
| `simulation-plan.md` | the original implementation plan | shard [`plan/stages/`](../components/server/spacetime/modules/shard/plan/stages/) (S0–S7) |
| `gaps.md` | the old known-gaps list | shard [`current/divergences.md`](../components/server/spacetime/modules/shard/current/divergences.md) + [`work/spacetime-rewrite/`](../work/spacetime-rewrite/) |
| `data-shards.md` | the split cold_tiles/cold_things shard design | the `shard` module's `cold` table (object model); the dead modules' removal is shard [`plan/cleanup.md`](../components/server/spacetime/modules/shard/plan/cleanup.md) |
| `object-shard.md` | the 2nd shard-class (object-shard) design | the unified `shard` (hot+cold in one module) |
