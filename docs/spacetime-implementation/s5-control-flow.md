# S5 — Control flow: `alias` / `AWAIT` / `TIMEOUT` / `?:` / `FAIL` / `LITERAL`

**Goal:** let a row block on another row, so multiple rows form a plan. Not one program run
across tics — **one action string per row**, rows chained by `await` on aliases
([event-dsl.md](../spacetime-tables/event-dsl.md) §Multi-step plans).

**Status:** ⬜ not started. **Depends on:** [S4](s4-producers.md), decision **D1**
([s0](s0-foundation.md)).

## The model (user-defined — do not add a "phase")

- **`alias` = a stand-in for an `event_reference`.** A row's `event_reference` is minted at
  write, unknown at compose time; the alias names "the row I'm about to write," and `append`
  populates it with the real `event_reference` as it writes the batch ([S2](s2-event-schema.md)).
  No persistent alias — `await` operates on the concrete `event_reference`. An action that
  **returns** an `event_reference` can push it directly (the cross-shard / dynamic path).
- **`await` *defers*, it never blocks.** When the worker reaches a row awaiting an incomplete
  alias, it **checks the `TIMEOUT`** (in **tics**): past it → run the `FAIL` branch; not past it
  → **leave the row pending, move on to other events, revisit on a later pass.** The worker must
  never spin/hold on an `await` — blocking one row would stall every other row (probably breaks
  us). This is exactly the worker's existing park-and-retry behavior (`resolve_one` returns
  without committing when a dependency isn't ready) — reuse it.
- **Queued at tics; executed in tic order; intra-tic order not guaranteed.** Cross-row order is
  expressed *only* by `await`.
- **Worker-triggered rows queue at tic+2 or later — never tic+1.** A worker resolving tic+1
  work queues follow-ons to a future tic it isn't processing, appended **inside the same atomic
  `resolve`** that marks the row complete. That's the **dedup**: a re-resolve finds the row
  complete (skip) or redoes the whole bundle (first committed nothing) — no duplicate follow-on,
  no provenance key needed (III).
- **`await`'s required `TIMEOUT` guarantees termination.** Every `await` has a tic timeout, so a
  deferred row always reaches a terminal state → releases its **holders** → GC can reclaim. A
  vanished dependency (dropped/failed/crashed) just times the dependent out; no cascade.

## Changes — interpreter op set

- **`LITERAL`** — push a constant (`TIMEOUT` budgets, ids, counts).
- **`ALIAS`** — push an `event_reference` (a populated alias, or one an action returned) for
  `AWAIT` to test.
- **`AWAIT` + `TIMEOUT`** — check the aliased row's completion (readable from `event_log`
  status / the target's resolved tic). If not complete and not timed out, the row **stays
  pending** (re-evaluated on a later tic — it does not resolve yet, holding no fence it can't
  keep). If timed out, take the `FAIL` branch.
- **`SKIP` (D1)** — forward-only branch: `?` skips the else/then run by a `LITERAL` word-count.
  No backward jumps → bounded.
- **`FAIL`** — abort this row / branch.

## The completion check

`await X` reads **X's `event_log` `status`** (`complete` vs not) — or, equivalently, whether X's
target is resolved through the relevant tic (`read_rule`). Either is deterministic under the
fence; the `event_log` status is the simpler read. (The old "how much of `read_rule`/`priority`
survives" question is resolved: **keep `read_rule`, drop `priority`/`Phase`** — [S3](s3-worker.md).)

## Verify

- **Unit:** a two-row plan (`c: move` ; `e: await c ? … : fail`) resolves across tics — exercise
  both the completed and the timed-out/`FAIL` branches.
- **Stack:** the same two rows issued through the edge resolve in the browser.
