# world-storage — pawn (hot), tile & thing (cold), joined by pack/unpack

> **Status: PLAN / intent.** Nothing built. Shapes cite [`VARIABLES.md`](../../VARIABLES.md) +
> [`TABLES.md`](../../TABLES.md). Builds on the live sim core (`intent/spacetime-again/`). Authored
> 2026-07-16.

**What this is for.** Store every world object — pawns, terrain tiles, scattered things — and mutate
it **only** through the event pipeline that's live (edge → orchestrator → worker). Three concrete
shard modules now; **no generic abstraction yet**.

---

## The one rule: everything flows through the pipeline

**No component writes a shard directly — workers do, driven by events. The only direct write is
`init`.** A pawn move, a tile change, a tree chopped: all are events a worker composes into absolute
finals. No side door.

## Hot and cold are **separate shards**

- **Hot** = active entities (pawns, and anything unpacked), keyed by a **globally-unique
  `entity_reference` (u32)**, composed per-tic (`state_log`/`state` — today's `data_shard`).
- **Cold** = packed, mostly-static data (a zone's whole tile/thing layer in one row), keyed by
  **`position` within its module**.

They must be separate because **a position is not a hot identity** — a mover isn't pinned to a cell, so
a hot id is **minted fresh and unique**. The **cold** side, by contrast, *is* pure position: a zone
lives in exactly **one cold shard per type**, so `(macro_position, type_id)` (the `type_id` rides in
`layer_reference`) routes to that one shard, and position fully resolves the cold object — no separate
cold identity, just position + routing. `GET(position)` and `UNPACK(position)` need nothing more.

## Cold is immutable-in-place; act in hot via `UNPACK` / `PACK`

You cannot edit cold where it sits — you `UNPACK` it into a hot entity, act in hot, and later `PACK`
it back. Both are cross-shard, riding the inter-shard event machinery already live.

- **`UNPACK(position)`** — a sibling of `CREATE`: mint a fresh hot entity with a
  **deterministic-from-event id** (spawn-log, so replay never duplicates — the `spacetime-again`
  deferred resolution), seed its payload from the cold entry at `position`, and **remove + tombstone**
  that cold entry. The client now sees the hot entity, not the cold cell.
- **`PACK(entity)`** — read the hot entity; **if its payload fits the cold payload**, write it back
  into the cold row and clear the tombstone and drop the hot entity. **If it doesn't fit, `PACK`
  fails and the object stays hot** — fine, those are rare enough that widening cold isn't worth it.
- **`GET(position)`** — read the cold entry at a position *without* unpacking. The client/edge needs
  this to **build** a program (click a tile → read what's there → decide what to `UNPACK`), separate
  from the worker's execution.

## Core owns the two-phase — the worker never waits

The hard version had the *worker* wait for the async mint mid-tic. Instead, **core** does the
sequencing (it's already async and subscribed to `state`), and `UNPACK` becomes a **fire-and-forget
event** — the worker mints the hot row and tombstones the cold cell in one pass and completes, no poll.

1. **N=0** — core issues `UNPACK(position)` (often **pre-empted**, below).
2. **~N=3** — the hot row appears in `state`. Core learns its id by **watching for the entity at
   `position`** (one thing per cold cell, so position disambiguates — no alias table).
3. **~N=3+** — core issues the real op with the resolved id; it **gates the op on seeing the row**, so
   it never fires against a not-yet-existing id. The op settles ~N=6.

Because `UNPACK` and the op are **separate events across tics**, no single program both unpacks and
references the result — so there is **no in-program pre-pass** and B-3 (the dropped stack) stays dead.

**Keys are server-minted — the client never picks them.** Spacetime mints every `entity_reference`
(unique, deterministic-from-event via the spawn-log), for `CREATE` and `UNPACK` alike; the client only
*learns* it by watching `state` (there is no client-chosen or local/alias key — a client picking keys
collides the instant two clients pick the same one). **Stopgap in the code today:** the npc picks its
own `wolf_key`s — that survives only because there is one automated player and no ownership model yet;
it is replaced by server-minted `CREATE` in the `pawn`-shard step.

**Pre-emption hides the latency.** Core owns the intent, so when it knows a sequence is coming
(move-to-then-pick-up) it sends the `UNPACK` **alongside the earlier action**. If that action takes ≥
the unpack latency (a multi-tile move easily does), the hot row is ready exactly when the op is issued
— the unpack is effectively **free**. It's **safe by construction**: correctness never depends on the
guess (a wrong one is a wasted unpack GC re-packs; a missing one is just late). Keep it conservative.

## Tombstones stop re-transmission; GC queues the packs

A cold row is cheap to hold subscribed (a whole zone-layer in one row). When a cell goes hot, its
**tombstone** (`cold_removed`, a `tile_reference:u8`) says "this cell is hot — ignore the cold entry,"
so the client never re-fetches a cold row for one changed cell. **GC queues `PACK` events** for idle
hot entities (never packs synchronously); a worker runs each and can **drop the pack** if the entity
is about to be modified again — no stop-the-world, and the hot set stays to *actively-changing*
objects.

---

## The `UNPACK` event itself — fire-and-forget, **deferred** to last

With core owning the wait (above), the worker-side `UNPACK` is a plain event, not an async monster.
What's settled:

- **Unpacks run on the assigned worker.** Grouping puts every unpack of a cell on **one** worker (the
  cell is its conflict-target) — no two workers unpack a tile.
- **The worker mints and tombstones, then completes — no poll.** `cold.unpack(position, hot_server)`
  writes the tombstone tagged with the destination server; `hot.mint(macro_position, payload)` mints
  the hot row (deterministic-from-event id). The worker issues both and is done; it does **not** read
  the row back — core does that. If the mint lands slowly, the component is just **late, never wrong**
  (deterministic id + idempotent mint).
- **The tombstone-tagged server makes it replay-safe.** A takeover worker (or replay) reads the
  tombstone, sees "already unpacked to server S," and skips the re-mint (idempotent by the fixed id).

**Why deferred:** it isn't needed to *render* cold (that's just a subscription) — only to *act* on it,
and it needs the cold storage + the `pawn` hot shard in place first. So we prove storage + rendering,
then add this. It's now a tractable cross-shard event plus core sequencing — not a worker rewrite.

## The three shards

| module | regime | key | payload |
|---|---|---|---|
| **`pawn`** | hot | `entity_reference` (u32) | `definition_reference:u32 \| position_reference:u32 \| data:u8` — today's `data_shard`. Also holds **unpacked** tiles/things (a hot entity is a hot entity) |
| **`tile`** | cold | `cold_row_reference` = `macro_position:16 \| layer_reference:8` (u32; **server = the module**) | `Vec<u16>` — 256 dense `kind_reference`s, index = `tile_reference`, no per-entry tile/data |
| **`thing`** | cold | `cold_row_reference` (same) | `Vec<u32>` — sparse `kind_reference:16 \| tile_reference:8 \| data:8` |

The **server drops out of the cold key** — it's the module. So a cold row key is `macro:16 |
layer_reference:8` (u32), and a cold `(row, tic)` slot is the *same* `u64` `state_uid` as hot
(`reserved:16 | key:32 | tic:16`) — the composition machinery is one shape. A cold **entry** is
addressed by `position_reference` (`macro | tile | layer`); the row is `position − tile`.

Cold modules shard **per server** (the deconfliction), routed by zone via the `index`. Unpacked
tiles/things become ordinary hot entities in the `pawn`/hot shard.

---

## Build order (one module at a time) — storage & rendering first, the async monster last

1. **codec** — `PACK` / `UNPACK` / `GET` palette + signatures (`UNPACK`/`GET` operand = a `position`;
   `PACK` = an `entity`) + the `cold_row`/`tile` split of a `position`. No behavior.
2. **`pawn` shard** — re-scope `data_shard` to `pawn`; wire orchestrator/worker/edge. Build
   **server-minted `CREATE`** (deterministic spawn-id via the spawn-log) and **retire the client-picks-
   keys stopgap** (today's npc `wolf_key`) — the client issues `CREATE` and learns the minted id by
   watching `state`. `UNPACK` reuses this spawn-log. Proves the rename + real minting.
3. **`tile` shard — cold + render, NO unpack.** Cold table (256×`u16`) + `cold_removed` tombstones +
   `GET`; worldgen seeds it (via `init`); edge subscribes the zone's cold rows; client draws the
   ground. **This revives the terrain path** (`edge/index.rs`, `worldgen.rs`, `Event::ColdObjects`) and
   is the big visible milestone — with none of the pack/unpack coordination.
4. **`thing` shard — cold + render, NO unpack.** Sparse `u32`; client draws scatter over the ground.
5. **`UNPACK` / `PACK` automation** — the hard phase (see §Unpack coordination): the cross-shard worker
   path (batch → mint → poll → get), the tombstone-tags-server, and the program pre-pass. Now cold can
   be *acted on*, not just rendered.
6. **GC-queues-`PACK`** — the master (or a scheduled reducer) queues pack events for idle hot entities;
   the worker's pack does the fit-check + drop-if-active.

---

## Open

- **Cold key = `u32`** `macro_position:16 | layer_reference:8` (server is the module) — **ratified.**
  Requires revising `cold_row_reference` `u64 → u32` in VARIABLES (drop `reserved:28`, `type_reference`
  → `layer_reference:8`, server out). Subtype lives in the per-entry `kind_reference:16`.
- ~~Pre-pass operand encoding.~~ **Dissolved** — core issues `UNPACK` and the op as separate events
  and resolves the id by watching `state` at the position, so no single program carries a dynamic id.
- **`PACK` fit-check** — per payload: tile (kind in range, no extra state), thing (kind/tile/data fit).
- **Pre-emption policy** — which sequences core pre-empts an `UNPACK` for (conservative; a wrong guess
  is a wasted unpack→GC-repack). Tune against a running client, not blocking.
