# S4 — Producers → word streams

**Goal:** move the event producers onto the DSL and retire the scalar `ACTION_*` surface.
After this stage the DSL fully owns the existing gameplay path. Completes **divergence #1**.

**Status:** ✅ **done + live-verified** — the edge's word-stream producers (`vm::encode_*`) + the master drop→bump metronome. Authoritative state: [`work/spacetime-rewrite/completed.md`](../../../../../../../work/spacetime-rewrite/completed.md). **Depends on:** [S3](s3-worker.md).

## Changes

- [`server/edge/src/ws.rs`](../../../../../../../../server/edge/src/ws.rs): the spawn and move paths build a
  `Vec<u64>` word stream (`tile, object, MOVE`; the spawn equivalent) + a `targets` list, and
  `append` them as a batch at **`event_tic = master + 3`** (the widened gap) instead of calling
  `append_event` with `resonantdust_tick::ACTION_MOVE` / `ACTION_SPAWN` scalars at +2
  (current call sites ~`ws.rs:477`, `ws.rs:539`).
- A small **encoder** (surface intent → `Vec<u64>`) lives edge-side. The client/npc
  `Command::{Spawn, Move}` surface ([`client/core/src/protocol.rs`](../../../../../../../../client/core/src/protocol.rs))
  is **unchanged** — only the edge's translation to the shard changes.
- Remove the now-dead scalar `ACTION_*` constants from `shared/tick` once no producer references
  them — **including `action_phase` and the `Phase` bands** ✅ *(done 2026-07-14, T-9 — deleted
  with the priority-DAG; divergence #9 closed)* (no phase in this design; ordering
  is tics + `await`, [S3](s3-worker.md)/[S5](s5-control-flow.md)).

## Verify — on the stack

Spawn + move end-to-end in the browser, driven entirely by word streams. Diff behavior against
pre-S4 (should be identical — same effects, new encoding).

## Milestone

The DSL now owns the existing spawn/move gameplay. [S5](s5-control-flow.md)–[S7](s7-tail.md) add
the *new* capability the design calls for (plans, hot/cold ops, cold convergence).
