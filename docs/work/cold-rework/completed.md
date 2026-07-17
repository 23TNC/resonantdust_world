# Completed — cold-rework

_Landed items, newest last. One commit each, verified against its surface._

---

## P1 · Subtype fix — restore biome, `type_id` → shard

Reworked `cold_row_reference` from `macro:16 | layer_reference:8 | reserved:8` (which had silently
dropped `subtype_id` = the biome) to **`macro:16 | subtype:12 | layer_id:4`**. `type_id` is now the
shard (`tile` module = `TYPE_BIOME_TILE`, `thing` = `TYPE_BIOME_THING`), reconstructed at read time.

- **codec** `object`: `pack_cold_row_reference(macro, subtype_id, layer_id)` + `cold_row_subtype` /
  `cold_row_layer_id` accessors; deleted `cold_row_layer_reference` / `cold_row_of` (a position no
  longer names a row — it lacks the biome). New composite test.
- **modules** `tile`/`thing`: `cold_tile`/`cold_thing` now carry `subtype_id` (u16, idx) + `layer_id`
  (u8), drop `layer_reference`; `seed(macro, subtype_id, layer_id, payload)`. Bindings regenerated.
- **worldgen** `zone_cold`: uses `gen.biome → biome_subtype_id`, **groups a zone's cells by subtype**,
  returns `ColdLayers { tiles: Vec<(subtype, 256)>, things: Vec<(subtype, sparse)> }` — one row per
  biome present (dense cells outside the biome `0`). Tests merge the per-biome rows.
- **edge** `seed_zone`: loops the biome groups, one `seed` per row. `ColdTile`/`ColdThing` frames carry
  `subtype_id` + `layer_id` (not `layer_reference`).
- **client**: `Event::ColdTiles`/`ColdThings` + wire mirror carry `subtype_id`/`layer_id`; wasm
  marshals `subtypeId`/`layerId`; `zone_cold_prims` takes `type_id` (the shard/frame) instead of
  `layer_reference`; pixijs keys cold rows by `(macro, subtype, layer)` so a zone's several biome-rows
  coexist, and passes `typeBiomeThing()` to the scatter prims.

**Verified (build gates):** codec object tests (9, incl. the new composite); `rd build core --check`;
`rd build shared`; pixijs `tsc --noEmit`; `rd build spacetime tile thing` (bindings regenerated); edge
`cargo test worldgen` (7, incl. the grouped-by-subtype + seam tests). **Browser pixel-confirm pending a
redeploy + re-seed** (schema change — the deployed cold tables are the old shape until published).

---

## P2 · Region router — `index.cold_shards`

Stood up the position → cold-shard indirection, region-keyed.

- **module** `index`: `cold_shards` (`route_reference` PK = `type_id:4 | region:8`, `type_id` +
  `region_reference` idx, `shard_reference`, `url`, `db_name`) + `set_cold_shard` / `remove_cold_shard`
  reducers. Bindings regenerated.
- **seed** (`rd index seed`, the dev bootstrap per F2): a `cold <type> <region> <shard> <url> <db>`
  manifest row → `set_cold_shard`. `content/servers/dev` routes region 0 (every visible zone today)
  → the local `tile-0` / `thing-0` for both families.
- **edge**: subscribes `cold_shards` on the index connection; `Pool::cold_endpoint(type, region)`
  resolves the endpoint; `build_world` connects cold through it (region-0 bootstrap), falling back to
  the configured default when a region is unrouted.

**Deferred (documented in `build_world`):** the multi-region → multi-shard *connection* pool — the edge
today keeps one tile + one thing connection resolved for region 0, which is correct while single-shard;
per-region lazy connections (resolve at subscribe time) land when a second cold shard is actually
deployed. The routing directory + reducers + resolution path — the durable, reusable part — are done.

**Verified:** `rd build spacetime index` (cold_shards + reducers compile, bindings regenerated); edge
`cargo check --all-targets` clean with the subscription + resolver + `build_world` routing. Live
routing exercised on the next `rd redeploy --run` (re-seeds `cold_shards`).
