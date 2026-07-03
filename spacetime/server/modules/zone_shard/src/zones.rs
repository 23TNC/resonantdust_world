//! `cold_zones` — the settled baseline. One current row per zone (history by
//! `valid_at`), holding the full 16×16 tile terrain plus the thing list.
//!
//! Cold is big but rarely rewritten: a single cell change writes a tiny row in a
//! `hot_*` table ([`crate::hot`]) instead, and the GC fold ([`crate::gc`]) folds
//! at-rest hot cells back into one new cold row per zone. The Gateway overlays
//! hot atop cold when serving the client; the shard just stores both.
//!
//! There's exactly one cold table, so its bitemporal primitives are hand-written
//! here rather than macro-generated (the macro pays off for the three identical
//! hot layers — see [`crate::hot`]).

use spacetimedb::{reducer, table, ReducerContext, Table};

use crate::sequence;
use crate::time::now_ms;
use resonantdust_codec::packed::{
    cell_x, cell_y, pack_thing, pack_valid_at, thing_x, thing_y, valid_at_time, ZONE_TILES,
};

/// The settled baseline for a zone.
#[table(accessor = cold_zones, public)]
pub struct ColdZone {
    /// Primary key: `(time_ms u48 << 16) | seq`. Rows for one `zone_id` form a
    /// history; the fold writes a new (usually back-dated) version.
    #[primary_key]
    pub valid_at: u64,
    /// Logical id — opaque to the shard (Gateway encodes region + local zone).
    #[index(btree)]
    pub zone_id: u32,
    /// Terrain: `ZONE_TILES` (256) cells, row-major (`y << 4 | x`). Each is a u16
    /// tile slot `def_id:12 | reserved:4` (see `resonantdust_codec::packed::pack_tile`); `def_id == 0`
    /// = no floor/wall. Tiles don't carry rotation; the reserved nibble is
    /// headroom for later.
    pub tiles: Vec<u16>,
    /// Every thing in the zone — packed `u32`s (see [`resonantdust_codec::packed::pack_thing`]).
    /// One list per zone: a layer field (carved from a thing's reserved bits)
    /// will later distinguish floor-level from wall-level things.
    pub things: Vec<u32>,
}

/// A fresh, empty terrain array (`ZONE_TILES` zeroes).
pub fn empty_tiles() -> Vec<u16> {
    vec![0u16; ZONE_TILES]
}

/// Cold row current at `time_ms` for `zone_id` (max `valid_at_time ≤ time_ms`).
pub fn prior_at(ctx: &ReducerContext, zone_id: u32, time_ms: u64) -> Option<ColdZone> {
    ctx.db
        .cold_zones()
        .zone_id()
        .filter(zone_id)
        .filter(|z| valid_at_time(z.valid_at) <= time_ms)
        .max_by_key(|z| valid_at_time(z.valid_at))
}

/// Cold row current at wall-clock now.
pub fn latest(ctx: &ReducerContext, zone_id: u32) -> Option<ColdZone> {
    prior_at(ctx, zone_id, now_ms(ctx))
}

/// The baseline to fold into: the current cold row at `time_ms`, or a fresh empty
/// zone when none exists yet (its `valid_at` is `0`, time `0`).
pub fn baseline(ctx: &ReducerContext, zone_id: u32, time_ms: u64) -> ColdZone {
    prior_at(ctx, zone_id, time_ms).unwrap_or_else(|| ColdZone {
        valid_at: 0,
        zone_id,
        tiles: empty_tiles(),
        things: Vec::new(),
    })
}

/// Delete every cold row for `zone_id` at exactly `time_ms` — the same-time purge
/// [`write_at`] does so "last write at this (zone, ms) wins".
pub fn delete_at(ctx: &ReducerContext, zone_id: u32, time_ms: u64) {
    let pks: Vec<u64> = ctx
        .db
        .cold_zones()
        .zone_id()
        .filter(zone_id)
        .filter(|z| valid_at_time(z.valid_at) == time_ms)
        .map(|z| z.valid_at)
        .collect();
    for pk in pks {
        ctx.db.cold_zones().valid_at().delete(pk);
    }
}

/// Stamp `valid_at = (time_ms, fresh seq)` and write, purging any same-time row
/// first. The single cold write entry point.
pub fn write_at(ctx: &ReducerContext, mut zone: ColdZone, time_ms: u64) -> ColdZone {
    delete_at(ctx, zone.zone_id, time_ms);
    zone.valid_at = pack_valid_at(time_ms, sequence::next_sequence(ctx));
    ctx.db.cold_zones().insert(zone)
}

/// Reap every non-latest cold row (prior-version GC), keeping the max-`valid_at`
/// row per `zone_id`.
pub fn reap_prior(ctx: &ReducerContext) {
    use std::collections::HashMap;
    let mut latest_by_id: HashMap<u32, u64> = HashMap::new();
    for z in ctx.db.cold_zones().iter() {
        latest_by_id
            .entry(z.zone_id)
            .and_modify(|m| {
                if z.valid_at > *m {
                    *m = z.valid_at;
                }
            })
            .or_insert(z.valid_at);
    }
    let mut to_delete: Vec<u64> = Vec::new();
    for z in ctx.db.cold_zones().iter() {
        if latest_by_id.get(&z.zone_id) != Some(&z.valid_at) {
            to_delete.push(z.valid_at);
        }
    }
    for v in to_delete {
        ctx.db.cold_zones().valid_at().delete(v);
    }
}

/// Upsert a thing into a packed thing-vec at `location`: drop any existing entry
/// in that cell, then add the new one. `object_id == 0` means "remove" — the
/// cell is left empty. Used by the GC fold to apply a hot thing-cell into cold.
pub fn upsert_thing(vec: &mut Vec<u32>, location: u8, rotation: u8, object_id: u16) {
    let (x, y) = (cell_x(location), cell_y(location));
    vec.retain(|&p| !(thing_x(p) == x && thing_y(p) == y));
    if object_id != 0 {
        vec.push(pack_thing(x, y, rotation, object_id));
    }
}

/// Seed (or overwrite) a cold zone outright — the Gateway's worldgen / bulk-load
/// path. Trusts its args (authorization is the Gateway's job, same posture as the
/// old `apply_action`); `now_ms` is the Gateway-resolved write time.
#[reducer]
pub fn seed_cold_zone(
    ctx: &ReducerContext,
    now_ms: u64,
    zone_id: u32,
    tiles: Vec<u16>,
    things: Vec<u32>,
) -> Result<(), String> {
    if tiles.len() != ZONE_TILES {
        return Err(format!(
            "seed_cold_zone: tiles len {} != {ZONE_TILES}",
            tiles.len()
        ));
    }
    write_at(
        ctx,
        ColdZone {
            valid_at: 0,
            zone_id,
            tiles,
            things,
        },
        now_ms,
    );
    Ok(())
}
