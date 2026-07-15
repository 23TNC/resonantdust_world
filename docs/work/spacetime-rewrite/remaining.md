# Remaining — spacetime rewrite (actively executing)

Executes: the `shard` component. Items here are **in progress right now**; they arrive from
[todo.md](todo.md) when work begins and move to [completed.md](completed.md) when done+verified.
Newest-first.

## In progress

_Nothing actively in progress — **the rewrite is done**_ ([completed.md](completed.md)): behavioral
core + integration; hot/identity/event (`refs.rs`, `event_reference` u32, `#5`/`#10`); the object
model (`object.rs`); the geographic geometry (legacy `zone_id`/`surface` retired →
realm/region/zone/tile/layer); the geographic cold `entity_reference`; PACK as an enqueued execute
op (divergence #2 closed); and the cross-shard foreign Phase-1 hold. All verified — browser (terrain
+ wolves), the cold round-trip (Interact → find-or-mint → PACK settle), and a real 2-shard rig for
the foreign hold. No blockers open ([blockers.md](blockers.md) — B-1 + B-2 resolved). The one
remaining [todo.md](todo.md) entry is a *deliberate non-item* (the `region_zone:u16` cold-key
compaction — recommended to wait for multi-realm), so nothing is in progress.
