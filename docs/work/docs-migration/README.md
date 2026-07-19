# Work — docs-migration (scattered top-level docs → the `components/` + `work/` convention)

_Opened 2026-07-14. Executes [`docs/CONVENTIONS.md`](../../CONVENTIONS.md): migrate each component's
scattered top-level docs into `components/<group>/<name>/{design,intent,current,plan}` — lazily, as
each is worked — and fold the old `docs/issues/` + `docs/references/` into the single-file convention._

**Status: done.** The migration landed across all worked components (see [`completed.md`](completed.md));
decisions in [`forks.md`](forks.md). Only two items remain in [`todo.md`](todo.md), both **deliberately
deferred** (server/edge lazy-create; `docs/prompts/*` moving with the `bin/`→`dev/scripts/` reorg).

Superseded-in-spirit by [`docs-authority`](../docs-authority/README.md), which adds the enforcement
this migration lacked — an audit + hook so the convention stays obeyed rather than drifting.
