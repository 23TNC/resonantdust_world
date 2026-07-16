# Intent — `event_shard` (what goes in, and why)

_Last updated: 2026-07-15._

## What goes in

The **edge** validates a client request (authenticated · owns the object · legal verb · rate) and
calls `queue(actions)`. That is the only door in. The row lands at `master_tic + TIC_GAP` (+3) —
never "now" — because a tic must be sealed before it can be composed.

Nothing else is stored. **No `targets` / `reads` columns**: every reference inside `actions` is a
u32 carrying its `server_id` in the top byte, so the write set *is* the program and a worker reads
it off. A column would restate what's already there.

## Who reads it

- **A worker**, and only its own rows: `SELECT * FROM event_log WHERE worker_reference = self`. It
  never sees another worker's work, and an unassigned row is visible to nobody — which is why
  reclaim has to live in `request_work` (below), not in a worker that can see the orphan.
- **The edge / client**, on `event`, by zone. Never `event_log`.

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
| assignment | `request_work(worker, max_batch)` — pull, and the only rescue path: it reclaims expired leases and fails events that can no longer make their tic |
| out | `settle(t)` — the master. Terminal rows → `event` (if `PROMOTE`), then deleted |
