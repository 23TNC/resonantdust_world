# Intent — `event_shard` (what goes in, and why)

_Last updated: 2026-07-16._

## What goes in

The **edge** validates a client request (authenticated · owns the object · legal verb · rate) and
calls `queue(actions)`. That is the only door in. The row lands at `master_tic + 3` — never "now" —
because a tic must be frozen before it can be grouped and composed.

Nothing else is stored. **No `targets` / `reads` columns**: every reference inside `actions` is a u32
carrying its `server_id` in the top byte, so the write set *is* the program. This module scans it once
to **group** — union events sharing a target `entity_reference` into an `event_group` — and to latch
`PROMOTE`. It never interprets a verb.

## Who reads it

- **The orchestrator** for this tic: `SELECT * FROM event_log WHERE orchestrator_reference = self`.
  It merges this shard's `event_group`s with every other shard's into cross-shard work-groups, then
  calls `assign` to stamp a worker.
- **A worker**, only its own rows: `SELECT * FROM event_log WHERE worker_reference = self`.
- **The edge / client**, on `event`, by zone. Never `event_log`.

## Why the completeness barrier

The shard serves a tic's groups to the orchestrator **only after it has frozen** (read master ≥ T-2,
so no more events for T can be born). A partial batch would let the orchestrator finalize a component
that a late event bridges into another — splitting one component across two workers, which corrupts.
So: freeze first, serve second. This is a correctness invariant, not a latency knob.

## Why the queue is not the log

`event_log` is the whole reason this module is cheap: it holds only in-flight work, so the
worker's subscription stays a fixed small thing forever. If settled rows stayed, the subscription
would grow with history and the design's central claim — *a subscription is an assignment* — would
decay into "mirror the shard again". `settle(t)` moves terminal rows to `event` and deletes them.

## Why `event` fans out per zone

One row **per zone the event's targets occupy**, keyed by
`event_uid = macro_position_reference | event_tic | event_reference`. So `event_reference` is not
unique here, on purpose: a client subscribed to one zone must see a **cross-zone** event that
reaches into it, not just events wholly inside it. One row per event would make an event visible in
exactly one of the zones it touched.

## Why promotion is opt-in

A row reaches `event` only if its program asked (`PROMOTE`, latched at `queue`). A program that
promotes nothing settles in `event_log` and no client ever learns it happened. Visibility is a
thing a program requests, not a tax the spine charges.

## Entry / exit

| | |
|---|---|
| in | `queue(actions)` — the edge |
| grouping | shard-local: union by shared target into `event_group`, frozen at T-2, served to the orchestrator |
| assignment | `assign(events, worker)` — the orchestrator, after it forms cross-shard work-groups; `fail(event_group)` for a late straggler |
| out | `settle(t)` — the master. Terminal rows → `event` (if `PROMOTE`), one per zone, then deleted |
