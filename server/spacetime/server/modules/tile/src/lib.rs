//! tile — the **cold** shard for a zone's ground tiles.
//!
//! One row per `(zone, layer)`: the dense 16×16 = 256 `kind_reference`s, indexed by `tile_reference`
//! (0..256). A tile carries only **what kind** it is — no per-entry `tile_reference` (that's the
//! index) and no `data`, which is what lets a tile be a `u16` where a thing needs a `u32`.
//!
//! Cold is static-ish: worldgen seeds it, the edge subscribes it per zone, the client draws the
//! ground. Mutation (chop/place) comes later via `UNPACK` → hot → `PACK`; this first increment is
//! storage + `seed` + the zone-keyed subscription, no tics.
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference` = `macro_position:16 | layer_reference:8`),
//! `docs/TABLES.md`. Server = this module, so it's out of the key.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::object::pack_cold_row_reference;

/// A zone is 16×16 tiles.
const TILES_PER_ZONE: usize = 256;

// ── cold_tile — one zone-layer's dense ground ───────────────────────────────────────

#[table(accessor = cold_tile, public)]
pub struct ColdTile {
    /// `cold_row_reference` = `macro_position:16 | layer_reference:8`.
    #[primary_key]
    pub cold_row_reference: u32,
    /// The zone — the client's subscription key (`WHERE macro_position_reference = <zone>`).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// `type_id:4 | layer_id:4`.
    pub layer_reference: u8,
    /// 256 `kind_reference`s, one per tile (index = `tile_reference`). Dense.
    pub tiles: Vec<u16>,
}

#[reducer(init)]
pub fn init(_ctx: &ReducerContext) {}

/// Seed (or overwrite) a zone-layer's ground. This is trusted server-side **generation** (worldgen),
/// not a player mutation — player edits go through `UNPACK` → the pipeline. Idempotent: re-seeding a
/// zone overwrites deterministically.
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    layer_reference: u8,
    tiles: Vec<u16>,
) -> Result<(), String> {
    if tiles.len() != TILES_PER_ZONE {
        return Err(format!("a zone has {TILES_PER_ZONE} tiles, got {}", tiles.len()));
    }
    let cold_row_reference = pack_cold_row_reference(macro_position, layer_reference);
    let row = ColdTile { cold_row_reference, macro_position_reference: macro_position, layer_reference, tiles };
    if ctx.db.cold_tile().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.cold_tile().cold_row_reference().update(row);
    } else {
        ctx.db.cold_tile().insert(row);
    }
    Ok(())
}
