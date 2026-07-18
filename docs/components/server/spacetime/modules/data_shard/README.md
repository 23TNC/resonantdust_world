# Component — `data_shard` (composition + state)

_Path: `server/spacetime/server/modules/data_shard`. Last updated: 2026-07-15._

> **Direction shift (2026-07-18):** `data_shard` becomes **`entity_tables!{data:u8}`** — its
> `state`/`state_log` rename to **`entity_state`/`entity_state_log`** under the generic `*_tables!`
> macros, and `PROMOTE` becomes a prefix. Live code still uses `state`/`state_log`. See
> [`work/shard-tables`](../../../../../work/shard-tables/README.md) + [`TABLES.md`](../../../../../TABLES.md).

Holds `state_log` (the per-`(entity, tic)` composition slot), `state_events` (an internal reverse
index), and `state` (client-visible latest). Where an event's effect is actually composed, and the
only place a client's world comes from.

- **[`intent/`](intent/)** — what goes in, who reads it, why four worker slots.
- **[`plan/`](plan/)** — build order, and what blocks it.

Shape: [`TABLES.md` §`data_shard`](../../../../../TABLES.md). Flow:
[`intent/spacetime-again/`](../../../../../intent/spacetime-again/README.md).
