# The object model — implementation status & runbook

> **Status: IMPLEMENTED & VERIFIED LIVE (2026-07-13), branch `0.2.3`.** The whole
> object-model redesign is built end to end (spacetime → edge → client → pixijs),
> deployed on the dev stack, and confirmed in the browser + the DB. This doc is the
> **implementation record + runbook**; the *design* lives in
> [`object-model.md`](../design/object-model.md) (identity/reference bit-layouts, shard classes,
> events) and [`data-shards.md`](../../../../data-shards.md) (the cold table on the generic module).

---

## 1. What it is, in one breath

Every game thing — a ground tile, a tree, a pawn — is an **object** with a packed
`object_reference` identity (type / subtype / kind / subkind / variant + placement).
A zone's terrain is stored **cold** (settled, packed) as `ColdRow`s split by
`(type, subtype = biome, layer)`, on a **generic `cold` table** that every
`decl_tick_pipeline!` module carries beside its hot `state` table. `unpack`/`pack`
reducers promote a cold object into the ticking pipeline and fold it back — so cold
objects are first-class in the tick pipeline, not inert terrain.

## 2. The data flow (what actually happens)

```
worldgen (edge, DSL)                     shard module (SpacetimeDB)              pixijs client
────────────────────                     ──────────────────────────              ─────────────
Worldgen::zone_cold_objects(zone)  ──▶   seed_cold_row  ──▶  cold table   ──sub──▶ RowData::ColdObjects
  biome per cell → subtype                (insert-if-absent)  (per (zone,        │  → Event::ColdObjects
  tile/thing kind → kind_id                                    type_reference))  │  → zoneColdPrims (wasm)
  → object_kind_reference (u32)                                                  │  → WorldBridge paints
  grouped by object_type_reference                            hot `state` table  │    biome-tile = ground
                                                              (minted, ticking)  │    biome-thing = sprite
right-click a cold thing  ──Interact──▶  edge handle_interact ──▶ unpack reducer ─┘
                                          (find object at cell,     cold row shrinks
                                           decode, call unpack)     + minted state entity
```

- **Cold** = settled/packed/static; never ticks (dirty-gating at its limit). Lives in
  the `cold` table.
- **Hot** = minted/ticking; lives in `state`. `unpack` (cold→hot) mints a `state`
  entity; `pack` (hot→cold) folds it back. Both **in-module** (cold sits on the same
  `shard` module as the object-shaped hot `state`, so no cross-module transfer).

## 3. The build order (commit arc `52f2280 → 2ea33b2`)

| Commit | Layer | What landed |
|---|---|---|
| `8291d04` | design | `docs/object-model.md` — the pinned contract |
| `33b7668` | codec | `shared/codec/src/object.rs` — packed reference encode/decode (11 tests) |
| `46eab10` | taxonomy | `type_id` code constants + biome-carried `@subtype` ids (no central registry) |
| `11e2a2e`→`e72f52c` | worldgen | `Worldgen::zone_cold_objects` — tiles + things as unified `ColdRow`s |
| `52f2280`→`c7b4207` | design | cold-object shard schema; corrected to *cold is a table, not a module* |
| `21aedbf` | spacetime | generic **`cold` table** + `seed_cold_row` on `decl_tick_pipeline!` |
| `c72b45b` | edge | seed + relay + subscribe cold rows; `RowData::ColdObjects` |
| `a2d9ce0` | client+wasm | `Event::ColdObjects`; `zoneColdPrims` + type helpers |
| `27d4181` | pixijs | `WorldBridge` renders zones from cold rows (one painter, dispatch by type) |
| `f111fc5` | re-home | cold table moved from `cold_tiles` onto `shard` (beside hot `state`) |
| `daeba4d` | spacetime | `unpack`/`pack` reducers — the cold↔hot pipeline bridge |
| `744cab9` | cleanup | retired the legacy `cold_tiles`/`cold_things` wire paths |
| `4fcac7c` | gameplay | `Interact` command → edge `handle_interact` → right-click unpacks |
| `9c3f6fb` | textures | tree onto `type/subtype/kind/subkind`; recursive manifest scan |
| `2ea33b2` | debug | `?user`/`?x`/`?y`/`?ambient` URL params |

## 4. Where everything lives (the map)

### shared/codec — the identity currency
- **`object.rs`** — pure packing. `pack_object_reference` (u64 = type-half u32 +
  kind-half u32); `pack_type_reference` (type_id:4 · subtype_id:12 · layer:4 ·
  reserved:12); `pack_kind_reference` (kind:10 · subkind:4 · variant:4 · x:4 · y:4 ·
  data:6); spatial u8 grids; `pack_region_zone` (u16); `pack_cold_reference` (u32);
  `data_default`/`data_stack` (type-decoded payload); **type constants**
  `TYPE_BIOME_TILE=1, TYPE_BIOME_THING=2, TYPE_PAWN=3, TYPE_PLAYER=4, TYPE_EVENT=5,
  TYPE_SERVER=6` (append-only; supersede refs.rs's entity/data types).
- `refs.rs` / `packed.rs` — the **prior** reference layers, superseded by `object.rs`
  but still consumed by the live pipeline (migrate off, then remove).

### content — the names→ids
- **`content/registry.rd`** does NOT exist (rejected). `type_id` = code constants;
  **biome `subtype_id` = each biome's `@subtype>` hook** in `content/biome/biomes.rd`
  (ocean=1…plains=7; 0 = default). `kind_id` = tile/thing bucket appearance order (the
  existing `tile_def_id`/`thing_object_id`). Parser learned `subtype` as a code hook;
  loader gained `biome_subtype_id()` (`shared/dsl`).

### worldgen — composition
- **`server/edge/src/worldgen.rs`** — `zone_cold_objects(zone) -> Vec<ColdRow>`. Each
  cell emits a `biome-tile` object (dense) + a `biome-thing` object where scattered,
  filed under the `object_type_reference` for `(type, subtype=biome @subtype, layer 0)`.
  `ColdRow { type_reference: u32, kinds: Vec<u32> }`. (The legacy `zone_terrain` Vec<u8>/
  Vec<u64> path is retired downstream but the fn remains as the generator's guts.)

### spacetime — the generic module
- **`server/spacetime/server/pipeline/src/lib.rs`** (`decl_tick_pipeline!`) emits, for
  **every** module: the hot tables (`event_log`/`state_log`/`state`/…) **plus** a
  uniform `cold` table `{ pk cold_key=(zone_id<<32|type_reference), idx zone_id,
  type_reference, kinds: Vec<u32>, version }`, and reducers `seed_cold_row`,
  **`unpack`** (`cold_zone, cold_type_reference, cold_x, cold_y, hot_entity_type,
  <payload…>`), **`pack`** (`entity_key, zone, type_reference, object_kind_reference`).
  Cold objects live on the **`shard`** module (its `state` payload is object-shaped).

### edge — seed / stream / interact
- **`server/edge/src/ws.rs`** — on zone subscribe: `wire_cold_relay` + a
  `SELECT * FROM cold WHERE zone_id` subscription + `seed_cold_if_empty` (writes
  `zone_cold_objects` via `seed_cold_row`), all on the **`shard`** connection.
  `handle_interact` — finds the `biome-thing` cold object at the clicked cell in the
  cached cold rows, decodes its kind, calls `unpack`.
- **`server/edge/src/protocol.rs`** — `RowData::ColdObjects`, `ClientMsg::Interact`.
- **`server/edge/src/tex_manifest.rs`** — `scan_masters` is now **recursive** (any tree
  depth), so both legacy 2-level and new 4-level texture layouts scan into stems.

### client core + wasm — decode
- **`client/core/src/{protocol,api,engine,web}.rs`** — `RowData/Event::ColdObjects`
  dispatched on both the native + wasm row loops; `Command/ClientMsg::Interact`.
- **`shared/wasm/src/lib.rs`** — marshals `ColdObjects` to JS; `Content::zoneColdPrims`
  (decodes each `object_kind_reference` → stride-7 `[tileX,tileY,tint,geoColor,kindId,
  data,variant]`, dispatching `visual_for_def` vs `visual_for_object` by type);
  `objectTypeId`/`typeBiomeTile`/`typeBiomeThing`; `WorldClient::interact`.

### pixijs — render + input
- **`client/pixijs/src/game/world/WorldBridge.ts`** — `onColdObjects` paints per
  `(zone, type_reference)`: `biome-tile` → 64×64 ground, `biome-thing` → bottom-centred
  sprite. (Legacy tiles/things painters removed.)
- **`client/pixijs/src/scenes/world/WorldScene.ts`** — right-click → `interact`;
  ambient boost + initial camera tile from URL params.
- **`client/pixijs/src/debug/urlParams.ts`** — `?user`/`?x`/`?y`/`?ambient`.

## 5. Verification record (what was actually confirmed)

| Claim | How verified |
|---|---|
| codec packs/roundtrips | 11 `object::` unit tests (rust:slim), full crate green |
| worldgen composes right rows | 9 edge tests; biome-tile=256/cell, biome-thing=legacy count |
| zones render from `cold` | browser: biomes + trees + shrubs draw; client legacy path *removed* so this is proof |
| cold rows are correct | `SELECT … FROM cold` → zone 0 = 5 biome-tile + 5 biome-thing rows, type_references decode to the right (type, biome) |
| cold re-homed to `shard` | `shard.cold` populated (edge log `rows=10`), `cold_tiles.cold` unread; renders |
| `unpack` promotes cold→hot | CLI `unpack` → cold row deleted, `state` gains a minted entity (type 4, kind, location) |
| right-click unpacks | edge log `interact → unpacked zone=0 x=9 y=3 kind=1`; `state` gained a `kind=1` entity |
| texture taxonomy renders | manifest lists `biome-thing/default/conifer/default/s`; full-colour conifers draw |
| debug params work | `/?user=Developer&x=9&y=3&ambient=1.5` auto-logs-in, frames (9,3), brightens the scene |

## 6. Debug URL params (client/pixijs/src/debug/urlParams.ts)

`http://localhost:5173/?user=Developer&x=9&y=3&ambient=1.5`

- `?user=<name>` — auto-login on the default server (skips the login form).
- `?x=/?y=<tile>` — centre the viewport on that tile at start.
- `?ambient=<f>` — lift the ambient floor to a white boost (e.g. `1.5`) so biomes/things
  are clearly visible; lighting is active (sun + ambient, default intensity `0.5`).

## 7. What remains (honest follow-ups)

- **Worker-resolved cold events** — `interact→unpack` is *edge-triggered*. The larger
  step is the worker resolving an event that *targets* a cold object (decode
  `object_kind_reference`→payload, run `unpack`), so cold interaction is event-driven
  through `event_log → state_log → state`.
- **Texture leaf reshape** — the folders moved to `type/subtype/kind/subkind`, but the
  *leaf* is still `<id>.<dir>.<layer>/<variant>/<map>`; the design (§8) wants
  `<variant>/<dir>.<part>.<map>`. Plus `bin/art` + `bin/lib/texpath.py` writers, and
  **biome-specific art** (real subtypes vs the `default` placeholder). `linked`/
  `pawns.human`/`berry` stems + on-disk moves also pending.
- **Undeploy** the now-unused `cold_tiles`/`cold_things` modules.
- **Reconcile** `object.rs` type ids with `refs.rs` `entity_type`/`server_reference`
  (the object-model type_id ≠ the minted entity_type; `unpack` takes it as an arg today).
- **Realm/server model** — `server_reference = realm · server_id`, per-realm shard
  classes, `region_zone` keying: designed (`object-model.md` §5) but the running stack
  is still single-shard-dev (index routing not wired).

## 8. Runbook

### Build/test in-container (no host cargo)
```bash
SC=<scratch>/cargo   # persist the cargo registry to skip re-downloads
# codec / dsl (rust:slim)
docker run --rm -v "$PWD/shared":/s -v "$SC":/cargo -e CARGO_HOME=/cargo -w /s \
  rust:slim bash -lc 'cd /s && cargo test -p resonantdust-codec -p resonantdust-dsl'   # (mount repo for the workspace)
# a spacetime module → wasm  (clockworklabs/spacetime:v2.3.0)
docker run --rm -v "$PWD/server/spacetime/server":/workspace/server \
  -v "$PWD/shared":/workspace/shared -w /workspace/server/modules/shard \
  clockworklabs/spacetime:v2.3.0 build
# module → edge Rust bindings (RAW / do NOT rustfmt — repo commits raw generate output)
docker run --rm -v "$PWD/server/spacetime/server":/workspace/server \
  -v "$PWD/shared":/workspace/shared -v "$PWD/server/edge":/workspace/game-server \
  -w /workspace/server --entrypoint /bin/bash \
  clockworklabs/spacetime:v2.3.0 generate-bindings.sh shard
# edge (rust:slim; needs OpenSSL + the git-hash env for spacetimedb-lib)
docker run --rm -v "$PWD/server/edge":/workspace -v "$PWD/shared":/shared \
  -e SPACETIMEDB_NIX_BUILD_GIT_COMMIT=aa73d1c35b4b346b98eeba10a3d756b4ae72162f \
  -w /workspace rust:slim bash -lc \
  'apt-get update -qq && apt-get install -y -qq pkg-config libssl-dev && cargo check --bin edge'
# wasm bundle (regenerates shared/pkg, gitignored)  → then pixijs `npm run typecheck`
```

### Deploy to the dev stack + verify
```bash
bin/rd deploy module shard      # rebuild+publish the shard module (resets its DB; re-seeds on subscribe)
bin/rd deploy edge              # rebuild+run the edge
bin/rd build shared             # regenerate the wasm bundle (vite dev-serves shared/pkg)
# open the client and pan/inspect:
#   http://localhost:5173/?user=Developer&x=9&y=3&ambient=1.5
```

### Inspect the running data
```bash
docker exec spacetime-start-1 spacetime sql resonantdust-dev-zone-0 \
  "SELECT zone_id, type_reference, version FROM cold WHERE zone_id = 0"
docker exec spacetime-start-1 spacetime sql resonantdust-dev-zone-0 \
  "SELECT entity_key, kind, zone_id, location FROM state"
```
`type_reference` decodes as `type_id = ref>>28 & 0xF` (1=biome-tile, 2=biome-thing),
`subtype = ref>>16 & 0xFFF` (the biome). Cold `kinds[]` entries decode via
`shared/codec/src/object.rs` (`kind_ref_kind_id`/`_x`/`_y`/`_variant_id`/`_data`).
