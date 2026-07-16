# Plan — `event_shard` (nothing → built)

_Last updated: 2026-07-15. Nothing is built. Active work:
[`work/spacetime-again/`](../../../../../../work/spacetime-again/README.md) W2._

## Reading `actions`

Encoding: [`ACTIONS.md`](../../../../../../ACTIONS.md). `queue` scans the program once, for two
things only:

- **`PROMOTE_EVENT` present** → latch `event_status.flags.PROMOTE`. That is the whole of promotion
  on this side; nothing later re-reads the program to decide it.
- **validity** — arity frames the stream, and a wrong arity mis-frames the rest of it with no
  re-sync point. Reject a program that doesn't parse *here*, at the only door in, rather than
  handing a worker something it can't decode.

The module never interprets a verb. Extracting targets is the worker's job (W4), off the same
signature table.

## Phases

1. **Tables + `queue`.** `event_log` + `event` per [`TABLES.md`](../../../../../../TABLES.md).
   `queue(actions)` mints `event_reference` as an `entity_reference` (`server_reference:8 |
   ++counter:24` — **not** SpacetimeDB `auto_inc`, which would increment the server byte and break
   the global order), stamps `event_tic = tic::add(master_tic, TIC_GAP)`, latches `PROMOTE`.
2. **`request_work`.** Pull assignment, and the only rescue path: reclaim expired leases (rewinding
   to the **phase's** start, not the beginning), fail what can no longer make its tic, then assign
   an ascending bounded batch. Verify here that a worker's filtered subscription actually delivers
   a row the reducer just stamped — the whole model rests on it.
3. **The worker's phase transitions.** `enqueue_done` (→ `QUEUE_SUCCESS`, keeping the worker),
   `running`, `fail` (sets the `FAILED` flag, leaves `status` on the phase it died in), `complete`.
   The module doesn't read `actions` here — the worker does.
4. **`settle(t)`.** Terminal rows → `event`, **one row per zone** the targets occupy, then delete
   from `event_log`.

## Ties into

`data_shard`'s plan — phase 3 here calls `declare_pending` there. Build `data_shard` phases 1–2
first, or stub them.
