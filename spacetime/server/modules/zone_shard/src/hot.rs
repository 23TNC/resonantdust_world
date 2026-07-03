//! The hot overlay — one tiny row per *changed* cell, in two layer tables that
//! share an identical schema: `hot_tiles` and `hot_things`.
//!
//! Changing one cell fans out a ~16-byte row instead of rewriting the whole cold
//! zone. The GC fold ([`crate::gc`]) writes batches of at-rest hot cells back
//! into cold (one cold row per zone), then deletes them. The Gateway overlays
//! hot atop cold when serving the client.
//!
//! # Single source of truth
//!
//! The previous iteration's cardinal sin was a second module keeping a "drifted
//! partial copy of the write primitives." SpacetimeDB's index accessors
//! (`ctx.db.hot_tiles().zone_id()`) are concrete methods, not a generic trait, so
//! one generic `fn` can't span the tables. [`decl_cell_history!`] generates each
//! layer's bitemporal primitives, so both layers are bit-identical by
//! construction.

use spacetimedb::{reducer, table, ReducerContext};

// The two layer tables share an identical schema. The structs are declared
// explicitly (the `#[table]` proc-macro doesn't expand cleanly from inside a
// `macro_rules!` metavariable), but their bitemporal logic — where the old code
// actually drifted — is generated once by `decl_cell_history!` below, so both
// are bit-identical by construction.

/// One row per changed cell on the tile (floor/wall) layer.
#[table(accessor = hot_tiles, public)]
pub struct HotTile {
    #[primary_key]
    pub valid_at: u64,
    #[index(btree)]
    pub zone_id: u32,
    /// Cell index `0..256` within the zone (`y << 4 | x`).
    pub location: u8,
    /// Rotation `0..3` (ignored for the tile layer — tiles don't rotate).
    pub rotation: u8,
    /// Object/def id at this cell; `0` = cleared/empty.
    pub id: u16,
}

/// One row per changed cell on the thing layer. One thing per cell until a
/// layer field (carved from the packed thing's reserved bits) lets floor- and
/// wall-level things share a cell.
#[table(accessor = hot_things, public)]
pub struct HotThing {
    #[primary_key]
    pub valid_at: u64,
    #[index(btree)]
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
}

/// Generate the `valid_at` write primitives for a hot layer, keyed on
/// `(zone_id, location)`. `$hist` is the namespace module, `$accessor` the
/// table's accessor, `$struct` its row type.
macro_rules! decl_cell_history {
    ($hist:ident, $accessor:ident, $struct:ty) => {
        pub mod $hist {
            use super::*;
            use spacetimedb::Table;

            use crate::sequence;
            use resonantdust_codec::packed::{pack_valid_at, valid_at_time};
            use crate::time::now_ms;

            /// Row current at `time_ms` for cell `(zone_id, location)`.
            pub fn prior_at(
                ctx: &ReducerContext,
                zone_id: u32,
                location: u8,
                time_ms: u64,
            ) -> Option<$struct> {
                ctx.db
                    .$accessor()
                    .zone_id()
                    .filter(zone_id)
                    .filter(|r| r.location == location && valid_at_time(r.valid_at) <= time_ms)
                    .max_by_key(|r| valid_at_time(r.valid_at))
            }

            /// Row current at wall-clock now for cell `(zone_id, location)`.
            #[allow(dead_code)]
            pub fn latest(ctx: &ReducerContext, zone_id: u32, location: u8) -> Option<$struct> {
                prior_at(ctx, zone_id, location, now_ms(ctx))
            }

            /// Delete every version of cell `(zone_id, location)` at exactly
            /// `time_ms` — the same-time purge [`write_at`] does.
            pub fn delete_at(ctx: &ReducerContext, zone_id: u32, location: u8, time_ms: u64) {
                let pks: Vec<u64> = ctx
                    .db
                    .$accessor()
                    .zone_id()
                    .filter(zone_id)
                    .filter(|r| r.location == location && valid_at_time(r.valid_at) == time_ms)
                    .map(|r| r.valid_at)
                    .collect();
                for pk in pks {
                    ctx.db.$accessor().valid_at().delete(pk);
                }
            }

            /// Delete every version of cell `(zone_id, location)`, regardless of
            /// time — used by the fold once the cell is baked into cold.
            pub fn delete_all(ctx: &ReducerContext, zone_id: u32, location: u8) {
                let pks: Vec<u64> = ctx
                    .db
                    .$accessor()
                    .zone_id()
                    .filter(zone_id)
                    .filter(|r| r.location == location)
                    .map(|r| r.valid_at)
                    .collect();
                for pk in pks {
                    ctx.db.$accessor().valid_at().delete(pk);
                }
            }

            /// Stamp `valid_at = (time_ms, fresh seq)` and write, purging any
            /// same-(cell, time) row first. The single write entry point.
            pub fn write_at(ctx: &ReducerContext, mut row: $struct, time_ms: u64) -> $struct {
                delete_at(ctx, row.zone_id, row.location, time_ms);
                row.valid_at = pack_valid_at(time_ms, sequence::next_sequence(ctx));
                ctx.db.$accessor().insert(row)
            }

            /// The latest settled (`valid_at_time ≤ time_ms`) row for every cell
            /// that has any row in `zone_id` — one per `location`. The fold's
            /// "what's currently overlaid in this zone" query.
            pub fn cells_in_zone(
                ctx: &ReducerContext,
                zone_id: u32,
                time_ms: u64,
            ) -> Vec<$struct> {
                use std::collections::HashMap;
                let mut by_loc: HashMap<u8, $struct> = HashMap::new();
                for r in ctx.db.$accessor().zone_id().filter(zone_id) {
                    if valid_at_time(r.valid_at) > time_ms {
                        continue;
                    }
                    let newer = by_loc
                        .get(&r.location)
                        .map_or(true, |p| r.valid_at > p.valid_at);
                    if newer {
                        by_loc.insert(r.location, r);
                    }
                }
                by_loc.into_values().collect()
            }

            /// Reap every non-latest row (prior-version GC), keeping the
            /// max-`valid_at` row per `(zone_id, location)`.
            pub fn reap_prior(ctx: &ReducerContext) {
                use std::collections::HashMap;
                let mut latest_by_cell: HashMap<(u32, u8), u64> = HashMap::new();
                for r in ctx.db.$accessor().iter() {
                    let k = (r.zone_id, r.location);
                    latest_by_cell
                        .entry(k)
                        .and_modify(|m| {
                            if r.valid_at > *m {
                                *m = r.valid_at;
                            }
                        })
                        .or_insert(r.valid_at);
                }
                let mut to_delete: Vec<u64> = Vec::new();
                for r in ctx.db.$accessor().iter() {
                    if latest_by_cell.get(&(r.zone_id, r.location)) != Some(&r.valid_at) {
                        to_delete.push(r.valid_at);
                    }
                }
                for v in to_delete {
                    ctx.db.$accessor().valid_at().delete(v);
                }
            }
        }
    };
}

decl_cell_history!(tiles_hist, hot_tiles, HotTile);
decl_cell_history!(things_hist, hot_things, HotThing);

// ── layer dispatch ───────────────────────────────────────────────────────────

/// Floor/wall terrain layer (`hot_tiles`).
pub const LAYER_TILE: u8 = 0;
/// Thing layer (`hot_things`).
pub const LAYER_THING: u8 = 1;

/// Write one hot cell on the given layer at `time_ms`. Shared by [`set_hot`] and
/// [`apply_hot`].
fn write_cell(
    ctx: &ReducerContext,
    layer: u8,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    time_ms: u64,
) -> Result<(), String> {
    // Every layer's def folds into a u12 cold slot (tile `def_id:12`, thing
    // `object_id:12`); reject an out-of-range id rather than truncating on fold.
    if id > resonantdust_codec::packed::DEF_ID_MAX {
        return Err(format!(
            "hot: def id {id} exceeds u12 max {}",
            resonantdust_codec::packed::DEF_ID_MAX
        ));
    }
    match layer {
        LAYER_TILE => {
            tiles_hist::write_at(
                ctx,
                HotTile { valid_at: 0, zone_id, location, rotation, id },
                time_ms,
            );
        }
        LAYER_THING => {
            things_hist::write_at(
                ctx,
                HotThing { valid_at: 0, zone_id, location, rotation, id },
                time_ms,
            );
        }
        _ => return Err(format!("hot: unknown layer {layer}")),
    }
    Ok(())
}

/// Upsert a single hot cell. `now_ms` is the Gateway-resolved write time; the
/// shard trusts it (authorization is the Gateway's job).
#[reducer]
pub fn set_hot(
    ctx: &ReducerContext,
    now_ms: u64,
    layer: u8,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
) -> Result<(), String> {
    write_cell(ctx, layer, zone_id, location, rotation, id, now_ms)
}

/// Upsert many hot cells on one layer in a single transaction → one commit per
/// zone (the "one fully-formed commit, no mid-write flicker" discipline carried
/// forward). Args ride as parallel JSON arrays over the `/call` path.
#[reducer]
pub fn apply_hot(
    ctx: &ReducerContext,
    now_ms: u64,
    layer: u8,
    zone_id: u32,
    locations: Vec<u8>,
    rotations: Vec<u8>,
    ids: Vec<u16>,
) -> Result<(), String> {
    if locations.len() != rotations.len() || locations.len() != ids.len() {
        return Err(format!(
            "apply_hot: parallel vec length mismatch (locations={}, rotations={}, ids={})",
            locations.len(),
            rotations.len(),
            ids.len()
        ));
    }
    for i in 0..locations.len() {
        write_cell(ctx, layer, zone_id, locations[i], rotations[i], ids[i], now_ms)?;
    }
    Ok(())
}
