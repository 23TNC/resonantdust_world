# Data shards — many modules, many shard classes

**Status (2026-07-11).** The plan for how the world grows from one data-shard module to
several **classes** of data shard, and how the edge routes to each. Reconciles the pre-0.2
[`object-shard.md`](object-shard.md) (written against `region_shard`/`object_shard`/
`cold_zones`/`hot_things`, most of which 0.2 merged away) with the current
`decl_tick_pipeline!` world ([`pipeline-generalization.md`](pipeline-generalization.md)).

## Where we are

Two data-shard **modules** exist, each its own DB, each a `decl_tick_pipeline!` payload:

| Module | DB (dev) | Payload | Carries | Reached by |
|---|---|---|---|---|
| `shard` | `resonantdust-dev-zone-0` | spatial (`kind, zone_id, location, rotation, offset, data0/1`) | mobile objects / movers / players (and, later, pawns) | index `region_shards`→`shards`, **default** `zone-0` |
| `zone` | `resonantdust-dev-zone-terrain-0` | terrain (`zone_id, tiles: Vec<u16>, things: Vec<u32>`) | static per-zone terrain (worldgen baseline) | **hardcoded** `config.default_terrain_db()` |

The edge dual-subscribes each zone: object `state` from the shard DB + terrain `state`
from the terrain DB ([`zones-to-screen.md`](zones-to-screen.md)). **Neither class is truly
class-routed** — objects fall back to the single default shard, terrain uses a hardcoded
name. This is the deliberate single-shard-dev shortcut (`object-shard.md` Phase 3 took the
same one: `resolve_object_or_default`).

## The class vocabulary already exists

[`shared/codec/src/refs.rs`](../shared/codec/src/refs.rs) reserves a **database-server type
range** in `server_reference` — `SERVER_TYPE_OBJECT_DB=32`, `ZONE_DB=33`, `TILE_DB=34`
(`server_type >= SERVER_TYPE_DB_BASE` = "has a `DbConnection`"). And `entity_type →
data_type` maps entity classes onto pipelines (`DATA_TYPE_OBJECT`, `DATA_TYPE_ZONE`, …). So
the *identity/typing* layer is ready for multiple DB classes; the **routing layer is not**.

## The gap — routing is class-agnostic

The `index` module routes only:

```
region_shards:  region_id → shard_id
shards:         shard_id  → { url, db_name }
```

**One shard per region.** There is no way to say "region R's *object* shard is X and its
*terrain* shard is Y." Every added class today needs another hardcoded `default_*_db()` on
the edge — which is why a new shard is invisible to `rd index show` / `content/servers/*`.

## The plan — add a class dimension to routing

Give the index a **shard class**, so each data-shard module is a first-class routed shard:

```
shards:        (shard_id) → { class: u8, url, db_name }     // class = SERVER_TYPE_*_DB
region_shards: (region_id, class) → shard_id                // per-class routing
```

- Each module **registers itself with its class** via `set_shard` (using the
  `SERVER_TYPE_*_DB` value), so the directory lists every shard and what it holds.
- The edge resolves **`(region, class) → endpoint`** per class it needs, and
  multi-subscribes each zone across those classes (generalizing today's hardcoded
  object+terrain pair into a loop over the classes a zone requires).
- `content/servers/<env>` gains a class column on `shard` rows
  (`shard <id> <class> <url> <db>`); `bin/rd index` seeds them.

### Two routing regimes coexist (from `object-shard.md`)

- **Static, region-partitioned** (terrain/zone, and object-by-region for now): `zone_id →
  region_id → (class) → shard_id`. Deterministic; resolved the instant a player anchors.
- **Dynamic, presence-routed** (object/pawn shards at scale): a shard's home is chosen at
  placement time and discovered by reading each object shard's `presence` count — *not* a
  static row. Deferred; single default object shard until it's needed
  (`object-shard.md` §Routing, still the design of record for this regime).

The class dimension serves the static regime directly; the dynamic regime layers on top
(presence is per-`(region, object-class-shard)`).

## Adding a new shard class (the recipe)

1. New `decl_tick_pipeline!` module under `spacetime/server/modules/<class>` with its
   payload; reserve/confirm its `SERVER_TYPE_*_DB` + `ENTITY_TYPE_*` + `DATA_TYPE_*`.
2. `bin/rd` deploy: map the module → its own DB family (as `zone → zone-terrain` already
   does in `redeploy.sh`), and register it in the index with its class.
3. Edge: add its `connect_<class>` (the `connector!` macro) and include the class in the
   per-zone resolve+subscribe loop; relay its rows as a new `RowData` variant → client
   events → renderer.
4. Client + pixijs: decode the row variant and render/consume it.

Steps 3–4 are exactly what [`zones-to-screen.md`](zones-to-screen.md) did for `zone`; the
class-routing work (step 2 + the edge resolve loop) is what promotes it from a hardcoded
default to a real directory-listed shard.

## Migration — no flag day

Single-shard dev keeps working throughout: with no `region_shards` rows, every class
resolves to its **default** shard (today's behaviour). The class dimension is additive —
add the column + `connect_<class>` + resolve loop; nothing breaks until you actually seed a
per-class split. Promote `zone` (terrain) off `default_terrain_db()` onto a class-routed
`shards` row as the first exercise of the new dimension.

## Open

- **`object-shard.md` is largely pre-0.2** — its transfer/presence *design* stands, but its
  modules/tables (`cold_zones`, `hot_things`, `region_shard`) don't exist; fold its still-
  valid parts (transfer saga, presence routing, witnessing) into the pack/unpack saga on the
  generalized engine (`pipeline-generalization.md` §Pack/unpack) when that lands.
- **`tile` shard class** (`SERVER_TYPE_TILE_DB`, hot per-cell cells) is reserved but
  unbuilt — it's Milestone 2 of the world-on-pipeline framing (hot cells over the cold
  terrain blob).
