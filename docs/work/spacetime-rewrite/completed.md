# Completed — spacetime rewrite

Executes: the `shard` component (design in `docs/spacetime-tables/`; plan detail in
`docs/spacetime-implementation/s0–s7`). Done + verified work, chronological (oldest first).
Each row: date · item · verification · commit. Rich detail for the full-design mechanisms
follows the log. (Was `docs/spacetime-implementation/completion-log.md`.)

## Log

- **2026-07-13** · **S0–S7 core hot path** — codec word frame, DSL interpreter, pipeline module
  (event_log/state/holder/cold/meta + lifecycle reducers), worker two-phase loop, master
  drop→bump, edge word-stream producers, await-gate/abort, terrain `seed_cold_row`, cold→hot
  `find-or-mint`, hot→cold `pack_settle`, legacy module retirement (div #3). Live-verified with
  real binaries. · commits …d47f152
- **2026-07-13** · **#1 Deterministic composition + tic-gated execution** — worker resolves only a
  sealed tic (master ≥ event_tic); each target folds ALL its events by `event_reference`. Live:
  two same-tic moves gated then folded 34→51. · 830d6ba
- **2026-07-13** · **#2 RUNNING-row recovery** — `claim` re-claims a RUNNING row on lease expiry.
  Live: takeover refused while lease held, succeeded after expiry. · 8571018
- **2026-07-14** · **#3 Actor-reading verbs (DAMAGE) + read-rule defer** — `Reads` trait; OBJECT
  operand (mint_server,entity_id, D3 48-bit identity) → actor hp at ≤T−1; unsettled actor defers.
  Live: hp 50 → victim 100→75; unit dead-actor void. · 0bdd2f3
- **2026-07-14** · **#4 Convergent cross-shard writes** — foreign targets apply via idempotent
  `resolve_foreign` (dedup by (source_shard, event_reference) in `applied_foreign`); row completes
  once all shards ack; re-drive dedups → exactly-once. Live on TWO real shard DBs. · 1843276
- **2026-07-14** · **#5 Forward SKIP branch (`?:`) + FAIL + multi-action vectors** — index-driven
  `run`; skip-if-false + `encode_if`; FAIL halt; multi-verb programs. 32 unit tests. · 2749902
- **2026-07-14** · **fix: home_shard routes unminted targets locally** — `mint_server == 0`
  (SERVER_REF_NONE) is unminted ⇒ resolve local, not foreign shard 0. Regression from #4, caught
  by the first real end-to-end run (wolves stuck at kind 0). · d14cf1b
- **2026-07-14** · **#6 Integration — self-driving dev stack + in-browser render** — `rd run`
  stands up worker+master (issue 004 B, light form); full stack (npc→gateway→edge→shard→worker→
  state) renders a moving wolf pack in pixijs. Client already new-schema (no porting). · e78db22
- **2026-07-14** · **`/pause` + `/unpause` simulation freeze** — `tic_meta.paused` + `set_paused`;
  `bump` no-ops paused; master skips paused shards; edge relays `Paused` to all subscribers;
  client/core + wasm + npc + pixijs chat wired. Live: browser `/pause` froze the tic, npc stopped,
  `/unpause` resumed. · b5337d8

---

## Detail — the full-design mechanisms (matching `docs/spacetime-tables/` in full)

### #1 Deterministic composition + tic-gated execution
The worker's `execute` resolves a row only when `master_tic ≥ event_tic` (the tic is *sealed*, no
further event can target it), and computes each target's value as `base@(tic−1)` folded over
**all** applicable events targeting that `(entity, tic)` in **`event_reference` order** — not
arrival order, not last-writer-wins. Idempotent + order-free. Realizes the designed `+3` latency.

### #2 RUNNING-row recovery
`claim` re-claims a `RUNNING` row when the fence is free (unowned, ours, or **lease-expired**) —
a worker that died mid-execute is taken over (status stays `RUNNING`, reassigned).

### #3 Actor-reading verbs (DAMAGE) + read-rule defer
`Reads` trait: an `OBJECT` operand `(mint_server, entity_id)` (the 48-bit identity that fits the
word — no re-key, per D3) resolves to the actor's hp at `≤ T−1`. `DAMAGE` = `[OBJECT(actor),
LITERAL(amount), ACTION(DAMAGE)]`: a **live** actor's blow lands; dead/absent/unsettled reads 0
and voids. Worker enforces the **read rule** (`resolved_through`): a row whose actor isn't settled
through `≤ T−1` defers, and such events aren't folded (`applicable`). The full `hot_reference`
re-key (#5) is *not* needed for actor-reads after all.

### #4 Convergent cross-shard writes
The worker reads each target's base from its **home shard**, partitions targets by home shard,
applies foreign groups first via that shard's `resolve_foreign(source_shard, event_reference, tic,
results)` — **idempotent** by `(source_shard, event_reference)` recorded in `applied_foreign` —
and only once **every** foreign shard acks does it `resolve` the local group (which completes the
row + releases holds). Crash before the final resolve → re-drive re-applies (foreign dedups) then
completes → **exactly-once, eventually-consistent**, never 2PC. *Follow-up (see todo):* a foreign
target gets no Phase-1 pending row/holder on its home shard during the in-flight window.

### #5 Full control flow — forward `SKIP` branch + multi-action vectors
`run` is index-driven: a **forward-only `SKIP`** pops a `LITERAL` count + boolean, jumps forward
when the boolean is false (`skip-if-false`); `FAIL` halts (abort branch). `encode_if(cond, then,
else)` composes `cond ? then : else`. No backward jumps ⇒ bounded execution. A program may carry
multiple action words (multi-verb row), applied left-to-right. Condition may come from an `OBJECT`
actor read. Rides the same `vm::run` the worker drives live.

### #6 Integration — self-driving dev stack + in-browser render
(a) **worker/master standup** via `rd run <up|worker|master|…>` (issue 004 option B, light form:
one persistent `rd-run-<env>` container, `--network host`, native SDK binaries vs
`resonantdust-<env>-zone-0`). (b) **browser client** — `rd redeploy --run` + `rd up gateway` +
`rd index seed` bring up edge/gateway/index; `npc` drives a wolf pack; pixijs renders it. Verified:
4 wolves spawn kind 7 and move in `state` AND render as moving pawns in the browser.
