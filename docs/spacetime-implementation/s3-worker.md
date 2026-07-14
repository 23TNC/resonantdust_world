# S3 — Worker: two-phase resolve (enqueue + execute)

**Goal:** turn the worker into the two-phase, recoverable lifecycle driver: **enqueue** (stand
up targets, acquire holds) then **execute** (run the interpreter, commit, release holds); add the
**master `drop_timed_out` → `bump`** barrier and **refcounted GC**. Closes **divergences #1, #6,
#7, #9**. Design: [lifecycle.md](../spacetime-tables/lifecycle.md).

**Status:** ⬜ not started. **Depends on:** [S1](s1-interpreter.md), [S2](s2-event-schema.md).

## Phase 1 — enqueue

Claim an `enqueue` row → `queueing`. For each `target`, call `stand_up` — open a pending
`state_log` row (fail if the slot is already resolved), register a **holder**, record it in
open-rows.

- **Cold target ⇒ `find-or-mint` (unpack absorbed).** A `cold_reference` has no `state_log` row,
  so standing it up *is* minting it: `find-or-mint` the hot entity at that location (idempotent
  **by location** — reuse if already hot, so re-drive / a second event never duplicates), seed
  its `state_log` row (that *is* the pending row), and rebind the target to the `hot_reference`.
  A structural data-shard reducer, not DSL — no `MINT`/`GET` verb ([S6](s6-hot-cold.md)).
  - **Append a `cold_removed` tombstone** (`x:4|y:4|layer:4|type_id:4`) instead of rewriting the
    `cold` row — doubles as the "already unpacked" marker. A `PACK` action compacts it (not GC).
- **All stood up** → `in_queue`. **Miss the window** → the master's `drop_timed_out` sets
  `queue_failed` and backs out the holds/open-rows (no watermark; the drop *is* the causality
  guard).
- **Crash-recoverable:** open-rows records what's stood up; a re-drive finishes the rest or backs
  out — the same deterministic decision on any worker.

## Phase 2 — execute

Claim an `in_queue` row → `running`. Run the S1 interpreter over the row's `actions`, computing
**all** target effects in scratch, then commit.

- **The row is atomic.** One commit writes every target together (same-shard = one ST
  transaction). Cross-shard = **convergent**: per-shard writes idempotent by
  `(source_shard, event_reference)`, row `complete` only when all shards applied; a mid-write
  crash re-drives (skip done, finish rest).
- **Defer, don't block.** Reading a source is valid only if it's settled through the read tic
  (`read_rule`/`resolved_through`); cross-entity reads resolve at **≤ T−1**. If not ready, defer
  (leave it, work others, revisit) — never spin.
- **Deterministic composition.** Multiple rows into one `(entity, tic)` compose in
  `event_reference` order, not arrival order.
- **Release holds on `complete`**, in the same transaction — tied to the terminal state so a
  crash can only leak a hold (harmless), never release early.

## Master barrier + GC

- **`drop_timed_out` then `bump`**, each tic — the master cancels stale pre-queued rows before
  the tic rolls. Drop targets only `enqueue`/`queueing` rows (reducer-serialized vs. a worker's
  `in_queue` write).
- **GC is the trivial rule**: reclaim a `state_log` row iff **no holders + not-latest + old**.
  No horizon heuristic. Cold compaction is a `PACK` action, not GC.

## `shared/tick` changes

- Retire the per-`ACTION_*` arms of `apply_event` — the interpreter subsumes them.
- ✅ **Keep `read_rule`** (`resolved_through`) — the defer/`await` substrate.
- ✅ **Delete `Phase` and `priority`** (`actor_read_tic`) — same-tic actor-read machinery the
  ≤ T−1 rule replaces (div #9). No cycles → no cycle-breaker.
- Keep the fence; it fences **rows** (per phase), not `(entity, tic)`.

## Verify — on the stack

1. `rd build worker` + redeploy the module (regenerate bindings).
2. In the browser: the 4 wolves still spawn and move — the `SPAWN`/`MOVE` path, now driven by the
   full enqueue→execute lifecycle.
3. Watch the **stale master/worker-binary** failure (worker runs an old binary, silently
   disconnects, `tic_meta` empty → nothing resolves). Rebuild the worker on every op-set change.

## Why this stage is the risk

Determinism, fencing, and the whole recovery story live here: `≤ T−1` reads make the graph a DAG
(no deadlock), `event_reference` composition makes re-execution deterministic, and idempotent
convergent writes make crashes safe. Prove all three on the stack — don't assume them (the phony
`Phase` came from assuming).
