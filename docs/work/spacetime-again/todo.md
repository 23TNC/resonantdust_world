# Todo — spacetime-again

_Planned, not started. Newest thinking at the top of each item; move an item to `remaining.md` when
you begin it. Ordered by dependency — each item's surface is what the next one consumes._

---

## W1 · `shared/codec` — the vocabulary

**Component:** `shared/codec` only.
**Surface it exposes:** the crate's public API. Every other item packs and unpacks through it and
never hand-rolls a shift.

- `state_uid` / `event_uid` pack + accessors (`reserved | entity_reference | tic`,
  `macro_position_reference | event_tic | event_reference`). Both are `VARIABLES.md` layouts with no
  implementation — the modules will need them on day one, and four hand-rolled shifts is how a
  nibble order gets reversed.
- `event_status` / `state_status` pack + accessors (`flags:4 | status:4`) + the constants:
  `event_status.status` `QUEUED`/`GROUPED`/`ASSIGNED`/`RUNNING`/`COMPLETE`, flags `FAILED`/`PROMOTE`;
  `state_status.status` `OPEN`/`PROMOTED`, flag `PROMOTE`.
- **The action program** — [`ACTIONS.md`](../../ACTIONS.md). `action_reference` + the palette +
  arity, `pack`/read helpers for the stream, and a signature table (per operand: written / read /
  number) — that table is what the write and read sets are scanned out of, so it belongs here, not
  in the worker.

**Done when:** `cargo test` green in `shared/codec`, every layout matches `VARIABLES.md` field for
field, and each new packer has a test that saturates its fields (proves the spans are adjacent and
exhaust the word) and one that proves an over-range argument can't bleed into a neighbour. For the
stream: a reader that round-trips `ECHO 5 PUSH <action> POP`, and a test that a wrong arity
mis-frames the rest — there is no re-sync point, so arity is wire.

**Watch:** the codec is the *code of record*, not the source of truth. If an implementation and
`VARIABLES.md` disagree, the code is the bug.

---

## W2 · `event_shard` module — the queue + the log

**Component:** `server/spacetime/server/modules/event_shard` only.
**Surface it exposes:** reducers (`queue`, `assign`, `fail`, `running`, `complete`, `settle`) +
`event_log` / `event`, and the shard-local **grouping** (`event_group`) + the completeness barrier.
**Consumes:** `shared/codec` (W1). Calls **nothing** on other components — the orchestrator and workers
call *it*.

Plan: [`event_shard/plan/`](../../components/server/spacetime/modules/event_shard/plan/README.md).

**Done when**, with **no orchestrator, worker, or data shard in existence**:
- `queue` mints `event_reference` (`server_reference:8 | ++counter:24`, *not* column `auto_inc`),
  stamps `event_tic = master + 3`, `orchestrator_reference`, latches `PROMOTE`, and unions the event
  into an `event_group` by shared write-target.
- A `queue` for a tic whose birth window has passed is **rejected**.
- A group request for tic T is **refused until the shard has read master ≥ T-2** (the completeness
  barrier) — prove the refusal, it's a correctness invariant.
- `settle` writes **one `event` row per zone** the targets occupy, then deletes the queue row.
- A filtered subscription (`WHERE orchestrator_reference = self`, `WHERE worker_reference = self`)
  actually delivers a row the reducer just stamped. **Prove this** — the whole model rests on it.

---

## W3 · `data_shard` module — composition + state

**Component:** `server/spacetime/server/modules/data_shard` only.
**Surface it exposes:** reducers (`claim`, `write`, `reap`, `gc`) + `state_log` / `state`.
**Consumes:** `shared/codec` (W1). Independent of W2 — the two shards never call each other.

Plan: [`data_shard/plan/`](../../components/server/spacetime/modules/data_shard/plan/README.md).

**Do first:** verify SpacetimeDB accepts
`WHERE worker_reference = 1 OR observer_reference = 1`. String-subset SQL, fails at runtime; if
rejected, two subscriptions is the fallback. Know before phase 2.

**Done when**, with **no orchestrator or worker in existence**:
- `claim` creates `(E, tic)` (`dirty = true`), stamps `worker_reference` + lease, and stamps
  `observer_reference` + lease on E's most-recent row `< tic`.
- `write` requires caller == `worker_reference`, skips already-`!dirty` rows (replay-safe), sets the
  absolute payload, clears `dirty`, and `state.upsert`s only on `PROMOTE` + not-yet-`PROMOTED`.
- `write` is **idempotent** — call it twice with the same values, same result.
- `reap` clears an expired role; `gc` drops old `!dirty` rows but **never the latest per entity**.

---

## W4 · `server/orchestrator` — the grouper

**Component:** a new `server/orchestrator` crate. An SDK client; it owns no tables.
**Surface it consumes:** every event shard's group-serving + `assign` / `fail` (W2), and every data
shard's `claim` (W3). Nothing consumes *it*.

Per tic T: subscribe `event_log WHERE orchestrator_reference = self`; wait for a **complete** batch
from every event shard (the barrier); union `event_group`s across shards into work-groups (merge any
two sharing an entity); fail a straggler that still spans two; assign a worker per work-group; batch
one `assign` per event shard and one `claim` per data shard.

**Done when:** given events across ≥2 event shards whose targets span ≥2 data shards, it produces
work-groups where **no entity appears in two groups**, and stamps exactly one worker per group onto
the event + state rows. Prove the barrier: assigning before the batch is complete must be impossible
(a late bridging event otherwise splits a component). Stays **stateless** — kill it mid-tic, a fresh
one recomputes the identical partition.

**Watch:** doubling is safe *only* because of the barrier — never assign on a partial view.

---

## W5 · `server/worker` — the resolver

**Component:** a new `server/worker` crate. The old one was deleted; do not resurrect it.
**Surface it consumes:** its two subscriptions + `data_shard.write` + `event_shard.complete`. Nothing
else, and nothing consumes it.

Per assigned work-group: block until every read target's previous row is `!dirty` (**the correctness
requirement** — a local fence can't cover a cross-shard read); compose the whole component in scratch
in `event_reference` order, from `< tic` bases; `write` absolute finals, one call per data shard;
`complete`.

**Done when:** one worker drives one component from assigned → composed `state_log` rows against live
modules, and its subscriptions are the only thing it reads. A cross-entity transaction
(`bob.apples--, alice.apples++` across two data shards) resolves atomically-on-replay: kill the worker
mid-`write` and a fresh assignment recomputes the identical finals.

**Watch:** never write a target while a read target is dirty — that's the corruption case. Defer the
whole component; never partial-write.

---

## W6 · `server/master` — the metronome

**Component:** a new `server/master` crate. The old one was deleted.
**Surface it consumes:** `bump`, `settle`, `reap`, `gc`; also **assigns the orchestrator per tic**
(the leaning resolution — it's singular, lockstep, guaranteed to run).

Advances `master_tic` every `1/TIC_HZ` on all shards in lockstep, then sweeps: `reap` expired roles,
`settle` terminal events, `gc` old rows. Liveness lives here because the master is the one thing
guaranteed to run.

**Done when:** the tic advances in lockstep, orchestrator assignment is stable per tic, leases reap,
terminal events settle, old rows GC — and it survives the u16 wrap (~9h at 2 Hz). Every comparison
`tic::`, never `<`.

---

## W7 · `server/edge` — the door

**Component:** `server/edge` only.
**Surface it exposes:** the WS protocol to clients. **Consumes:** `event_shard.queue` and a zone
subscription on `state` / `event`.

- `queue(actions)` from a validated client request — the edge is where "authenticated · owns the
  object · legal verb · rate" is enforced. It is the only door into the queue.
- Zone subscriptions: `state` and `event` `WHERE macro_position_reference = <zone>`, relayed to the
  client.
- Restore `ClientMsg`/`ServerMsg` for the world surface, which W-shard-deletion stripped to login +
  ping.
- `index.rs` and `worldgen.rs` have been waiting for this since the deletion — `resolve_zone_or_default`
  is `#[allow(dead_code)]` and worldgen has nothing to seed. Decide whether either returns *here*,
  not by leaving them dangling.

**Done when:** a client's move request becomes an `event_log` row, and a composed `state` row reaches
the client that is subscribed to its zone.

---

## W8 · `client/core` — reconcile

**Component:** `client/core` only.
**Surface it consumes:** the WS protocol (W7).

It does not build today: `entity_ref_is_positional`, `entity_ref_reference_id` and `pack_hot_entity`
are gone with the union. Its `StateRow` decode is the *old* pipeline's.

**This is a rewrite of that path, not a repair.** The mover decode assumed a `reference_id` variant
tag; the type now comes from the server byte (`entity_ref_type_id`). Port the intent, not the code.

**Done when:** it builds, and a subscribed zone's `state` rows render as movers.

---

## W9 · `shared/wasm`, `client/npc`, `client/pixijs` — follow

**Components:** three, done **one at a time**, in this order — each consumes only the one before.

- `shared/wasm` — the JS bridge; `LoggedIn` already carries `playerShardReference`.
- `client/npc` — `pack_hot_entity` → `pack_entity_reference`; wolves need a `server_reference` with
  `TYPE_PAWN`, not `SERVER_REF_NONE`.
- `client/pixijs` — `MoverLayer` hardcodes `REF_HOT = 1` and filters on it. That constant is gone;
  the type is the server's `type_id` nibble.

**Done when:** `rd build shared`, `core --check`, `npc --check` and `tsc --noEmit` are all green —
the first time since the deletion — and a wolf moves in the browser.
