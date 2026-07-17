# world-storage — hot & cold, one machinery

> **Status: intent / design.** The cold **baseline** is built + live (seed → edge relay → render).
> The shape below — `subtype`-keyed rows, the `state`/`state_log` overlay, region routing, and the
> mutation lifecycle — is the target the [`cold-rework`](../../work/cold-rework/README.md) work stream
> builds to. Layouts cite [`VARIABLES.md`](../../VARIABLES.md) + [`TABLES.md`](../../TABLES.md); builds
> on the live sim core ([`spacetime-again`](../spacetime-again/README.md)). Authored 2026-07-16,
> reworked 2026-07-17.

**What this is for.** Store every world object — pawns, terrain tiles, scattered things — and mutate
it **only** through the live event pipeline (edge → orchestrator → worker). Hot and cold are separate
shard *classes*, but they share one composition machinery: **`state_log` (truth) + `state` (the
client projection)**.

---

## The one rule: everything flows through the pipeline

**No component writes a shard directly — workers do, driven by events. The only direct write is
`init`/`seed`.** A pawn move, a tile change, a tree chopped: each is an event a worker composes into
absolute finals. No side door. Cold's baseline (`cold_tile`/`cold_thing`) is written *once* at seed;
after that, every change is a `state_log` row, exactly like a hot entity.

## Hot and cold share the spine, differ in the baseline

Both shard classes carry the **same** `state_log` / `state` pair (`TABLES.md § data_shard`):

- **`state_log`** — server-side truth, one row per `(entity, tic)`, written/modified by events. The
  server runs *entirely* on `state_log`.
- **`state`** — the **client projection**: the most-recent row the edge ships to core. Deliberately
  *throttled* — see below.

**Hot** (`data_shard` today; a `pawn` shard tomorrow) is *only* that pair — every entity is an
`entity_reference`-keyed row, id-addressed, because a mover isn't pinned to a cell.

**Cold** (`tile`, `thing`) is that pair **plus** a compressed, position-addressed **baseline**
(`cold_tile`/`cold_thing`). The baseline is the immutable seed; `state`/`state_log` are the mutation
overlay. The client renders **baseline ⊕ state** — a `state` row at a position overrides that cell.

## Cold row identity — `type_id` is the shard

A cold row's identity is `(macro_position, subtype_id, layer_id)` = the `u32 cold_row_reference`
(`macro:16 | subtype:12 | layer_id:4`, [`VARIABLES.md`](../../VARIABLES.md)). **`type_id` is not in
the row** — the `tile` module *is* `TYPE_BIOME_TILE`, `thing` *is* `TYPE_BIOME_THING`. Dropping that
`u4` (shard-implied) plus the old `reserved:8` freed the `u12 subtype_id` — so biome, which lived in
`subtype` all along, comes back **without growing the key past `u32`**.

Reconstruction sources `type_id` from the shard: `type_reference = shard.type_id | row.subtype_id`,
`layer_reference = shard.type_id | row.layer_id`; combined with each entry's `kind_reference` /
`tile_reference` they rebuild the full `definition_reference` + `position_reference`.

**A zone with N biomes is N rows** (same `macro`, different `subtype`). Biomes are large noise-blobs,
so most zones are one row; boundary zones split. Worldgen groups a zone's cells by
`biome_subtype_id` and seeds one row per group. (This restores the `zone_cold_objects` grouping that
an earlier simplification dropped — the defect that lost biome.)

### Dense vs sparse

- **Dense (tile):** `tiles: Vec<u16>` of `kind_reference`, index = `tile_reference`. Cells outside
  this row's biome are `0` (skipped on render). Position is the index; identity is `type_reference`
  (row) + `kind_reference` (cell).
- **Sparse (thing):** `things: Vec<u32>` of `kind_reference:16 | tile_reference:8 | data:8`, one per
  occupied cell — each entry carries its own `tile_reference`.
- **`data` is universal for things:** always `rotation:2 | count:6`, no per-type parse; game rules
  manipulate/display `count`. Tiles carry no `data`.

## `state_log` is truth; `state` is a throttled projection

The server simulates on `state_log`; `state` is a *view* it chooses to publish. **If `state` never
updates, clients freeze even while the sim churns** — intentional. It decouples client visibility from
sim progress: some actions never fan out.

> **Movement** writes every intermediate position to `state_log` (so an action that *reads* a position
> resolves correctly), but only **start + end** promote to `state` — clients interpolate the rest. The
> in-between is real server truth the client never needs.

## The mutation lifecycle — mint → act → fold

A cold cell is never rewritten in place. To change it:

1. **`UNPACK` (mint).** The cell has no id until it's touched. The first mutating event **mints an
   `entity_reference`** for the cell (server-minted, deterministic-from-event) and writes a `state_log`
   row in *the same cold shard* — the baseline stays untouched; the `state_log` row now *is* the truth
   for that position. (This is the old "UNPACK," but the entity lives in the cold shard's own overlay,
   not migrated to a separate hot shard.)
2. **Act.** Further events compose that `state_log` row like any hot entity — same worker, same
   `state_uid`, same `promote_state`. Client-visible changes promote to `state`; internal ones don't.
3. **GC = PACK (fold).** GC **queues** a fold for a settled cell: write its final back into the
   compressed baseline (`cold_tile`/`cold_thing`), **drop the `state` row** (the info now lives in the
   baseline the client already reads), and **tombstone — not drop —** the `state_log` rows (a takeover
   or a still-pending read may need them). A removal is just a `state_log`/`state` row with the removed
   marker — there is **no separate `cold_removed` table** anymore.

The client always sees **baseline ⊕ state** = current truth: while a cell is hot, its `state` row wins;
once GC folds it, the `state` row drops and the baseline carries the same value — the pixel never moves.

## Core owns the two-phase — the worker never waits

The mint is async, but the **worker never polls for it**. Core (already async, subscribed to `state`)
sequences a mint-then-use across tics; the worker mints + writes and completes in one pass.

1. **N=0** — core issues `UNPACK(position)` (often **pre-empted** — sent alongside an earlier action
   so the mint is ready by the time the op fires; safe by construction, a wrong guess is just a wasted
   unpack GC re-folds).
2. **~N=3** — the row appears in `state`; core learns the id by **watching the position** (one thing
   per cell, so position disambiguates — no alias table).
3. **~N=3+** — core issues the real op against the resolved id, gated on seeing the row.

Because `UNPACK` and the op are separate events across tics, no single program both mints and
references the result — no in-program pre-pass.

**Keys are server-minted — the client never picks them.** Every `entity_reference` (for `CREATE` and
`UNPACK` alike) is minted by the shard, unique and deterministic-from-event; the client only *learns*
it by watching `state`. (Today's npc `wolf_key` stopgap — one automated player, no ownership — is
replaced by server-minted `CREATE`.)

## Routing — position → cold shard, via `index`

A cold shard is **assigned one or more regions** (a region = 256 zones, position-contiguous — keeps
locality, shrinks the routing table 256×). `index.cold_shards` maps `(type_id, region_reference) →
shard endpoint`; the edge resolves `position_reference → macro_position → region_reference` then looks
up the shard. **The indirection ships now with a single default row** (`region * → shard 0`); a second
shard is one more row, no code change. (This is coord-purge's "rebuild the router fresh, macro-keyed"
— not the dead `zone_id`/`region_id` `region_shards`.)

## The shard classes

| module | class | key | payload |
|---|---|---|---|
| **`data_shard`** (→ `pawn`) | hot | `entity_reference` (u32) | `definition_reference:u32 \| position_reference:u32 \| data:u8` — `state_log`/`state` only |
| **`tile`** | cold | baseline `cold_row_reference` (`macro:16 \| subtype:12 \| layer_id:4`); overlay `entity_reference` | baseline `Vec<u16>` dense `kind_reference`; overlay = `state_log`/`state` |
| **`thing`** | cold | same | baseline `Vec<u32>` sparse `kind_reference:16 \| tile_reference:8 \| data:8`; overlay = `state_log`/`state` |

A cold `(entity, tic)` slot is the **same `u64 state_uid`** as hot — the composition machinery is one
shape, so cold rides everything the sim core already does.
