# Plan — `event_shard` (nothing → built)

_Last updated: 2026-07-16. Nothing is built. Active work:
[`work/spacetime-again/`](../../../../../../work/spacetime-again/README.md) W2._

Flow: [`intent/spacetime-again/`](../../../../../../intent/spacetime-again/README.md). Shapes:
[`TABLES.md`](../../../../../../TABLES.md). This module owns the queue (`event_log`) and the settled,
client-visible log (`event`). It **groups** its own events locally; the orchestrator merges across
shards.

## Reading `actions`

Encoding: [`ACTIONS.md`](../../../../../../ACTIONS.md). The module scans a program for three things and
**never interprets a verb**:

- **write targets** — for grouping. Union events that share a target `entity_reference` into an
  `event_group`. (Targets come off the program per ACTIONS.md; a written operand must be literal, not
  `POP`'d — see the actions notes.)
- **`PROMOTE_EVENT` present** → latch `event_status.flags.PROMOTE` at `queue`.
- **validity** — arity frames the stream with no re-sync point. Reject a program that doesn't parse
  *here*, at the only door in.

## Phases

1. **Tables + `queue`.** `event_log` + `event`. `queue(actions)` mints `event_reference`
   (`server_reference:8 | ++counter:24` — **not** column `auto_inc`, which increments the server byte
   and breaks the global order), stamps `event_tic = tic::add(master_tic, 3)`, `orchestrator_reference
   = orchestrator_for(event_tic)`, latches `PROMOTE`, and joins/creates its `event_group`.
2. **Local grouping.** As events arrive, union by shared write-target into `event_group`s. Reject a
   `queue` for tic T once the shard has advanced past T's birth tic (T-3) — too late, the set is
   frozen. This freeze is what lets the orchestrator trust a served batch is complete.
3. **Serve the orchestrator.** Expose N=3's groups **only after the shard has read master ≥ T-2** (set
   frozen). Before that, reject a group request — a partial batch would let the orchestrator split a
   component (the completeness barrier). Subscription: `WHERE orchestrator_reference = self`.
4. **`assign` + the worker transitions.** `assign(events, worker)` stamps `worker_reference`
   (→ `ASSIGNED`); `fail(event_group)` sets `FAILED` (load-shed on a late straggler); `complete` /
   `running` from the worker. Verify a filtered subscription actually delivers a row the reducer just
   stamped — the whole model rests on it.
5. **`settle(t)`.** Terminal rows (`COMPLETE` or `FAILED`) → `event`, **one row per zone** the targets
   occupy, then delete from `event_log`.

## Do-first / watch

- **The `event_reference` mint is the global composition order.** Get the "not `auto_inc`" part right
  in phase 1 or the order breaks silently.
- **The completeness barrier (phase 3) is a correctness invariant, not an optimization.** Serving a
  partial batch corrupts. Test it: an orchestrator that requests before T-2 must be refused.

## Ties into

- **orchestrator** — consumes phase 3's served groups and calls phase 4's `assign`. Independent
  process (`server/orchestrator`, work W4).
- **data_shard** — the orchestrator (not this module) calls `data_shard.claim`. This module and the
  data shard never call each other.
