# Plan — `shard` (how we close current → design)

_Last updated: 2026-07-14._

## Done — the staged build (S0–S7)

The original path onto the [`design/`](../design/) is the staged plan in
[`stages/`](stages/) (S0 foundation → S7 tail). It's essentially complete; the granular done-log
is [`work/spacetime-rewrite/completed.md`](../../../../../../work/spacetime-rewrite/completed.md).

## Forward

1. **[`cleanup.md`](cleanup.md)** — retire the dead `cold_tiles` / `cold_things` / `experiment`
   modules + their `redeploy.sh` fam-entries. Pure removal; brings `current` toward `design`
   (which never had them). *Not yet done — code change, deferred while we're documenting.*
2. **`event_reference` → `u32`** — design-decided ([`../intent/event-reference.md`](../intent/event-reference.md)); mechanical once scheduled.
3. **Representation re-keys** — `hot_reference` u32 (#5/D3), `region_zone` cold key (#4),
   geographic `server_reference` (#10). **Blocked** on the object-model decisions —
   [`work/spacetime-rewrite/blockers.md`](../../../../../../work/spacetime-rewrite/blockers.md) B-1.
4. **`PACK` trigger** + **cross-shard foreign Phase-1 hold** — see
   [`work/spacetime-rewrite/todo.md`](../../../../../../work/spacetime-rewrite/todo.md).

The live execution state for all of the above is the work-stream
[`docs/work/spacetime-rewrite/`](../../../../../../work/spacetime-rewrite/).
