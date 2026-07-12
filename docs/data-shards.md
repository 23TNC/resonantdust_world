# Data shards — many modules, many shard classes

**Status (2026-07-11).** The plan for how the world grows from one data-shard module to
several **classes** of data shard, and how the edge routes to each. Reconciles the pre-0.2
[`object-shard.md`](object-shard.md) (written against `region_shard`/`object_shard`/
`cold_zones`/`hot_things`, most of which 0.2 merged away) with the current
`decl_tick_pipeline!` world ([`pipeline-generalization.md`](pipeline-generalization.md)).

## Where we are

Three data-shard **modules** exist, each its own DB, each a `decl_tick_pipeline!` payload:

| Module | DB (dev) | Payload | Carries | Reached by |
|---|---|---|---|---|
| `shard` | `resonantdust-dev-zone-0` | spatial (`kind, zone_id, location, rotation, offset, data0/1`) | mobile objects / movers / players (and, later, pawns) | index `region_shards`→`shards`, **default** `zone-0` |
| `cold_tiles` | `resonantdust-dev-cold-tiles-0` | tiles (`zone_id, tiles: Vec<u8>`) | a zone's **dense** ground grid — 256 `u8` tile-kinds | **hardcoded** `config.default_cold_tiles_db()` |
| `cold_things` | `resonantdust-dev-cold-things-0` | things (`zone_id, things: Vec<u64>`) | a zone's **sparse settled** things — `kind:16\|x:4\|y:4\|data:5\|layer:3\|variant:5\|reserved:27` each | **hardcoded** `config.default_cold_things_db()` |

The edge **triple**-subscribes each zone: object `state` (shard DB) + tiles (tiles DB) +
things (things DB), seeding each from worldgen if-absent
([`zones-to-screen.md`](zones-to-screen.md)). Splitting tiles (dense, ~256 B/zone) from
things (sparse — only occupied `(cell, layer)` slots) keeps a mostly-empty zone tiny.
**None of the three is truly class-routed** — objects fall back to the single default
shard, tiles/things use hardcoded names. The deliberate single-shard-dev shortcut
(`object-shard.md` Phase 3 took the same one: `resolve_object_or_default`).

### Zone aggregates are keyed by `zone_reference`

The tiles and cold-things entities (one per zone) are keyed by a **`zone_reference`**
(`shared/codec/src/refs.rs`): `u64 = server_id:16 | reserved:16 | zone_id:32`. It widens the
globally-unique `u32 zone_id` to the `u64` the pipeline's `actor_key`/`target_key` need, so a
zone can be an event actor/target; `server_id` records the minting server. Distinct from the
`entity_type`-discriminated `entity_reference` — a zone aggregate lives in its own DB, so
`zone_id` alone is unique. The coming **`hot_things`** shard is the counterpart: its things
each get a *minted* `entity_reference` (`u64` id) so they're first-class actors/targets with
richer state; a settled hot thing folds back into `cold_things` (pack/unpack).

### Thing encoding & the category direction

A thing is `kind:16 | x:4 | y:4 | data:5 | layer:3` (`shared/codec/src/packed.rs`):
`layer:3` stacks up to 8 things on a cell; `data:5` is kind-interpreted (rotation for
placeables, count for stacks). The `u16` **kind** space can later be **partitioned into
categories** — e.g. a *utilities* band (pipes/wires), a *secondary* band (surface details),
a *primary* band (one main placeable per tile), a *stacks* band (stackable items) — so a
cell can hold 8 *per category* rather than 8 total. That partition is a content convention
on top of this encoding, not an encoding change; add it when the content needs it.

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
per-class split. Promote `cold_tiles` (terrain) off `default_cold_tiles_db()` onto a class-routed
`shards` row as the first exercise of the new dimension.

## Cold objects on the object model (0.2.3)

The [object model](object-model.md) makes tiles and things the *same* thing — cold
**objects**, each an `object_reference`. A zone's cold data becomes **N `ColdRow`s
split by (type, subtype, layer)** (`object-model.md` §4): each row = a shared
`object_type_reference : u32` + a `Vec<object_kind_reference : u32>`. `biome-tile`
rows are the dense ground, `biome-thing` rows the sparse scatter — a row's `subtype`
*is* its biome. `Worldgen::zone_cold_objects` (`server/edge/src/worldgen.rs`) emits
exactly this.

**This is a change to what a module's COLD TABLE stores — NOT a new module.** A
pipeline module is generic (`decl_tick_pipeline!`) and carries **both a hot and a cold
table**:

- **hot** — the `state` table: minted, ticking entities (mobile objects, active
  cells), keyed by a minted `entity_reference`.
- **cold** — settled, packed, *static* objects (terrain, at-rest things), never
  ticks, keyed geographically. `unpack` promotes a cold object to a hot `state`
  entity; `pack` folds a settled hot entity back — **within the same module** (no
  cross-module transfer for settle/release; that's the whole point of one generic
  module holding both).

So the object-model work updates the **cold table** to hold `ColdRow`s, replacing
today's split cold payloads (`cold_tiles` `Vec<u8>` + `cold_things` `Vec<u64>`). The
dense `Vec<u8>` grid can't survive — a zone spans biomes, so a tile needs its
`subtype`, which the row carries. The engine, hot `state`, and pack/unpack are
unchanged; only the cold representation moves onto the object model.

### The cold table (generic, uniform across modules)

`object_kind_reference`s are type-agnostic `u32`s, so the cold table is **one fixed
shape** for every module (unlike the payload-parameterised hot `state`). Extend
`decl_tick_pipeline!` to emit it beside the hot tables:

```
cold {                                  // static; tick_gc ignores it
  #[primary_key] (region_zone_reference, type_reference)   // one ColdRow per (zone, type/subtype/layer)
  kinds: Vec<u32>                        // the object_kind_references
  version: u32                           // bumped on mutation → drives client re-send
}
```

Every module instance (tile shard, object shard, per realm) carries its own hot +
cold — the **shape** is generic and shared; the **data** stays sharded (consolidating
all shards into one table would be foolish, and isn't the point).

### Decisions (for review)

1. **Cold is a table in the generic module, not a module.** Extend
   `decl_tick_pipeline!` with the cold table; today's cold-only `cold_tiles`/
   `cold_things` modules converge into it as the module's cold side, beside the hot
   `state`. *(The established hot/cold-generic model — `world-on-pipeline.md`
   milestones, `object-shard.md` pack/unpack.)*
2. **Row = one `ColdRow` per `(region_zone, type_reference)`**, a Vec of its
   `object_kind_reference`s — matching §4's cold-table key, so a zone subscription's
   `WHERE` narrows by zone and the client decodes the handful of rows locally.
3. **`ColdRow` type** — a plain struct in `shared/codec` (canonical shape), with a
   `#[derive(SpacetimeType)]` **feature-gated** behind a `spacetime` codec feature the
   shard enables. One type end-to-end without dragging `spacetimedb` into the pure
   codec. *(Sub-decision: feature-gate vs module-local mirror vs flattening.)*
4. **Wire projection (later).** A dense `biome-tile` row's `x/y` is implicit, so the
   edge can drop it on the client hop (`u32`→~`u16`/tile). Deferred — correctness first.

### Migration (additive)

Add the cold table + its ColdRow representation alongside the current cold payloads;
edge seeds it from `zone_cold_objects`; the client learns to decode `ColdRow`s. Once a
zone renders from the new cold table in the browser, retire the old `Vec<u8>`/`Vec<u64>`
cold payloads. **Verify on the running stack** (`rd up` → `rd deploy` → browser) — this
changes what the client draws, so unit tests can't close it.

## Open

- **Cold-object encoding sub-decision** (above §3): feature-gate `SpacetimeType` on
  `codec::ColdRow`, a module-local mirror, or primitive parallel vecs.
- **`object-shard.md` is largely pre-0.2** — its transfer/presence *design* stands, but its
  modules/tables (`cold_zones`, `hot_things`, `region_shard`) don't exist; fold its still-
  valid parts (transfer saga, presence routing, witnessing) into the pack/unpack saga on the
  generalized engine (`pipeline-generalization.md` §Pack/unpack) when that lands.
- **`tile` shard class** (`SERVER_TYPE_TILE_DB`, hot per-cell cells) is reserved but
  unbuilt — it's Milestone 2 of the world-on-pipeline framing (hot cells over the cold
  terrain blob).
