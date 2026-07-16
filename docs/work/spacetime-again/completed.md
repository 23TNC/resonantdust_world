# Completed — spacetime-again

_Done and live-verified against its own surface. Newest at the top. Each entry is the item as it was
carried out — the "done when" bar it cleared._

---

## W4 · `server/orchestrator` — the grouper ✓ 2026-07-16

New `server/orchestrator` crate — a tokio SDK client over `server/st-bindings` + `shared/codec`.
Live-verified against master + both shards.

- **Intake, master-assigned.** The master hands each event shard its orchestrator at standup
  (`event_shard.set_orchestrator`, stored in a new public `orchestrator` row); `queue` stamps that
  reference onto every event and **rejects if none**. The orchestrator subscribes
  `event_log WHERE orchestrator_reference = self` + `clock`.
- **Drain-all-frozen loop, not one-tic-per-pass.** Each pass assigns every frozen (`event_tic ≤
  master+2`) still-`QUEUED` event in the cache, grouped per tic — one tic normally, a backlog of
  several on catch-up/takeover. Never groups the open tic (`master+3`), so union-find always runs on
  a complete set (the completeness barrier).
- **Union-find by write target** (pure, unit-tested in `grouping.rs`, 6 tests): events sharing a
  write target merge transitively into one component → one worker. Reads don't conflict (`< tic`).
- Per component: `event_shard.assign(events, worker)` + `data_shard.claim(entities, tic, worker)`,
  least-loaded worker across the pass. Idempotent; a local `assigned` set skips redundant calls and
  is pruned to the live queue (so a fresh orchestrator re-drives everything).
- **Live proof:** queued PLACE A, MOVE_TO A, PLACE B → the two on A **merged** into one work-group
  (`events=2, entities=1`), B was its own group; all three `event_log` rows stamped `orch=0x61`,
  `worker=0x62`, `ASSIGNED`; `state_log` got exactly two `dirty` slots (A, B) — A once, despite two
  events (claim dedups the component's entities).

**Deferred (belongs to later items):** `CREATE`'s spawned slot (its minted id isn't a write operand,
so a bare `CREATE` is a singleton with no claim — the worker/spawn path, W5, resolves the id);
multi-data-shard `claim` routing (one shard today); orchestrator liveness/takeover (§Open); stragglers
(can't occur under single-pass union). New reusable `bin/sim` build/run path for the SDK-client sim
crates (master/orchestrator/worker) — cached builder image, `build`/`check`/`run`/`stop`/`logs`.

## W6 · `server/master` — the metronome ✓ 2026-07-16

New `server/master` crate — a tokio SDK client over `server/st-bindings`. Live-verified.

- Advances `master_tic` every `1/TIC_HZ` on `event_shard` + `data_shard` in **lockstep** via each
  shard's `bump` (absolute write; the master is the sole writer, so a call is self-correcting).
- **Seeds from the persisted clock, never resets.** The startup bug: reading the clock before its
  subscription applied returned 0 and clobbered the persisted tic. Fixed — wait for `on_applied` on
  both clock subscriptions, seed a local counter once, then own it locally (never re-read). Verified
  12→24 over 3 s at 4 Hz, both shards equal, no reset.
- Sweeps each tic: `settle(next-3)` drains terminal events (a tic seals once no append can reach it —
  appends land at master+3), `gc(next-GC_BEHIND)` every `GC_EVERY` tics.
- u16 tic ring — `wrapping_add`; all comparison elsewhere is `tic::` serial math.

**Deferred to when the orchestrator exists (W4):** orchestrator-per-tic assignment + dead-orchestrator
reaping. The master is the guaranteed-to-run liveness owner, but there is no orchestrator to assign
yet. Metronome + settle + gc are the live-verified core.

Prerequisites folded into the same commit: `server/st-bindings` (shared SDK bindings crate);
`clock` made `public` in both modules (an SDK client can't read a private table); regenerated edge
bindings; TABLES.md documents `clock` as public.

---

## W3 · `data_shard` module — composition + state ✓ 2026-07-16

`state_log` (per `(entity, tic)` composition slot) + `state` (client-visible latest), reducers
`claim` / `write` / `gc` / `bump` / `init`. Live-verified with `spacetime call`, no orchestrator or
worker in existence.

- **Do-first cleared:** `WHERE worker_reference = 17 OR observer_reference = 17` is accepted **and
  delivers** — no two-subscription fallback.
- `claim` creates the `(E, tic)` slot `dirty`, stamps `worker_reference`, stamps `observer_reference`
  on E's serially-previous row. No lease; a re-`claim` overwrites the stamp (the eviction).
- `write` fences caller == `worker_reference`, skips `!dirty` (replay-safe), writes **absolute
  finals**, clears `dirty`, and `state`-upserts only on `PROMOTE` + not-yet-`PROMOTED`.
  **Idempotent** — verified by replaying the same call.
- `gc` drops old `!dirty` rows but never the latest per entity (every future tic's base).
- Observer chain + promotion verified live.

---

## W2 · `event_shard` module — the queue + the log ✓ 2026-07-16

`event_log` (in-flight queue) + `event` (settled, client-visible), reducers `queue` / `assign` /
`running` / `complete` / `fail` / `settle` / `bump` / `init`. Live-verified, no orchestrator/worker/
data shard in existence.

- `queue` mints `event_reference = pack_entity_reference(SERVER_REFERENCE, ++counter & 0xFF_FFFF)`
  (not column `auto_inc`), stamps `event_tic = tic_add(master, TIC_GAP=3)`, validates the program via
  `action::Program`, latches `PROMOTE`.
- `settle(through_tic)` drains terminal rows and fans `event` out **one row per zone** the targets
  occupy (`pack_event_uid`), then deletes the queue row.
- Filtered subscription (`WHERE orchestrator_reference = self` / `WHERE worker_reference = self`)
  delivers a just-stamped row — verified. `SERVER_REFERENCE = pack_server_reference(TYPE_EVENT,0) =
  0x50`.

---

## W1 · `shared/codec` — the vocabulary ✓ 2026-07-16

The pack/unpack surface every other component consumes. `cargo test` green.

- `uid.rs` — `pack_state_uid` (`reserved:16 | entity_reference:32 | tic:16`, entity-major) +
  `pack_event_uid` (`macro_position_reference:16 | event_tic:16 | event_reference:32`) + accessors.
  Layouts pinned to `VARIABLES.md`, saturation + no-bleed tests.
- `status.rs` — `pack_status(flags:4, status:4)` + accessors + constants (`EVENT_QUEUED`…
  `EVENT_COMPLETE`, `EVENT_FLAG_FAILED`/`_PROMOTE`; `STATE_OPEN`/`STATE_PROMOTED`, `STATE_FLAG_PROMOTE`).
- `action.rs` — palette + `OperandKind` signature table + `arity` + a `Program` iterator that frames
  by arity and errors on `UnknownAction`/`Truncated` (arity is wire — no re-sync point), plus
  `write_targets` / `read_targets` / `asks_promote_event`. Multi-action round-trip + mis-frame tests.
- `refs.rs` — `pack_server_reference` made `const fn` so a shard can name `SERVER_REFERENCE` in a
  `const`.
