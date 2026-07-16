# Component — `data_shard` (composition + state)

_Path: `server/spacetime/server/modules/data_shard` (**not built**). Last updated: 2026-07-15._

Holds `state_log` (the per-`(entity, tic)` composition slot), `state_events` (an internal reverse
index), and `state` (client-visible latest). Where an event's effect is actually composed, and the
only place a client's world comes from.

- **[`intent/`](intent/)** — what goes in, who reads it, why four worker slots.
- **[`plan/`](plan/)** — build order, and what blocks it.

Shape: [`TABLES.md` §`data_shard`](../../../../../TABLES.md). Flow:
[`intent/spacetime-again/`](../../../../../intent/spacetime-again/README.md).
