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
**objects**, each an `object_reference` (type / subtype / kind / subkind / variant /
x / y / data). A zone's cold data becomes **N `ColdRow`s split by (type, subtype,
layer)** (`object-model.md` §4): each row = a shared `object_type_reference : u32` +
a `Vec<object_kind_reference : u32>`. `biome-tile` rows are the dense ground (one per
cell), `biome-thing` rows the sparse scatter — and a row's `subtype` *is* its biome.
`Worldgen::zone_cold_objects` (`server/edge/src/worldgen.rs`) already emits exactly
this. It replaces the two split payloads:

| today | 0.2.3 |
|---|---|
| `cold_tiles` — `tiles: Vec<u8>` (kind only, no biome) | `biome-tile` `ColdRow`s (kind + biome via `subtype`) |
| `cold_things` — `things: Vec<u64>` (old `pack_thing`) | `biome-thing` `ColdRow`s (new `object_kind_reference`) |

The dense `Vec<u8>` grid can't survive: a zone spans biomes, so a tile needs its
`subtype`, which the row carries for free.

### Decisions (for review)

1. **Unify `cold_tiles` + `cold_things` → one `cold` module.** Both are geographic
   cold `ColdRow`s now, and the doc's *tile shard* holds both (`object-model.md` §5).
   Merging turns the edge's triple-subscribe into a double (object `state` + `cold`).
   An empty zone still costs only its `biome-tile` rows (ground you need anyway), so
   the original split's "don't pay for an empty thing array" rationale is gone.
   *(Recommended.)*
2. **One entity per zone; payload = the zone's rows.** Keep the current
   one-entity-per-zone shape (keyed by `zone_reference`), payload becomes the zone's
   `ColdRow`s. Subscriptions stay `WHERE zone_id`, stream the whole zone, and the
   client decodes locally — matching §4 ("subscriptions never filter; queries do").
   *(Recommended over one-entity-per-`(zone, type_reference)` row — the per-zone
   seeding doesn't need the finer key.)*
3. **Encoding + where `ColdRow` lives.** Payload `rows: Vec<ColdRow>` with `ColdRow {
   type_reference: u32, kinds: Vec<u32> }`. To share one type across edge (produces),
   shard (stores), client (decodes) without dragging `spacetimedb` into the pure
   `codec` crate: define `ColdRow` in `shared/codec` as the canonical shape, and
   **feature-gate a `#[derive(SpacetimeType)]`** behind a `spacetime` codec feature the
   shard enables. *Sub-decision:* feature-gate vs a module-local mirror vs flattening
   to primitive parallel vecs (`row_types: Vec<u32>`, `row_kinds: Vec<Vec<u32>>`).
   *Lean: feature-gate, one type end-to-end.*
4. **Wire projection (later).** A dense `biome-tile` row's `x/y` is implicit (one per
   cell), so the **edge** can project it out on the client hop (`u32`→~`u16`/tile), per
   §4's storage-vs-wire split. Deferred — correctness first, bandwidth second.

### Migration (additive, no flag day)

New `cold` module *alongside* `cold_tiles`/`cold_things`. Edge seeds it from
`zone_cold_objects` and subscribes it in parallel; the client learns to decode
`ColdRow`s. Once a zone renders from `cold` in the browser, drop the two legacy
modules + their subscribes. **Verify on the running stack** (`rd up` → `rd deploy` →
browser) — this changes what the client draws, so unit tests can't close it.

## Open

- **Cold-object encoding sub-decision** (above §3): feature-gate `SpacetimeType` on
  `codec::ColdRow`, a module-local mirror, or primitive parallel vecs. Blocks the
  `cold` module + edge/client wire.
- **`object-shard.md` is largely pre-0.2** — its transfer/presence *design* stands, but its
  modules/tables (`cold_zones`, `hot_things`, `region_shard`) don't exist; fold its still-
  valid parts (transfer saga, presence routing, witnessing) into the pack/unpack saga on the
  generalized engine (`pipeline-generalization.md` §Pack/unpack) when that lands.
- **`tile` shard class** (`SERVER_TYPE_TILE_DB`, hot per-cell cells) is reserved but
  unbuilt — it's Milestone 2 of the world-on-pipeline framing (hot cells over the cold
  terrain blob).
