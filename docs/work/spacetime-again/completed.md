# Completed — spacetime-again

_Done and live-verified against its own surface. Newest at the top. Each entry is the item as it was
carried out — the "done when" bar it cleared._

---

## W8 · `client/core` — reconcile ✓ 2026-07-16

Rewrote the Rust client's WS-facing layer to the edge's W7 surface (the old bitemporal `StateRow` /
`Row` / `Applied` decode assumed the deleted reference union). Builds green on **native and wasm32**;
decode unit-tested.

- **protocol.rs** mirrors the edge exactly: `ClientMsg` = Login/Ping/Queue/SubscribeZone/
  UnsubscribeZone; `ServerMsg` = LoginOk/LoginErr/Pong/QueueOk/QueueErr/State/StateGone/Event/Error.
- **world.rs** (new, pure, tested): `position_reference` ⇄ global tile; `zone_id` ⇄
  `macro_position_reference` (the macro *is* the middle two bytes of `zone_id`, so the anchor manager
  keeps thinking in `zone_id` and only the wire boundary converts — `zones.rs` untouched); `state_event`
  decodes a row to the host mover; `move_to`/`place` program builders.
- **api.rs**: `Command` gains `Queue`/`Move`/`Place` (typed verbs compile to action programs);
  `Event::StateObject` carries the new model — `entity_reference:u32` (JS-safe), `definition_reference`,
  global `(tile_x, tile_y)`, `facing`, `tic`.
- **engine.rs** + **web.rs** (the native + wasm twins) rewired: subscribe/unsubscribe by macro,
  `Queue`→`ClientMsg::Queue`, `State`/`StateGone`→mover decode, realm taken from
  `player_shard_reference`. The sid map is gone (the protocol keys by zone).

**Done-when:** builds (native `cargo check` + wasm32 `-p client --features web`) ✓; a subscribed
zone's `state` row decodes to a mover (`state_event` test: entity/zone_id/tile/facing/tic) ✓. The
pixel proof ("renders as movers") is W9 (pixijs); the mover event it renders is now correctly produced.

**Deferred (no server source yet):** `Event::ColdObjects`/`Paused` variants kept for W9 compat but
never fire (terrain + pause are the deferred tracks); settled `event` frames aren't rendered as movers.

## W7 · `server/edge` — the door ✓ 2026-07-16

Restored the edge's world surface (stripped to login + ping when the pipeline was deleted).
Live-verified end-to-end through the browser-facing WebSocket protocol.

- **Protocol** (`protocol.rs`): `ClientMsg::Queue { cid, actions }`, `SubscribeZone { zone }`,
  `UnsubscribeZone { zone }`; `ServerMsg::QueueOk`/`QueueErr`, `State`, `StateGone`, `Event`.
- **Queue** — the only door in. Validates (logged in · program frames) and relays to
  `event_shard.queue` via `queue_then`, reporting the reducer's own verdict back as `QueueOk`/`QueueErr`.
  Ownership + rate limiting are future (no ownership model yet).
- **Zone subscriptions** — per client, its own `event_shard`+`data_shard` upstreams (own connection =
  own subscriptions, dodging the set-semantics hazard). `SubscribeZone` adds
  `state`/`event WHERE macro_position_reference = zone`; row callbacks (registered once, on insert/
  update/delete) relay `State`/`StateGone`/`Event` frames. `UnsubscribeZone` drops the handles
  (= unsubscribe).
- **Live proof:** a WS client logged in, subscribed zone 7, and queued `PROMOTE_STATE E, PLACE E`
  → `QueueOk`, then the composed `State{entity, zone:7, position}` row arrived back — the whole path
  client → `queue` → orchestrator → worker → `state` → edge → client.

**`index.rs` / `worldgen.rs` decision:** they stay `#[allow(dead_code)]` — they belong to the
**terrain** (cold-zone) subscription path (`zone → region → shard` routing + terrain seeding), not the
dynamic-sim surface W7 built. The edge subscribes `state`/`event` directly on the single sim shards
today; zone→shard routing returns when multi-shard data routing / the terrain pipeline is wired, not
in W7. Not dangling — deferred to that track.

**Operational note (not a code fix):** login first timed out because the **deployed** `players` module
in `dev` was an older layout than the edge's (correct) bindings — a stale deploy, not stale code.
Redeploying `players` from current source aligned it. (Also hit a WSL2/docker cargo mtime miss — a
regenerated file didn't trigger recompilation until a source `touch`; see [[docker-cargo-mtime-miss]].)

## W5 · `server/worker` — the resolver ✓ 2026-07-16

New `server/worker` crate — a tokio SDK client over `server/st-bindings` + `shared/codec`.
Live-verified against the full stack (master + orchestrator + both shards).

- **Subscriptions are its assignment:** `event_log WHERE worker_reference = self` (its programs),
  `state_log WHERE worker_reference = self OR observer_reference = self` (its slots + their bases),
  `index.master_clock` (the tic). It owns no tables.
- **Per assigned event tic** (ascending serial order): **block** if any target's base (`< tic`) is
  still `dirty` (defer the whole tic — never partial-write); **compose** scratch from each target's
  base, applying every event's program in ascending `event_reference` order; **write** absolute
  finals (`data_shard.write`, one call per data shard); **complete** each event with the zones its
  targets ended up in (`event_shard.complete`).
- **Verbs:** `PLACE` (set position), `MOVE_TO` (arrive — multi-tile stepping + self-requeue is the
  movement-content follow-up), `PROMOTE_STATE` (flag for promotion). `CREATE` deferred (its minted
  id isn't a write operand, so no slot is claimed yet — needs the orchestrator's spawn-id claim).
- **Live proof:**
  - single-entity component (`PROMOTE_STATE A, PLACE A`) → composed, `state_log` clean, `state`
    promoted with `macro_position` derived from the position;
  - **cross-entity event** (`…B…C` in one program) → both B and C composed at one tic, both promoted
    — the transaction is ordinary sequential code in one worker;
  - **serial chain** — a later-tic `MOVE_TO A` composed from A's earlier clean base (A: tic 522 →
    tic 727, `state` moved zones);
  - **opt-in promotion** — `PROMOTE_STATE` populates `state`; `event` stays empty (no `PROMOTE_EVENT`);
    `event_log` drains via `settle`;
  - **stateless takeover / idempotent replay** — with the worker **down** the orchestrator still
    claimed the dirty slot; a fresh worker instance picked up the durable assigned work and drove it
    to completion (absolute writes + skip-clean = replay-safe).

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

## W6 · `server/master` — the metronome (durable index clock) ✓ 2026-07-16

New `server/master` crate — a tokio SDK client over `server/st-bindings`. Live-verified. Revised the
same day to make the tic **durable**, per the user's design.

- **The tic's one durable home is `index.master_clock` (per realm), not the shard clocks.** The
  master runs the metronome and calls `index.bump_tic(realm)` — the reducer owns the increment, so
  the master holds **no counter of its own**. It then copies the tic (low 16 bits) into the event/
  data shard `clock` **mirrors** (`bump`) — the only manual fan-out, because modules can't subscribe
  cross-DB — and sweeps: `settle(tic-3)`, `gc(tic-GC_BEHIND)` every `GC_EVERY` tics.
- **Why the redesign:** the first cut seeded a local counter from a shard clock, which reset to 0 on
  any shard redeploy (and hit a read-before-subscription-applied race). Now the authority lives on
  the index (the singleton control-plane DB that isn't wiped with a shard).
- **Live-verified durability:** (Test C) stop+restart the master → tic **continued** from the durable
  80→89, not reset. (Test B) redeploy a shard → its mirror inits to 0, and the master re-stamps it to
  the current tic on restart (259→269 lockstep). Steady state: all three clocks equal.
- **Gap noted (future hardening):** a shard *destroyed* while the master keeps its now-dead SDK
  connection isn't auto-restored (the fire-and-forget `bump` is silently dropped) — the normal
  redeploy flow restarts the SDK-client binaries (as `rd redeploy --run` does), which reconnects and
  re-stamps. Live SDK auto-reconnect on `on_disconnect` is deferred.

Also folded in earlier: orchestrator-per-tic assignment stayed deferred (single orchestrator now,
master assigns it at standup — see W4). `server/st-bindings` shared bindings crate; `index` bindings
added; `bin/sim` build/run path.

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
