# Remaining — spacetime rewrite (actively executing)

Executes: the `shard` component. Items here are **in progress right now**; they arrive from
[todo.md](todo.md) when work begins and move to [completed.md](completed.md) when done+verified.
Newest-first.

## In progress

_Nothing actively in progress._ Landed + verified ([completed.md](completed.md)): behavioral core +
integration; hot/identity/event (`refs.rs`, `event_reference` u32, `#5`/`#10`); the object model
(`object.rs`); the geographic geometry (legacy `zone_id`/`surface` retired →
realm/region/zone/tile/layer); the geographic cold `entity_reference`; PACK as an enqueued execute
op (divergence #2 closed); and the cross-shard foreign Phase-1 hold. No blockers open
([blockers.md](blockers.md) — B-1 + B-2 resolved).

**One real bug is open, not started** — [todo.md](todo.md): the **`cold` row lost two of its three
design header fields** in the re-cut (`macro_position_reference` + `layer_id`), so its key can't
select a row by layer and `find_or_mint` ignores the target's `layer_reference` entirely
(divergence **#11**). Live symptom: on a tile holding both ground and a thing, which one an Interact
mints is iteration-order luck. Not started, so not listed here.
