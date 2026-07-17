//! thing — the **cold** shard for a zone's scattered things (trees, rocks, …).
//!
//! One row per `(zone, layer)`: a **sparse** `Vec<u32>` of `kind_pos_reference`s
//! (`kind_reference:16 | tile_reference:8 | data:8`). Unlike a tile — which is only *what kind* and
//! whose position is its dense index — a thing carries *where in the zone* it sits and *its data*
//! (facing/count), so it needs the full `u32` and can't be indexed by position. Only occupied cells
//! appear.
//!
//! Cold is static-ish: worldgen seeds it, the edge subscribes it per zone, the client draws the
//! scatter over the ground. Mutation (chop/place) comes later via `UNPACK` → hot → `PACK`; this
//! increment is storage + `seed` + the zone-keyed subscription, no tics.
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference`, `kind_pos_reference`), `docs/TABLES.md`.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::object::pack_cold_row_reference;

// ── cold_thing — one zone-layer's sparse scatter ────────────────────────────────────

#[table(accessor = cold_thing, public)]
pub struct ColdThing {
    /// `cold_row_reference` = `macro_position:16 | layer_reference:8`.
    #[primary_key]
    pub cold_row_reference: u32,
    /// The zone — the client's subscription key (`WHERE macro_position_reference = <zone>`).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// `type_id:4 | layer_id:4`.
    pub layer_reference: u8,
    /// Sparse `kind_pos_reference`s — one per occupied cell (`kind:16 | tile:8 | data:8`).
    pub things: Vec<u32>,
}

#[reducer(init)]
pub fn init(_ctx: &ReducerContext) {}

/// Seed (or overwrite) a zone-layer's scatter. Trusted server-side **generation** (worldgen), not a
/// player mutation — player edits go through `UNPACK` → the pipeline. Idempotent: re-seeding a zone
/// overwrites deterministically. (Sparse, so no fixed length; entries are expected to hold distinct
/// `tile_reference`s — one thing per cell per layer — but that isn't enforced here.)
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    layer_reference: u8,
    things: Vec<u32>,
) -> Result<(), String> {
    let cold_row_reference = pack_cold_row_reference(macro_position, layer_reference);
    let row = ColdThing { cold_row_reference, macro_position_reference: macro_position, layer_reference, things };
    if ctx.db.cold_thing().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.cold_thing().cold_row_reference().update(row);
    } else {
        ctx.db.cold_thing().insert(row);
    }
    Ok(())
}
