//! tile — the **cold** shard for a zone's ground tiles (`TYPE_BIOME_TILE`).
//!
//! One row per `(zone, biome, layer)`: the dense 16×16 = 256 `kind_reference`s, indexed by
//! `tile_reference` (0..256). A tile carries only **what kind** it is — no per-entry `tile_reference`
//! (that's the index) and no `data`, which is what lets a tile be a `u16` where a thing needs a
//! `u32`. A zone spanning several biomes is several rows (one per `subtype`); cells outside a row's
//! biome are `0`.
//!
//! Cold is static-ish: worldgen seeds it, the edge subscribes it per zone, the client draws the
//! ground. Mutation (chop/place) comes later via the `state`/`state_log` overlay; this increment is
//! storage + `seed` + the zone-keyed subscription, no tics.
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference` = `macro_position:16 | subtype_id:12 |
//! layer_id:4`), `docs/TABLES.md`. **`type_id` is this module** (`TYPE_BIOME_TILE`), so it's off the
//! row — reconstruct with `type_reference = TYPE_BIOME_TILE | subtype_id`.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::object::pack_cold_row_reference;

/// A zone is 16×16 tiles.
const TILES_PER_ZONE: usize = 256;

// ── cold_tile — one zone-layer-biome's dense ground ─────────────────────────────────

#[table(accessor = cold_tile, public)]
pub struct ColdTile {
    /// `cold_row_reference` = `macro_position:16 | subtype_id:12 | layer_id:4`.
    #[primary_key]
    pub cold_row_reference: u32,
    /// The zone — the client's subscription key (`WHERE macro_position_reference = <zone>`).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// The biome (holds `u12`); searchable.
    #[index(btree)]
    pub subtype_id: u16,
    /// The layer (holds `u4`).
    pub layer_id: u8,
    /// 256 `kind_reference`s, one per tile (index = `tile_reference`); cells outside this biome are
    /// `0`. Dense.
    pub tiles: Vec<u16>,
}

#[reducer(init)]
pub fn init(_ctx: &ReducerContext) {}

/// Seed (or overwrite) a zone-layer-biome's ground. This is trusted server-side **generation**
/// (worldgen), not a player mutation — player edits go through the overlay. `type_id` is the module.
/// Idempotent: re-seeding a `(zone, subtype, layer)` overwrites deterministically.
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tiles: Vec<u16>,
) -> Result<(), String> {
    if tiles.len() != TILES_PER_ZONE {
        return Err(format!("a zone has {TILES_PER_ZONE} tiles, got {}", tiles.len()));
    }
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    let row = ColdTile {
        cold_row_reference,
        macro_position_reference: macro_position,
        subtype_id,
        layer_id,
        tiles,
    };
    if ctx.db.cold_tile().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.cold_tile().cold_row_reference().update(row);
    } else {
        ctx.db.cold_tile().insert(row);
    }
    Ok(())
}
