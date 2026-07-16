# Intent — `data_shard` (what goes in, and why)

_Last updated: 2026-07-16._

## What goes in

- `claim(entities, tic, worker)` — the **orchestrator** calls it, having grouped tic T's events into
  components and picked a worker per component. It creates each `(entity, tic)` slot (`dirty = true`),
  stamps `worker_reference` on it, and stamps `observer_reference` on the entity's previous row (the
  base the worker will read).
- `write(event_refs, tic, results)` — the **worker** calls it with each target's **absolute final**
  value. It sets the payload, clears `dirty`, and promotes if asked.
- `reap` / `gc` — the **master**.

A slot's payload is the reference model's three orthogonal references — `definition_reference` (what),
`position_reference` (where), `data` (state).

## Who reads it

- **A worker**, only its own rows: `SELECT * FROM state_log WHERE worker_reference = self OR
  observer_reference = self` — the rows it writes, and the previous rows it reads as base.
- **The edge / client**, on `state`, by zone. Never `state_log`.

## Why worker + observer, not four slots

One worker owns a whole component (the orchestrator guarantees it), so exactly **one** worker writes a
given row — `worker_reference`, a single column, not four. The `observer` is the worker of the
entity's *next* tic, which reads this row as its base; the per-entity chain has exactly one next, so
exactly one observer. Two roles, one per direction.

This is what lets a worker subscribe to `state_log` itself and **read the payload off the row** — so
there is no `state_hold` table, no `ready` go-signal, and no `base` copy. Those existed only because
an older design had multiple workers per row and none could see the whole picture; the orchestrator
removed the premise.

## Why `dirty` is a boolean, and how ordering works

`dirty` is a boolean because one worker owns the component — the row is *pending* or *settled*, never
a count. The ordering rule:

> A row at tic T may not be written while its entity's previous row is still `dirty`. The entity's
> settled value is its newest `!dirty` row.

Checking the *immediate previous* is enough, because rows are created in tic order (nothing earlier
appears late) and clean propagates by induction (a row is written clean only after its previous was).

**The block is the worker's job — this module cannot enforce it.** `A += B` reads B, which may be on
another shard; `write(A)` sees only A's chain, not B's dirtiness. A worker that skips the block reads
a stale B and writes a wrong-but-clean A. The worker *can* block (it's subscribed to its read
targets); the store can't. `write` fences only `caller == worker_reference`.

## Why writes are absolute, and replay is free

The worker holds the whole component, so it computes each target's *final* value and writes it
absolutely. A dead worker's replay recomputes the identical value from the immutable `T-1` — so
`write` just skips already-`!dirty` rows and is idempotent. No deltas, no applied-event set.

## Why promotion is opt-in

`state` is written only where a program ran `PROMOTE_STATE`, and only once the row settles. A program
that promotes nothing composes in `state_log` and no client sees it — the simulation can run entirely
server-side.

`state.macro_position_reference` duplicates the payload's `position_reference` high half on purpose: a
subscription filters on columns, not expressions, so without the column there's no per-zone
subscription. Writers must keep the two in step — nothing in the schema does.

## Entry / exit

| | |
|---|---|
| in | `claim` (orchestrator: slots + roles), `write` (worker: absolute finals) |
| out | `state`, on a `PROMOTE_STATE` action, once settled |
| sweep | `reap` frees expired roles; `gc` drops old settled rows (never the latest per entity) |
