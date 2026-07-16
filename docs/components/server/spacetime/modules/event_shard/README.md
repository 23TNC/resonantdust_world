# Component — `event_shard` (the queue + the log)

_Path: `server/spacetime/server/modules/event_shard` (**not built**). Last updated: 2026-07-15._

Holds `event_log` — work **in flight** — and `event`, the settled, zone-scoped, client-visible log.
The queue is not the log: settled rows leave `event_log`, which is what keeps a worker's
subscription small forever rather than growing with history.

- **[`intent/`](intent/)** — what goes in, who reads it, why it's split.
- **[`plan/`](plan/)** — build order, and what blocks it.

Shape: [`TABLES.md` §`event_shard`](../../../../../TABLES.md). Flow:
[`intent/spacetime-again/`](../../../../../intent/spacetime-again/README.md).
