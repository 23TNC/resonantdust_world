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
- `event_status` / `state_status` pack + accessors (`flags:4 | status:4`) + the `QUEUED`/`QUEUEING`/
  `QUEUE_SUCCESS`/`RUNNING`/`COMPLETE`, `FAILED`/`PROMOTE`, `OPEN`/`PROMOTED` constants.
- **The word format** — blocked, see `blockers.md` B-1. Everything below that *interprets* `actions`
  waits on it; nothing else here does.

**Done when:** `cargo test` green in `shared/codec`, every layout matches `VARIABLES.md` field for
field, and each new packer has a test that saturates its fields (proves the spans are adjacent and
exhaust the word) and one that proves an over-range argument can't bleed into a neighbour.

**Watch:** the codec is the *code of record*, not the source of truth. If an implementation and
`VARIABLES.md` disagree, the code is the bug.

---

## W2 · `event_shard` module — the queue + the log

**Component:** `server/spacetime/server/modules/event_shard` only.
**Surface it exposes:** reducers (`queue`, `request_work`, `enqueue_done`, `running`, `fail`,
`complete`, `settle`) + the `event_log` / `event` tables.
**Consumes:** `shared/codec` (W1). Calls `declare_pending` on a data shard — **stub it**; W2 does not
build W3.

Plan: [`event_shard/plan/`](../../components/server/spacetime/modules/event_shard/plan/README.md).
Phases 1–2 are unblocked; 3–4 need the word format only where they extract targets.

**Done when**, with **no worker and no data shard in existence**:
- `spacetime call queue` inserts a row whose `event_reference` is an `entity_reference`
  (`server_reference:8 | ++counter:24`, *not* `auto_inc` on the column — that would increment the
  server byte and destroy the global order), `event_tic == master_tic + 3`, `status.status == QUEUED`.
- `request_work` assigns an ascending bounded batch, reclaims an expired lease **to the phase's
  start**, and fails what can no longer make its tic.
- `settle` writes **one `event` row per zone** the targets occupy, then deletes the queue row.
- A filtered subscription actually delivers a row the reducer just stamped. **Prove this in W2** —
  the entire model rests on it and every later item assumes it.

---

## W3 · `data_shard` module — composition + state

**Component:** `server/spacetime/server/modules/data_shard` only.
**Surface it exposes:** reducers (`declare_pending`, `request_state`, `apply`, `reap`, `gc`) + the
`state_log` / `state_events` / `state` tables.
**Consumes:** `shared/codec` (W1). Independent of W2 — the two shards never call each other.

Plan: [`data_shard/plan/`](../../components/server/spacetime/modules/data_shard/plan/README.md).

**Do first, before any of it:** verify SpacetimeDB accepts
`WHERE worker_a = 1 OR worker_b = 1 OR worker_c = 1 OR worker_d = 1`. Subscription SQL is a string
subset that fails at **runtime**; if rejected, four subscriptions is the fallback. An hour now, or a
redesign later.

**Done when**, with **no worker in existence**:
- `declare_pending` creates the slot, increments `dirty`, pushes the `state_events` reverse index,
  latches `PROMOTE`. Re-driving it does **not** double-count — idempotence is what lets a reclaimed
  event redeclare.
- `request_state` claims a slot on **every** row for the entity (that's what hands the worker the
  history), stamps `lease_*`, and returns false on the fifth distinct worker.
- `apply` composes, decrements `dirty`, rejects when an earlier tic for that entity is still dirty,
  and `state.upsert`s only on `PROMOTE` + settled.
- `reap` frees an expired slot; `gc` drops old settled rows.

---

## W4 · `server/worker` — the resolver

**Component:** a new `server/worker` crate. The old one was deleted; do not resurrect it.
**Surface it consumes:** W2's and W3's reducers, via `spacetimedb-sdk`, plus its two subscriptions.
Nothing else. It has no tables of its own and no other component talks to it.

The whole flow in one process: `request_work` → declare the slots (targets read off `actions`, routed
by each reference's top byte) → `enqueue_done` → claim `state_log` slots **early, in the same tic** →
next tic, compute from the payload on the subscribed row → `apply` → `complete`.

**Done when:** one worker drives one event from `queue` to a composed `state_log` row against live
modules — and its subscriptions are the only way it learns anything. If it reads a table it isn't
subscribed to, the design is being violated, not extended.

**Watch:** `blocked` is *not* worker state — it's "an earlier tic for my target is still dirty",
read off rows it already holds. Defer and revisit; never spin, never block.

---

## W5 · `server/master` — the metronome

**Component:** a new `server/master` crate. The old one was deleted.
**Surface it consumes:** `bump`, `settle`, `reap`, `gc`.

Advances `master_tic` every `1/TIC_HZ`, then sweeps. Liveness lives here **because the master is the
one thing guaranteed to run** — a worker may never ask.

**Done when:** the tic advances, expired leases are reaped, terminal events settle, old slots are
GC'd — and it survives the tic wrapping (u16, ~9h at 2 Hz). Every comparison is `tic::`, never `<`.

---

## W6 · `server/edge` — the door

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

## W7 · `client/core` — reconcile

**Component:** `client/core` only.
**Surface it consumes:** the WS protocol (W6).

It does not build today: `entity_ref_is_positional`, `entity_ref_reference_id` and `pack_hot_entity`
are gone with the union. Its `StateRow` decode is the *old* pipeline's.

**This is a rewrite of that path, not a repair.** The mover decode assumed a `reference_id` variant
tag; the type now comes from the server byte (`entity_ref_type_id`). Port the intent, not the code.

**Done when:** it builds, and a subscribed zone's `state` rows render as movers.

---

## W8 · `shared/wasm`, `client/npc`, `client/pixijs` — follow

**Components:** three, done **one at a time**, in this order — each consumes only the one before.

- `shared/wasm` — the JS bridge; `LoggedIn` already carries `playerShardReference`.
- `client/npc` — `pack_hot_entity` → `pack_entity_reference`; wolves need a `server_reference` with
  `TYPE_PAWN`, not `SERVER_REF_NONE`.
- `client/pixijs` — `MoverLayer` hardcodes `REF_HOT = 1` and filters on it. That constant is gone;
  the type is the server's `type_id` nibble.

**Done when:** `rd build shared`, `core --check`, `npc --check` and `tsc --noEmit` are all green —
the first time since the deletion — and a wolf moves in the browser.
