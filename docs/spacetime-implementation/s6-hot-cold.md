# S6 — `PACK` (settle); cold→hot mint is already in enqueue

**Goal:** the hot↔cold bridge. The **cold→hot half is already done in [S3](s3-worker.md)** —
promoting a cold *target* is absorbed into enqueue's `find-or-mint` stand-up, so there's no
`MINT`/`GET` verb. This stage adds the reverse (`PACK`, settle) and retires the standalone
reducers. Closes **divergence #2**.

**Status:** ⬜ not started. **Depends on:** [S3](s3-worker.md) (which already mints on enqueue),
[S5](s5-control-flow.md). Design: [hot-cold.md](../spacetime-tables/hot-cold.md).

## What S3 already gave us (no work here)

- **cold→hot on target** — enqueue's `stand_up` does `find-or-mint` by location for a
  `cold_reference` target (idempotent, reuses an existing hot entity). The action's target is a
  `hot_reference` by execute time.
- **rebinding** — gone; there's no execute-time `GET`, because the mint happened at enqueue.

## Changes here

- **`PACK`** (settle hot→cold + compact) — an enqueued, worker-resolved action, **not a GC job**
  ([hot-cold.md](../spacetime-tables/hot-cold.md)):
  - for each hot object at rest here with **zero holders**, append its `object_kind_reference`
    into the cold row and delete its hot `state`/`state_log`; skip any with holders (stays hot);
  - apply the zone's `cold_removed` tombstones and clear the delta; stamp the cold row with the
    tic it became cold.
  - The refcount-check + cold-write is **one atomic transaction** (single shard).
- **Re-unpack race** — an event that finds its target already cold verifies the cold data is
  consistent, then tombstones + re-mints (the `find-or-mint` path).
- **Delete the standalone `pack` / `unpack` `#[reducer]`s** — `unpack` is subsumed by enqueue
  `find-or-mint`; `pack`'s logic is now the `PACK` action.
- ❓ **What triggers `PACK`** (edge / master / periodic sweep over zones with settled hot objects
  or accumulated `cold_removed`) — needs an owner.
- **(Optional) explicit mint of a *non-target* object** — the only cold→hot case not covered by
  enqueue. Add a verb only if a plan needs it; defer until a real use appears.

## Verify — on the stack

- "Open the crate" (now **two** rows: `move` → `await ? inspect`) resolves end-to-end — the
  crate is minted at `inspect`'s enqueue, inspected at execute.
- A hot object `PACK`s back to cold and disappears from `state`, reappears in `cold`.

## Notes

The generic interpreter still never grows a per-verb branch for cold handling — the one
structural cold operation at execute is `PACK`; cold→hot is a data-shard reducer called during
enqueue, not DSL.
