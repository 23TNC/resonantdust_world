//! `can_exist` — the per-zone "this zone is allowed to be generated" gate. One
//! row per zone the world is permitted to materialise; the server seeds a fresh
//! zone's terrain only when its zone is both *can-exist* (a row here) and not yet
//! *exists* (no `cold_zone` row — see the server's `seed_zone_if_empty`).
//!
//! Today this is the crudest possible policy: module init ([`crate::gc::init`])
//! calls [`seed_region_00`] to stamp `can_exist` for every zone in region (0, 0),
//! giving us exactly one playable region for testing. How the allowed set is
//! really decided (world bounds, content-driven region maps, neighbour growth …)
//! is left for later — this table is the seam that lets the answer change without
//! touching the seed path.

use spacetimedb::{table, ReducerContext, Table};

use resonantdust_codec::packed::{pack_zone_id, REGION_DIM};

/// A zone the world is allowed to generate. Presence is the gate; `can_exist`
/// carries the flag explicitly so the column can later grow into a richer policy
/// value (weights, biome hints, …) without reshaping the table.
#[table(accessor = can_exist, public)]
pub struct CanExist {
    /// The zone this row permits — `region_x:8 | region_y:8 | surface:8 | zone_x:4 | zone_y:4`.
    #[primary_key]
    pub zone_id: u32,
    /// Whether the zone may be generated. Always `true` for now (a row is only
    /// written for allowed zones); kept as a field so disallowing a zone later
    /// can flip the flag instead of deleting the row.
    pub can_exist: bool,
}

/// Mark every zone in region (0, 0) as allowed, so we have one region to play in.
/// Called from module init ([`crate::gc::init`]); idempotent — re-running leaves
/// existing rows untouched.
pub fn seed_region_00(ctx: &ReducerContext) {
    for zone_x in 0..REGION_DIM {
        for zone_y in 0..REGION_DIM {
            // region (0, 0), surface 0 — the test region.
            let zone_id = pack_zone_id(0, 0, 0, zone_x, zone_y);
            if ctx.db.can_exist().zone_id().find(zone_id).is_none() {
                ctx.db.can_exist().insert(CanExist { zone_id, can_exist: true });
            }
        }
    }
}
