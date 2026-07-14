# S2 — Event schema + lifecycle + holder table

**Goal:** reshape `event_log` to `actions: Vec<u64>`, add the **lifecycle state machine**, the
**holder table** (refcount for GC), and the reducers the enqueue phase + master drop need. Closes
**divergences #1, #6, #7** (schema-side); the worker that drives it is [S3](s3-worker.md).

**Status:** ⬜ not started. **Depends on:** [S0](s0-foundation.md). **Parallel with**
[S1](s1-interpreter.md); converges at [S3](s3-worker.md). Design: [lifecycle.md](../../intent/lifecycle.md).

## `EventLog` shape

In [`decl_tick_pipeline!`](../../../../../../../../server/spacetime/server/pipeline/src/lib.rs):

- Replace the scalar columns (`actor_key`, `target_key`, `action`, `data0/1`, the named
  `*_server_reference`) with **`actions: Vec<u64>`** (one vector/row, may touch many entities)
  and a **`targets`** list the issuer designates (decision A).
- Add the **lifecycle columns**: `status` (`enqueue|queueing|in_queue|running|complete|queue_failed`),
  `failed: bool`, `tic_state_change: u32` ([lifecycle.md](../../intent/lifecycle.md)).
- Keep `event_reference` (identity, minted on write), `event_tic`. **No `alias` column** —
  aliases are compose-time stand-ins resolved to `event_reference`s at write.

## Data-shard side: holder table, open-rows, cold_removed

- **holder table** — which `event_reference`s hold a pending **read** or **write** on each
  `state_log` row (`state_log_id, event_reference, kind`). GC reclaims a row only when it has no
  holders (+ not-latest + old). **No `read_watermark`** — causality is staging + the master drop.
- **open-rows table** keyed by `event_reference` — which `state_log` rows an enqueue stood up, so
  a re-drive finishes or backs out idempotently.
- **`cold_removed`** — per-zone `removed: Vec<u16>` (`x:4|y:4|layer:4|type_id:4`): the removal
  delta a mint appends to instead of rewriting the `cold` row; a **`PACK` action** compacts it
  (not GC) ([tables.md](../../design/tables.md) §`cold_removed`).

## Reducers

- **`append(actions, targets, …)`** — writes a **batch** of rows at `event_tic = master + 3`,
  mints each `event_reference`, and **substitutes intra-plan aliases → the minted refs**.
  `status = enqueue`.
- **`stand_up(event_reference, target, tic)`** — idempotently open a pending `state_log` row for
  a target (fail if the slot is already resolved), register the holder, and record it in
  open-rows. For a **cold_reference target**, `find-or-mint` the hot entity at that location
  (idempotent by location) and use its fresh row — the cold→hot mint is absorbed here
  ([S3](s3-worker.md)/[S6](s6-hot-cold.md)).
- **`drop_timed_out()`** — **master-called, before `bump`**: `queue_failed` every still-
  `enqueue`/`queueing` row whose window expired, backing out its holds/open-rows. The per-tic
  causality barrier ([lifecycle.md](../../intent/lifecycle.md)).
- **`back_out(event_reference)`** — remove the row's open state_log rows + holders; set
  `status = queue_failed`, `failed = true`.
- **`resolve(...)`** — fenced atomic commit of a row's target writes; **release the row's holders**
  in the same transaction; `status = complete`. (Same-shard atomic; cross-shard convergent —
  [S3](s3-worker.md).)
- `bump` advances the tic + surfaces the tic's `enqueue`/`in_queue` rows as claimable; never
  executes the program (reads `targets` only). The master calls `drop_timed_out` then `bump`.

## Verify

Module compiles + deploys (`rd build` / `rd deploy` in-container). The worker won't build until
[S3](s3-worker.md) (its bindings regenerate from this schema) — expected.

## Notes

- The `(source_shard, event_reference)` **provenance pair** stays as the **cross-shard
  idempotency key** (it's not needed for same-shard dedup, which is structural, but it is for
  cross-shard convergent writes — [lifecycle.md](../../intent/lifecycle.md)).
- Which physical shard (`event` vs `data`) holds which table is a deployment concern (div #8) —
  the generic module still defines them all; the split lands at [S7](s7-tail.md).
