# Remaining work — honest ledger

The spacetime rewrite's **core is implemented and live-verified** (spawn/move/await/abort/drop/
GC/find-or-mint/PACK, real binaries end-to-end). But the ✅ marks in the [status table](README.md)
mean **"the hot/core path works,"** not "every sub-feature of the design stage is built." This
lists what's genuinely still open, so nothing is lost. Grouped by kind.

## A. Simplifications inside "done" stages — ✅ ALL CLOSED

These were deliberately scoped out to land the working core; the design ([../spacetime-tables/])
specifies them. The full-design pass ([completion-log.md](completion-log.md) #1–#5) has now
closed **every behavioral one** — only the `event_reference`-width *representation* note remains,
and it's tied to the #5 re-key in section B.

- ~~Verbs — only `MOVE`/`SPAWN`~~ — **DONE** ([completion-log](completion-log.md) #3): `DAMAGE`
  (actor-reading) implemented; more verbs are additive.
- ~~Execution is not tic-gated~~ — **DONE** ([completion-log](completion-log.md) #1): the worker
  resolves only a sealed tic (`master ≥ event_tic`).
- ~~Single-shard only — no cross-shard convergent writes~~ — **DONE**
  ([completion-log](completion-log.md) #4): foreign targets apply via idempotent
  `resolve_foreign` keyed by `(source_shard, event_reference)`; the row completes only once all
  target shards acked; re-drive dedups → exactly-once. **Sub-item still open:** a foreign
  target's **Phase-1 pending row / holder on its home shard** (in-flight read-rule/GC visibility);
  the *write* converges, the in-flight *hold* is eventually-consistent.
- ~~No deterministic composition~~ — **DONE** ([completion-log](completion-log.md) #1): a
  target's value folds all its events in `event_reference` order.
- ~~No operand-read defer (`read_rule`)~~ — **DONE** ([completion-log](completion-log.md) #3):
  actor-reads defer until the actor is settled through `≤ T−1`.
- ~~Control flow is await-gate only~~ — **DONE** ([completion-log](completion-log.md) #5): the
  forward `SKIP` branch (`? then : else` via `encode_if`), `FAIL` halt, and multi-action vectors
  are implemented + tested. `OBJECT` now feeds a branch condition (actor read); `ALIAS` remains
  the worker-level await gate by design.
- ~~`RUNNING`-row recovery~~ — **DONE** ([completion-log](completion-log.md) #2): re-claim on lease expiry.
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

- ~~Worker/master run-compose~~ — **DONE** ([completion-log](completion-log.md) #6): `rd run`
  (issue 004 option B, light form) stands up worker+master as a self-driving dev stack.
- ~~pixijs browser client~~ — **DONE** ([completion-log](completion-log.md) #6): the client was
  already on the new `state`/`cold` schema; the full stack (npc→gateway→edge→shard→worker→state)
  renders a moving wolf pack in the browser. Found+fixed the `home_shard` mint_server=0 bug.

## Verified done (for contrast)

Two-phase lifecycle, tic gap (recorded), fence + same-worker re-claim, refcount holders + dumb
GC, master drop barrier, await/abort, `seed_cold_row`, cold→hot `find-or-mint`, hot→cold
`pack_settle`, legacy retirement (div #3). All live-verified with real binaries.
