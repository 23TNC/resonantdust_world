# Plan — `event_shard` (nothing → built)

_Last updated: 2026-07-15. Nothing is built._

## Blocked before phase 3

**`actions : Vec<u32>` has no encoding.** `event_word` was deleted with the old pipeline (it had
zero call sites), and the rebuild has not chosen a word format. Phases 1–2 don't read `actions`, so
they can proceed; everything that *interprets* it cannot. Deciding the format probably also settles
read-set derivation and the `promote_*` verbs — see
[`intent/spacetime-again/`](../../../../../../intent/spacetime-again/README.md) §Open.

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
   Needs the word format only insofar as the worker must extract targets.
4. **`settle(t)`.** Terminal rows → `event`, **one row per zone** the targets occupy, then delete
   from `event_log`.

## Ties into

`data_shard`'s plan — phase 3 here calls `declare_pending` there. Build `data_shard` phases 1–2
first, or stub them.
