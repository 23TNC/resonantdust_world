# world-storage — pawn, tile, and thing shards (cold ⇄ hot via pack/unpack)

> **Status: PLAN / intent.** Nothing built. Shapes cite [`VARIABLES.md`](../../VARIABLES.md) (the
> object model) + [`TABLES.md`](../../TABLES.md). Builds on the live sim core
> (`intent/spacetime-again/`). Authored 2026-07-16.

**What this is for.** Store every world object — pawns, terrain tiles, scattered things — and mutate
it **only** through the event pipeline that's already live (edge → orchestrator → worker). Three
concrete shard modules now; **no generic abstraction yet** (a `decl_shard!` macro can factor them
later, once two are proven divergent).

---

## The one rule: everything flows through the pipeline

**No component writes a shard directly — workers do, driven by events. The only direct write is
`init`.** A pawn move, a tile change, a tree chopped: all are events an orchestrator groups and a
worker composes into absolute finals. There is no side door.

## Cold is immutable-in-place; act in hot

Cold data (packed terrain) **cannot be modified where it sits.** To change it you **`UNPACK`** it into
a hot entity, act on it in hot (the normal `MOVE_TO` / `PLACE` / … pipeline), and later **`PACK`** it
back. `PACK` / `UNPACK` are new actions ([`ACTIONS.md`](../../ACTIONS.md)) that act on a
**`position_reference`** — the cell whose cold entry is promoted/demoted.

- **`UNPACK(position)`** — read the cold row's entry at that tile, mint a hot entity from it (payload
  from the cold entry: kind → definition, the position, data), and write a **tombstone** for that
  `tile_reference` in the cold row. The client now sees the hot entity, not the cold entry.
- **`PACK(position/entity)`** — read the hot entity, and **if its payload fits the cold payload**,
  write the entry back into the cold row, clear the tombstone, drop the hot entity. **If it doesn't
  fit, `PACK` fails** and the object stays hot (see below).

**Cold payloads may carry less than hot.** If a hot object grew past what cold can compress, it can't
be packed — it just **stays hot**. That's fine: such objects are rare enough that widening cold to fit
them isn't worth 2 bytes × 256 × every zone. The hot set is "everything that didn't fit or is still
active"; cold is the quiet majority.

## Tombstones stop re-transmission

A cold row is mostly-static and cheap to hold subscribed (a zone's whole layer in one row). When an
entry goes hot, its **tombstone** in the cold row says "this cell is hot — ignore the cold entry."
So the client holds the cold row *and* the hot entities *and* the tombstones, and never re-fetches a
cold row just because one cell changed. `cold_removed` (a `tile_reference:u8` per VARIABLES) is the
tombstone.

## GC queues pack work; workers run it

Nothing packs synchronously. **GC queues `PACK` events** for idle hot entities; a worker composes each
like any event. Because it's queued, a worker can **order the pack around other events on that entity,
and drop it entirely** if the entity is about to be unpacked/modified again anyway (no point packing
then immediately re-promoting). This keeps the hot set to *actively-changing* objects without a
stop-the-world pack.

---

## The three shards

| shard | regime | cold table? | payload |
|---|---|---|---|
| **`pawn`** | hot only | no | `definition_reference:u32 \| position_reference:u32 \| data:u8` (today's `data_shard`) |
| **`tile`** | cold + hot | yes | **cold:** `u16` `kind_reference` (dense, 256/zone, index = `tile_reference`). **hot:** the 3-ref pawn-style payload when unpacked |
| **`thing`** | cold + hot | yes | **cold:** `u32` = `kind_reference:16 \| tile_reference:8 \| data:8` (sparse). **hot:** 3-ref when unpacked |

- **`pawn`** has no cold — pawns are always hot. It is the current `data_shard`, re-scoped.
- **`tile`** cold is a dense `Vec<u16>` of exactly 256 `kind_reference`s (position implicit by index,
  no per-entry `tile_reference` or `data` — a tile is only *what kind*). Saving those 2 bytes/tile ×
  256 × every zone is worth carrying tiles as their own narrower payload.
- **`thing`** cold is a sparse `Vec<u32>` of `kind_pos_reference`s — a thing carries *where in the
  zone* and *its data* (facing/count), so it can't share the tile payload.

Each shard has its own `state_log` + `state` (hot composition, exactly like `data_shard` — **not
generalized yet**), and `tile`/`thing` add a `cold` table + `cold_removed` tombstones. All are
zone-keyed (`macro_position_reference`) for the edge/client subscription.

---

## Build order (one module at a time, the method that carried `spacetime-again`)

1. **`PACK` / `UNPACK` in `shared/codec`** — palette + signatures (operand = a `position_reference`),
   `Program` support. No behavior yet.
2. **`pawn` shard** — rename/re-scope `data_shard` to `pawn`; wire the orchestrator/worker/edge to it.
   Zero new mechanics (proves the rename is clean).
3. **`tile` shard** — cold table (256×`u16`) + tombstones + `UNPACK`/`PACK` in the worker
   (read-modify-write the cold Vec, mint/drop the hot entity). Worldgen seeds cold via `init` (the one
   direct write) or a seed event. Edge subscribes cold + hot; **revives the deferred terrain path**
   (`edge/index.rs`, `worldgen.rs`, `Event::ColdObjects`). Client renders the ground.
4. **`thing` shard** — same, sparse `u32` payload; client renders scatter over the ground.
5. **GC-queues-`PACK`** — the master (or a scheduled reducer) queues pack events for idle hot
   entities; the worker's pack handles the fit-check + drop-if-active.

---

## Open — small, mostly settled

- **Cold row key = `u32`** `macro_position:16 | layer_reference:8 | server_reference:8` (realm implied),
  so a cold `(row, tic)` slot is the *same* `u64` `state_uid` as hot (`reserved:16 | key:32 | tic:16`).
  Subtype lives in the per-entry `kind_reference:16`, not the row — the row carries `layer_reference:8`
  (`type_id:4 | layer_id:4`), not `type_reference:16`. Requires revising `cold_row_reference` `u64 →
  u32` in VARIABLES. (Confirm; this is the only shape still to ratify.)
- **Unpacked entity id.** A hot tile/thing needs an `entity_reference`; deriving it from the cell's
  `position` (so `PACK`/`UNPACK` round-trips deterministically and replays idempotently) is the
  natural scheme — settle the exact bit map when the `tile` worker path is built.
- **`PACK` fit-check** is per-payload: does this hot payload compress into this cold entry? Defined per
  shard (tile: is the kind in range + no extra state; thing: kind/tile/data all fit).
