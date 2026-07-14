# Remaining — spacetime rewrite (actively executing)

Executes: the `shard` component. Items here are **in progress right now**; they arrive from
[todo.md](todo.md) when work begins and move to [completed.md](completed.md) when done+verified.
Newest-first.

## In progress

_Nothing actively in progress._ The behavioral core + integration, the **hot/identity/event re-cut**
(`refs.rs`, `event_reference` u32, `#5`/`#10`), and the **object-model re-cut** (`object.rs` →
reference model) are all complete + browser-verified ([completed.md](completed.md)). No blockers
open ([blockers.md](blockers.md) — B-1 + B-2 both resolved). The remaining tranche (retire the
legacy world-global `zone_id`: drop `surface`, geographic realm/region/zone, `region_zone:u16` cold
key, + the PACK trigger) is scoped + **unblocked** in [todo.md](todo.md) but not yet started, so it
isn't listed here.
