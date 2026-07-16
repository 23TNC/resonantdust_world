# Intent — `data_shard` (what goes in, and why)

_Last updated: 2026-07-15._

## What goes in

`declare_pending(event, tic, entity, promote)` creates the `(entity, tic)` slot and increments its
`dirty`. `apply` composes an event's result into the slot and decrements it. Both are called by a
worker; neither is a client door.

A slot's payload is the reference model's three orthogonal references — `definition_reference`
(what), `position_reference` (where), `data` (state). Seeded from the resolved value at `< tic`.

## Who reads it

- **A worker**, only rows it holds a slot on:
  `SELECT * FROM state_log WHERE worker_a = self OR worker_b = self OR worker_c = self OR worker_d = self`.
- **The edge / client**, on `state`, by zone. Never `state_log`.
- `state_events` is **internal** — nothing subscribes to it.

## Why four worker slots

This is the load-bearing decision. It is what lets a worker subscribe to `state_log` *itself*
rather than to a projection of it — and therefore **read the payload off the row it is assigned**.

The alternatives all fail: a single `worker_reference` column can't name the several workers
touching a row; per-target subscriptions are a bazillion subscriptions; per-event ones collide when
two events share a row in a tic.

What it buys is the deletion of an entire machine. The design this replaces needed a `state_hold`
table carrying `ready` (a go-signal) and `base` (a payload copy) *because the worker couldn't see
`state_log`*. Remove that premise and the hold, the go-signal, the copy, and the scheduler mirror
all go with it.

A slot is a worker **address**, not a per-event lock: one worker holding a thousand events against
a row takes one slot. So the cap binds only when five *distinct workers* want one entity at once —
a partitioning question, not a contention one.

## Why `dirty`, and how ordering works

`dirty` is the count of events holding a slot; `dirty == 0` is settled. The rule:

> A row at tic T may not be written while any earlier tic for that entity is still dirty. The
> entity's settled value is the newest row with `dirty == 0`.

The worker evaluates that itself, because `request_state` gives it a slot on **every** row for its
targets — it holds the entity's history, not a snapshot. `apply` re-checks, since a worker may have
computed from a base another worker dirtied underneath it.

## Why promotion is opt-in

`state` is written only where a program asked (`PROMOTE`, latched per slot at `declare_pending`),
and only once the slot settles. A program that promotes nothing composes in `state_log` and no
client sees it — **the simulation can run entirely server-side**.

`state.macro_position_reference` duplicates the payload's `position_reference` high half on purpose:
a subscription filters on columns, not expressions, so without the column there is no per-zone
subscription at all. Writers must keep the two in step — nothing in the schema does.

## Entry / exit

| | |
|---|---|
| in | `declare_pending` (slot + `dirty`), `request_state` (claim a worker slot), `apply` (compose) |
| out | `state`, on a `promote_state` action, once settled |
| sweep | `reap()` frees dead workers' slots; `gc(t)` drops old settled rows |
