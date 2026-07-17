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

## The program pre-pass — resolving minted ids (replaces the dropped stack)

A program may `UNPACK` a thing and then act on it — but its hot id isn't known until the `UNPACK`
runs. So execution is two passes: **sweep the program's `UNPACK`s (and `CREATE`s) first, run them to
learn each minted id, rewrite the remaining operands with those ids, then execute the rest.** Because
the ids are deterministic-from-event, the rewrite is deterministic and replay-safe.

This is the answer to blocker **B-3** (dropped `PUSH`/`POP`/`ECHO`): a later action *can* reference a
target that didn't exist at queue time — via this rewrite, not a stack. Chosen over `UNPACK` then
`GET(position)`, which needs a mid-program round-trip and the id flowing back. **Open:** how an
operand names "the thing unpacked by step N" for the rewrite (a position stand-in, or a step index).

## Tombstones stop re-transmission; GC queues the packs

A cold row is cheap to hold subscribed (a whole zone-layer in one row). When a cell goes hot, its
**tombstone** (`cold_removed`, a `tile_reference:u8`) says "this cell is hot — ignore the cold entry,"
so the client never re-fetches a cold row for one changed cell. **GC queues `PACK` events** for idle
hot entities (never packs synchronously); a worker runs each and can **drop the pack** if the entity
is about to be modified again — no stop-the-world, and the hot set stays to *actively-changing*
objects.

---

## Unpack coordination — the hard part, **deferred** to last

Automatic pack/unpack is the one genuinely hard piece, so it's built **last** (see build order). What's
already settled about it:

- **Unpacks run at N+1, on the assigned worker.** Grouping puts every unpack of a cell on **one**
  worker (the cell is its conflict-target), which run in sequence — no two workers unpack a tile.
- **The tombstone tags the destination hot server.** `cold.unpack(position, hot_server)` writes the
  tombstone *and* the server it sent the object to. A takeover worker (or a replay) reads that and does
  `GET` instead of re-minting — this is what makes unpack **replay-safe**. The mint itself is
  idempotent (deterministic-from-event id, check-exists-else-mint).
- **The mint is async, so the worker polls.** `cold.unpack` → `hot.mint(macro_position, payload)` (the
  hot shard stamps the worker on the row, so its subscription catches it), but the row isn't visible
  the instant the reducer is issued. So the worker **batches unpacks/creates by destination server,
  issues them, defers to its other work, and loops back**: for each, "does the hot row exist yet? no →
  (re)mint idempotently; yes → `GET`." All resolve within the tic if it's quick; if not, **the pipeline
  is tolerant — the component is just late, never wrong** (deterministic id + idempotent mint).

**Why deferred:** this turns the worker from a *synchronous* composer into one doing async cross-shard
round-trips mid-tic — a real step up from today's worker. It isn't needed to *render* cold (that's just
a subscription); only to *act* on it. So we prove storage + rendering first, then build this with the
storage solid and "late not wrong" as the safety net. The sketch above is the leaning approach, not a
frozen design.

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
2. **`pawn` shard** — re-scope `data_shard` to `pawn`; wire orchestrator/worker/edge. Give it
   `CREATE`'s deterministic spawn-id (the spawn-log `UNPACK` will reuse). Proves the rename.
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
- **Pre-pass operand encoding** — how a later operand names "the entity `UNPACK`/`CREATE` minted at
  step N" so the sweep can rewrite it (a position stand-in vs a step index). Settle when step 3 builds
  the pre-pass.
- **`PACK` fit-check** — per payload: tile (kind in range, no extra state), thing (kind/tile/data fit).
