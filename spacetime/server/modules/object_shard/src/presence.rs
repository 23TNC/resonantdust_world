//! `presence` — per-region count of mobile objects (loose things **and** pawns).
//!
//! The object shard's **routing table**: gates subscribe every object shard's
//! `presence` and, from it, decide which shards to subscribe for a given region
//! (object routing is dynamic — placement chooses the shard at release time —
//! unlike zone shards' static `index` routing). See docs/object-shard.md. A
//! region with *any* mobile object (a thing or a pawn) must route, so the count
//! spans both tables; `object_id`s are unique across them (shared allocator), so
//! nothing double-counts.
//!
//! Counts are maintained **transactionally with the row** in the `free_things`
//! and `pawns` reducers (create → `inc`, region-change → `dec`+`inc`, remove →
//! `dec`), so an ordinary crash rolls both back together. [`recount`] rebuilds
//! from ground truth on `init` and on every GC sweep, covering the cases
//! atomicity doesn't: snapshot restore, migration, logic bugs.

use std::collections::HashMap;

use spacetimedb::{table, ReducerContext, Table};

use resonantdust_codec::packed::region_of;

use crate::free_things::free_things;
use crate::pawns::pawns;

/// One row per region that currently holds any loose thing. Absent row = 0.
#[table(accessor = presence, public)]
pub struct Presence {
    #[primary_key]
    pub region_id: u32,
    pub count: u32,
}

/// Current count for `region_id` (0 if no row).
pub fn get(ctx: &ReducerContext, region_id: u32) -> u32 {
    ctx.db
        .presence()
        .region_id()
        .find(region_id)
        .map(|p| p.count)
        .unwrap_or(0)
}

/// Set `region_id`'s count, deleting the row at 0 to keep the table sparse.
fn set(ctx: &ReducerContext, region_id: u32, count: u32) {
    ctx.db.presence().region_id().delete(region_id);
    if count > 0 {
        ctx.db.presence().insert(Presence { region_id, count });
    }
}

/// Increment `region_id`'s count.
pub fn inc(ctx: &ReducerContext, region_id: u32) {
    set(ctx, region_id, get(ctx, region_id).saturating_add(1));
}

/// Decrement `region_id`'s count (saturating; drops the row at 0).
pub fn dec(ctx: &ReducerContext, region_id: u32) {
    set(ctx, region_id, get(ctx, region_id).saturating_sub(1));
}

/// Rebuild every count from ground truth: tally the latest version of each
/// distinct `object_id` by its current region, then replace the table. Distinct
/// by `object_id` so bitemporal history rows never double-count.
pub fn recount(ctx: &ReducerContext) {
    // object_id -> (latest valid_at, its zone_id), across BOTH free things and
    // pawns (disjoint object_id spaces, so one map covers both).
    let mut latest_by_obj: HashMap<u64, (u64, u32)> = HashMap::new();
    for r in ctx.db.free_things().iter() {
        let e = latest_by_obj.entry(r.object_id).or_insert((r.valid_at, r.zone_id));
        if r.valid_at > e.0 {
            *e = (r.valid_at, r.zone_id);
        }
    }
    for r in ctx.db.pawns().iter() {
        let e = latest_by_obj.entry(r.object_id).or_insert((r.valid_at, r.zone_id));
        if r.valid_at > e.0 {
            *e = (r.valid_at, r.zone_id);
        }
    }

    let mut counts: HashMap<u32, u32> = HashMap::new();
    for (_obj, (_va, zone_id)) in latest_by_obj {
        *counts.entry(region_of(zone_id)).or_insert(0) += 1;
    }

    // Replace: clear existing rows, then write the fresh non-zero counts.
    let existing: Vec<u32> = ctx.db.presence().iter().map(|p| p.region_id).collect();
    for region_id in existing {
        ctx.db.presence().region_id().delete(region_id);
    }
    for (region_id, count) in counts {
        ctx.db.presence().insert(Presence { region_id, count });
    }
}
