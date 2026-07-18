# world-storage — hot & cold, one machinery

> **Status: intent / design.** The cold `subtype`-keyed baseline, a per-cell overlay, region routing,
> the `tic`-ordered composite, and the event-driven per-cell `SET` are **built + live** (on the
> pre-generalization `cold_tile`/`cold_thing` + old `state`/`state_log`). The **2026-07-18 direction
> shift below is design-only, not yet built**: fold every shard into the generic **`*_tables!`** macros
> — a primary `entity_state`/`entity_state_log` pair (`entity_tables!` / `dense_entity_tables!` /
> `sparse_entity_tables!`) + a cold override `overlay`/`overlay_log` (`overlay_tables!`) — drive every
> write through **events** (retiring the direct `seed`/`set_*`/`fold` reducers), and make **`PROMOTE`** a
> smart + atomic prefix. Closes the seed/acquire race by construction. Tracked in
> [`cold-rework`](../../work/cold-rework/README.md). Layouts [`VARIABLES.md`](../../VARIABLES.md) +
> [`TABLES.md`](../../TABLES.md); actions [`ACTIONS.md`](../../ACTIONS.md); builds on the live sim core
> ([`spacetime-again`](../spacetime-again/README.md)). Authored 2026-07-16, reworked 2026-07-18.

**What this is for.** Store every world object — pawns, terrain tiles, scattered things — and mutate
it **only** through the live event pipeline (edge → orchestrator → worker). Hot and cold are separate
shard *classes*, but they share **one convention**: a server-side **`*_log`** (truth + history the
worker composes) and a client-visible **`*`** (the throttled projection) — run for `event`, `state`,
**and now `cold`**.

---

## The one rule: everything flows through the pipeline

**No component writes a shard directly — workers do, driven by events.** A pawn move, a tile change, a
tree chopped, *and even the initial seed* (`init_zone`): each is an event a worker composes into
absolute finals, then a `PROMOTE` makes visible. No side door — not even `init`. A baseline row lands
in `entity_state_log` (whole-row) and a per-cell change in `overlay_log`, both exactly like a hot
entity's `entity_state_log`.

## One convention everywhere: a server-side `*_log`, a client-visible `*`

The whole system is built on **one pair-shape**: a server-side **`*_log`** (truth + history, what
workers compose) and a client-visible **`*`** (the throttled projection the edge ships to core). We
already run it twice — `event_log`/`event` and `state_log`/`state`. **Cold now uses the *same* shape**,
so there is no bespoke cold machinery to drift:

- **`*_log`** — server truth, one row per `(subject, tic)`, written/modified by events, composed by the
  worker. The server runs *entirely* on the log.
- **`*`** — the client projection: the most-recent promoted row the edge ships. Deliberately *throttled*
  — a value reaches `*` only when an event `PROMOTE`s it (§throttling).

**Hot** (`data_shard` today; a `pawn` shard tomorrow) carries exactly one pair — `state_log`/`state`,
each entity an `entity_reference`-keyed row, id-addressed (a mover isn't pinned to a cell).

**"Cold" isn't one thing — it's two row *forms*, `dense` and `sparse`** (the word "cold" survives only
in `cold_row_reference`). A shard carries **two** pairs, because a cell has two very different kinds of
change, and conflating them fans out 256 tiles for a one-cell edit:

| role | pair | macro | addressed by | holds | changes |
|---|---|---|---|---|---|
| **baseline** | `entity_state`/`entity_state_log` | `dense_entity_tables!` or `sparse_entity_tables!` | `cold_row_reference` (the biome-row) | the compressed row — `Vec<DenseItem>` (full `ZONE_DIM²`) or `Vec<SparseItem>` (occupied only) | **whole-row**, infrequent — seed, or a GC fold |
| **overlay** | `overlay`/`overlay_log` | **`overlay_tables!`** (sparse) | `cold_row_reference`, cell by `tile_reference` | the **overridden** cells (a sparse shadow of the baseline) | **few cells**, frequent — a placed wall, a chopped tree |

The **overlay is always sparse** — an override touches a handful of cells, so it's a sparse shadow of
the baseline; the hot, `entity_reference`-addressed pair (`entity_tables!`) is for **hot** shards only.
All are **generic macros over an optional payload `<T>`** ([`TABLES.md`](../../TABLES.md)), composed per
shard: `data_shard = entity_tables!{data:u8}`, `tile = dense_entity_tables!() + overlay_tables!()`,
`thing = sparse_entity_tables!{data:u8} + overlay_tables!{data:u8}`. Written once, they let actions + the
worker machinery be written once too. The client renders **`entity_state` ⊕ `overlay`** — an overlay
cell shadows its baseline cell (overlay present ⇒ override wins; no per-cell `tic` compare needed).

**Why two pairs, not one.** A player placing a wall every 3 tics must not re-broadcast the zone's whole
256-tile `entity_state` row to every subscriber every 3 tics — that's catastrophic fan-out. Instead the
edit is one small `overlay` cell, streamed cheaply; the big `entity_state` row only changes when GC
*folds* accumulated overrides back into it (a `PACK`), infrequently. Placing a water tile needs **no
hot shard** — it's an `overlay` cell written straight on the cold shard. Giving the baseline its own
`entity_state_log` (the same **history** the overlay has) is what lets the *same worker machinery*
compose both.

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

## Everything through events — no direct writes

**The only writer of a shard is a worker, driven by an event** (§the one rule). The old *direct*
reducers — `seed`, `set_tile`/`set_thing`, `fold` — are **removed**. They were the seed/acquire-race
source (a baseline row written directly, its `tic` never bumping, so a relay the edge missed never
re-fired → black ground under rendered scatter). In the event model a row becomes visible only on
`PROMOTE`, which the client acquires like any `*` update — no special path, no race.

**Write a baseline row (seed / regen).** Queue an event for the zone (`promote init_zone macro`). A
worker builds the **whole row** in scratch, writes it to `entity_state_log`, and — because the
`promote` prefix set the bit — the write reducer projects it to `entity_state` in the same call. Until
the worker finishes and promotes, the new row **isn't visible** — so a zone that takes several tics to
write never shows a half-built baseline.

**A per-cell override (place a wall, water a tile).** Queue an event targeting the cell's biome-row
(`cold_row_reference`), addressed by `tile_reference`. The worker composes the change into
`overlay_log`; the `promote` bit projects it to `overlay`. The client renders `entity_state ⊕ overlay`.
Cheap, frequent — one small cell, never the whole row.

**GC = PACK (fold overrides into the baseline).** When a zone's overrides have settled, GC queues a
`promote pack …` event: the worker folds each settled `overlay` cell into the row's `entity_state_log`
(the new baseline) *and* clears the `overlay_log`, then the smart `promote` projects **both** —
`entity_state` and `overlay` — in **one reducer transaction**.

### Atomic promote replaces promote-ordering

The client sees the fold's baseline-update + overlay-clear as **one atomic update** (a single commit),
so there's no window where the overrides are gone but the baseline hasn't caught up — **no flash of the
pre-edit terrain, and no `PROMOTE`-ordering to get wrong**. (Smart promote copies only what changed:
`entity_state`/`overlay` where `visible.tic != log.tic`.) This is simpler than the earlier ordered
`PROMOTE_COLD`-then-`PROMOTE_STATE` scheme it supersedes — atomicity does the work ordering used to.

## Keys are server-derived — the client never picks them

A baseline cell is addressed by its **position** (`cold_row_reference` + `tile_reference`), a pure
function of where it is — so there's no mint to wait on and no id-watch for cold. A *general* `CREATE`
(a pawn, whose identity is **not** its position) still needs a server-minted, deterministic-from-event
id learned by watching `entity_state`; that remains the hot-shard concern. (Today's npc `wolf_key`
stopgap is the placeholder until `CREATE`'s minted-id claim lands.)

## Routing — position → cold shard, via `index`

A cold shard is **assigned one or more regions** (a region = 256 zones, position-contiguous — keeps
locality, shrinks the routing table 256×). `index.cold_shards` maps `(type_id, region_reference) →
shard endpoint`; the edge resolves `position_reference → macro_position → region_reference` then looks
up the shard. **The indirection ships now with a single default row** (`region * → shard 0`); a second
shard is one more row, no code change. (This is coord-purge's "rebuild the router fresh, macro-keyed"
— not the dead `zone_id`/`region_id` `region_shards`.)

## The shard classes

| module | class | macros → pairs | keys | payloads |
|---|---|---|---|---|
| **`data_shard`** (→ `pawn`) | hot | `entity_tables!{data:u8}` → `entity_state`/`entity_state_log` | `entity_reference` (u32) | `definition` + `micro_position` + `u8 data` |
| **`tile`** | cold | `dense_entity_tables!()` + `overlay_tables!()` → `entity_state` + `overlay` | baseline `cold_row_reference` (`macro:16 \| subtype:12 \| layer_id:4`); overlay same | baseline `Vec<DenseItem>` (`kind` only); overlay `Vec<SparseItem>` |
| **`thing`** | cold | `sparse_entity_tables!{data:u8}` + `overlay_tables!{data:u8}` → same | same | baseline `Vec<SparseItem>` (`tile \| kind \| u8 data`); overlay same |

Every pair rides the **same composition slot shape**: an `entity_state_log` slot is a `u64` uid
(hot: `state_uid` = `entity, tic`; cold: `cold_uid` = `cold_row_reference, tic`) — same layout,
different subject. So the worker's compose/block/write/`PROMOTE` machinery serves all pairs; only the
payload differs (a single row for the hot entity, a `Vec` of items for a cold row).

**One structural note (open at build time).** The composition *columns* (`uid`, subject, `tic`,
`worker_reference`, `observer_reference`, `dirty`, `status`) are identical across all pairs; only the
payload column differs (a single payload vs a `Vec<DenseItem>`/`Vec<SparseItem>`). Whether the
`*_tables!` macros share one payload-generic core or each hand-roll their payload is a build decision;
the *shape* is settled here.
