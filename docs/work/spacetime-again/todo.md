# Todo — spacetime-again

_Planned, not started. W1-W7 are done — see [`completed.md`](completed.md). Move an item to
`remaining.md` when you begin it. Ordered by dependency — each item's surface is what the next one
consumes. **W8 (client/core) is next** — the edge world surface is live (queue + zone state/event
relay, verified over WS); client/core is the Rust client's decode/reconcile of it, a rewrite (the old
`StateRow` decode assumed the deleted union)._

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
