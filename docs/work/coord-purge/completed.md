# Completed — coord-purge

_Landed items, newest last. One commit each, verified against its surface._

---

## A · Authority — world dims + `macro_world_origin` in VARIABLES / object

Moved `ZONE_DIM`/`REGION_DIM`/`REALM_DIM` = 16, `ZONE_TILES` = 256, `REGION_TILES`/`REALM_TILES`
from `packed` into `shared/codec/object` (packed now re-exports them via `pub use`). Added
`macro_world_origin(macro_position_reference) -> (i32, i32)` in `object` — the region/zone nibbles →
world-tile origin, replacing `biome::zone_world_origin` → `packed::global_tile`. Documented the
world-structure table in [`VARIABLES.md`](../../VARIABLES.md) (neutral world-size knobs, not legacy).

**Verified:** 47 codec tests pass incl. `macro_world_origin_tiles_continuously` (adjacent macros
differ by exactly `ZONE_DIM`); `rd build core --check` still green (the packed→object re-export
didn't break consumers). Commit `e388654`.

---

## B · Delete the dead worldgen generators (+ port the seam test)

Deleted `zone_terrain(zone_id) -> (Vec<u8>, Vec<u64>)`, `zone_cold_objects(zone_id) -> Vec<ColdRow>`,
the `ColdRow` struct, the `WORLDGEN_LAYER` const, and their now-dead imports (`packed::{cell,
pack_thing_at}`, `object::{pack_type_reference, TYPE_BIOME_THING, TYPE_BIOME_TILE}`). `zone_cold` is
the only remaining generator; it now imports `ZONE_DIM`/`ZONE_TILES` from `object` (not `packed`).

Ported the four value-carrying tests onto `zone_cold` (they read `kind_reference`s now, via
`kind_ref_kind_id` / `kind_pos_ref_kind_id`): `every_cell_gets_a_known_tile`, `a_region_shows_variety`,
`deterministic_reseed`, and — the one that guards the fixed transposition bug —
`seamless_across_zone_boundary` (west zone's `tile_x=15` column vs east zone's `tile_x=0`, read with
`pack_tile_reference`, must match adjacent world columns 15/16). Deleted the two old-model tests
(`cold_objects_carry_biome_subtype_and_kind`, `cold_objects_match_the_legacy_terrain`) and reshaped
`things_land_on_grass_cells_only` → `things_scatter_kind_tree_only`.

**Verified:** `cargo test worldgen` in the edge build container — 7 passed, 0 failed (incl. the ported
seam test); edge compiles clean; no external references to the deleted `pub` symbols.

---

## C · Delete the dead zone→shard router in `index.rs`

Deleted `server/edge/src/index.rs` whole (`ShardEndpoint`, `resolve_zone_or_default`, the `region_of`
re-export) and its `mod index;` — every item was `#[allow(dead_code)]`, no consumer. Removed the dead
router bleed from the rest of the edge: the `region_shards`/`shards` subscription + count-log +
binding imports in `connections.rs` (the index connection is now write-only — the `set_server`
registration heartbeat, i.e. live player routing, is all it does), and `config::default_shard_db()`
(the router's single-shard fallback, only `resolve_zone_or_default` called it). Refreshed the stale
router prose in `config.rs` / `ws.rs` docs.

**Tables decision — leave inert (not drop from the module).** `region_shards`/`shards` stay in the
deployed `index` module but with no edge consumer. Dropping them would mean editing a live module +
regenerating two binding sets + a redeploy, for tables the future router won't reuse (it'll be
macro-keyed, per the README). The edge code is fully purged either way — that's what the invariant
asks. TABLES marks both **Dead / inert**.

**Left untouched:** `servers` / `player_servers` / `master_clock` (live player routing + durable tic),
and `data_shard_db()` (the live pipeline data shard — unrelated to the region router).

**Verified:** `cargo check --all-targets` in the edge build container — clean, no warnings; grep
confirms no `region_of` / `resolve_zone_or_default` / `default_shard_db` / `crate::index` left in the
edge source; `set_server` registration path intact.
