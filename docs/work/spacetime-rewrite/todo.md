# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

_Nothing planned **in this work-stream**._ Its scope (S0–S7 + the conformance re-cut T-1…T-5,
deviations **D-1…D-5**, divergence **#11**) is complete + live-verified → [completed.md](completed.md).
Every **identity/keying** divergence is now closed.

**The forward plan lives in the component, not here:**
[`shard/plan/README.md`](../../components/server/spacetime/modules/shard/plan/README.md) — P1 restore
the build gates + cleanups (#3, #9), P2 the word-DSL `event_log` (#1), P3 lifecycle + holder GC
(#6/#7), P4 the event/data shard split (#8, deferred), with the reasoning for that order. Per
[`CONVENTIONS.md`](../../CONVENTIONS.md) each phase gets its **own work folder** when we start it;
this stream stays closed rather than growing a second scope.

Loose end, tracked in [issues.md](issues.md): the workspace test suite has a **pre-existing** red
test (`dsl::loader::material_registry_and_packed_channels`), so `cargo test` can't gate until it's
fixed.
