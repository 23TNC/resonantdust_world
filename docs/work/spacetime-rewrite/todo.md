# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

_Nothing planned._ The conformance re-cut (T-1…T-5 — deviations **D-1…D-5**, divergence **#11**)
landed + is live-verified → [completed.md](completed.md). Every **identity/keying** divergence is
now closed; what remains in the shard's
[divergences.md](../../components/server/spacetime/modules/shard/current/divergences.md) is the
pipeline-shape work (#1 word-DSL `event_log`, #6 lifecycle state machine, #7 drop barrier / GC,
#8 event/data shard split) and the cleanups (#3 dead modules, #9 drop `priority`) — none of it
scoped here yet.

Loose end, tracked in [issues.md](issues.md): the workspace test suite has a **pre-existing** red
test (`dsl::loader::material_registry_and_packed_channels`), so `cargo test` can't gate until it's
fixed.
