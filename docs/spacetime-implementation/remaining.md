# Remaining work — honest ledger

The spacetime rewrite's **core is implemented and live-verified** (spawn/move/await/abort/drop/
GC/find-or-mint/PACK, real binaries end-to-end). But the ✅ marks in the [status table](README.md)
mean **"the hot/core path works,"** not "every sub-feature of the design stage is built." This
lists what's genuinely still open, so nothing is lost. Grouped by kind.

## A. Simplifications inside "done" stages (the ✅s don't cover these)

These were deliberately scoped out to land the working core; the design ([../spacetime-tables/])
specifies them.

- **Verbs — only `MOVE`/`SPAWN`** (`shared/tick/vm.rs::domain_action`). `DAMAGE` and other
  **actor-reading / cross-entity** actions are not implemented (they map to a no-op).
- ~~Execution is not tic-gated~~ — **DONE** ([completion-log](completion-log.md) #1): the worker
  resolves only a sealed tic (`master ≥ event_tic`).
- **Single-shard only — no cross-shard convergent writes.** `resolve` commits all a row's
  `TargetState`s in one reducer call on one shard. A row whose targets span shards is not
  handled (no idempotent per-`(source_shard, event_reference)` writes, no "complete when all
  shards applied"). ([../spacetime-tables/lifecycle.md] §convergent write.)
- ~~No deterministic composition~~ — **DONE** ([completion-log](completion-log.md) #1): a
  target's value folds all its events in `event_reference` order.
- **No operand-read defer (`read_rule`).** The hot path (move/spawn) reads no other entity, so
  the "defer until the source is settled through `≤ T−1`" machinery is not implemented/exercised.
  Needed for actor-reading verbs (DAMAGE) and multi-row data dependencies.
- **Control flow is await-gate only.** S5 implements `[LITERAL(timeout), ALIAS, ACTION(AWAIT),
  body]` + defer/abort. The **forward `SKIP` branch / `? then : else`** conditional and a
  **multi-action vector per row** are not implemented; `OBJECT`/`ALIAS` beyond the gate are
  stubs (push-payload). ([../spacetime-tables/event-dsl.md].)
- **`RUNNING`-row recovery.** `claim` advances `enqueue→queueing` and `in_queue→running` only —
  a `RUNNING` row whose worker died isn't re-claimable on lease expiry yet.
- **`event_reference` width.** The `ALIAS` word carries a `u32`; `event_reference` is a `u64` PK.
  Fine while refs are small; reconcile with the hot_reference re-key (#5).

## B. Design tail — functional-neutral reconciliation (also in [divergences.md](../spacetime-tables/divergences.md))

The system works fully without these; they change *representation*, not *behavior*.

- **#4 `region_zone` cold keying** — `cold` keys by flat `zone_id`; target is `region_zone:u16`.
- **#5 `hot_reference` u32 re-key** — `state`/`state_log` key on `u64 entity_key`; target is the
  per-server `hot_reference:u32` (decision D3, deliberately deferred). Big PK change.
- **#10 geographic `server_reference`** — `server_type:6|server_id:10` (functional) vs
  `realm:u8|server_id:u8` (geographic).
- **`PACK` trigger** — `pack_settle` exists; *who* calls it (periodic sweep / edge) is unbuilt.

## C. Infra & integration (different layers)

- **Worker/master run-compose** — [issue 004](../issues/004-running-worker-master.md) option B
  (they run today only via a throwaway container). Needed for a real dev stack.
- **pixijs browser client** — wire the front end to the new `state`/`cold` schema so the game
  renders on the new pipeline. The server pipeline is proven; the client is untouched.

## Verified done (for contrast)

Two-phase lifecycle, tic gap (recorded), fence + same-worker re-claim, refcount holders + dumb
GC, master drop barrier, await/abort, `seed_cold_row`, cold→hot `find-or-mint`, hot→cold
`pack_settle`, legacy retirement (div #3). All live-verified with real binaries.
