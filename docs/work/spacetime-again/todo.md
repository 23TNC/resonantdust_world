# Todo — spacetime-again

_Planned, not started. W1/W2/W3/W4/W6 are done — see [`completed.md`](completed.md). Move an item to
`remaining.md` when you begin it. Ordered by dependency — each item's surface is what the next one
consumes. **W5 (worker) is next** — its intake (assigned `event_log` + claimed `state_log` rows) is
live, produced by the orchestrator._

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
