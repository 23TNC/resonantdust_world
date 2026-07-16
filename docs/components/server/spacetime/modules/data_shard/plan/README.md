# Plan — `data_shard` (nothing → built)

_Last updated: 2026-07-16. Nothing is built. Active work:
[`work/spacetime-again/`](../../../../../../work/spacetime-again/README.md) W3._

Flow: [`intent/spacetime-again/`](../../../../../../intent/spacetime-again/README.md). Shapes:
[`TABLES.md`](../../../../../../TABLES.md). This module owns `state_log` (the composition slots) and
`state` (client-visible latest). Reducers are called by the orchestrator (`claim`) and by workers
(`write`); the master calls `gc`.

## Do this first — DONE (2026-07-16)

SpacetimeDB **accepts** the two-way disjunction as a subscription and **delivers** it — verified live:
`SELECT * FROM state_log WHERE worker_reference = 17 OR observer_reference = 17` returned the owning
row in its initial update. No two-subscription fallback needed.

## Phases

1. **Tables.** `state_log` + `state`. `state_uid` is composite (`reserved | entity_reference | tic`),
   entity-major, and *is* the key — so `entity_reference` and `tic` are duplicated out as columns
   (a subscription filters on columns, a reducer needs values). `dirty` is a **boolean** (one worker
   owns a component, so binary — not a count). Two role columns: `worker_reference` (writes),
   `observer_reference` (reads as the next tic's base). **No lease** — worker liveness is the
   orchestrator's; re-assignment re-stamps `worker_reference` and fences the old worker out.
2. **`claim`.** The orchestrator calls it per data shard. For each entity: find-or-create `(E, tic)`
   (`dirty = true`), stamp `worker_reference`; find E's most-recent row `< tic` (always exists — the
   latest per entity is never GC'd) and stamp `observer_reference` on it. Confirm the §Do-first
   subscription actually delivers the two roles.
3. **`write`.** The worker calls it, per shard, with **absolute final** values. Require caller ==
   `worker_reference`. Skip a row already `!dirty` (a replay). Set payload, clear `dirty` (this
   unblocks the observer). `state.upsert` if `PROMOTE` and not yet `PROMOTED`. **Idempotent by
   construction** — absolute values from immutable `T-1`, so a re-execution writes the same thing.
4. **`gc`.** Drop `!dirty` rows that are **not** the latest for their entity and older than the
   horizon. **Never the latest per entity** — it is every base. (No `reap` — nothing on `state_log`
   expires; a stale `worker_reference` is overwritten by the orchestrator's next `claim`.)

## Watch

- **The block is the *worker's* job, and this module cannot enforce it.** `A += B` reads B, possibly
  on another shard; `write(A)` can only check A's own chain, not B's dirtiness. Do **not** pretend a
  local fence covers cross-shard reads — it doesn't, and a stale read writes a wrong-but-clean value.
  The check `require(caller == worker_reference)` is the only fence here; correctness of the *base*
  rests on the worker having blocked. (See intent §Why 4.)
- **Every tic comparison is `tic::` serial arithmetic, never `<`.** Wrapping ring; `<` inverts.
- **`state.macro_position_reference` is a projection of the payload**, unenforced. Disagreement makes a
  row invisible in its real zone and visible elsewhere. The move verb is where that breaks.
- **The GC horizon must stay `<< TIC_WINDOW` (32767)** or tic comparison silently inverts.

## Ties into

- **orchestrator** — calls `claim`.
- **worker** (`server/worker`) — subscribes to `state_log`, calls `write`.
- **master** — calls `gc`. This module and `event_shard` never call each other.
