//! Recurring garbage-collection sweep.
//!
//! Runs every [`GC_INTERVAL_MS`] on a schedule seeded by [`init`]. Two jobs:
//!
//! 1. **Hot → cold fold** ([`fold_hot_to_cold`]) — at-rest hot cells are folded
//!    back into one new `cold_zone` row per zone (minimising fan-out), then the
//!    folded hot rows are deleted. The new cold row is **back-dated** to the
//!    folded cells' own timestamps so it's promotable the instant the hot rows
//!    vanish — avoiding the "fold-back flash" (tile snapping to the stale
//!    pre-change baseline for a buffer-length) the old tile-card demotion hit.
//! 2. **Prior-version reap** — keep the latest row per id across every table,
//!    drop the rest.

use std::collections::BTreeSet;

use spacetimedb::{reducer, table, ReducerContext, ScheduleAt, Table, TimeDuration};

use crate::hot::{hot_things, hot_tiles};
use crate::hot::{things_hist, tiles_hist, LAYER_THING, LAYER_TILE};
use crate::zones;
use resonantdust_codec::packed::valid_at_time;

/// Sweep interval. 10 minutes.
const GC_INTERVAL_MS: i64 = 10 * 60 * 1_000;

/// A hot cell is foldable once its latest version has been at rest this long.
/// Comfortably past the client's promotion buffer so a cell that's still being
/// actively rewritten stays hot (cheap small-row fan-out) rather than thrashing
/// the cold zone. Tune as the write cadence becomes clear.
const FOLD_HORIZON_MS: u64 = 30 * 1_000;

/// Recurring schedule. Single row, `ScheduleAt::Interval`, seeded by [`init`].
#[table(accessor = gc_schedule, scheduled(gc_sweep))]
pub struct GcSchedule {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub scheduled_at: ScheduleAt,
}

/// Module init — runs once on fresh publish. Seeds the recurring GC schedule and
/// the `can_exist` gate (region (0, 0), for testing).
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    if ctx.db.gc_schedule().iter().next().is_none() {
        ctx.db.gc_schedule().insert(GcSchedule {
            id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_micros(
                GC_INTERVAL_MS.saturating_mul(1_000),
            )),
        });
    }
    crate::exists::seed_region_00(ctx);
}

fn now_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000) as u64
}

/// Periodic sweep. Fold first (so the freshly-written cold rows survive the reap
/// and the retired hot/cold versions are collected), then reap prior versions.
#[reducer]
pub fn gc_sweep(ctx: &ReducerContext, _row: GcSchedule) -> Result<(), String> {
    let now = now_ms(ctx);
    fold_hot_to_cold(ctx, now);
    zones::reap_prior(ctx);
    tiles_hist::reap_prior(ctx);
    things_hist::reap_prior(ctx);
    Ok(())
}

/// Fold every zone's at-rest hot cells into one new cold row, then delete the
/// folded hot rows. Batched per zone so a zone pays the cold-rewrite cost once
/// per sweep regardless of how many of its cells changed.
fn fold_hot_to_cold(ctx: &ReducerContext, now: u64) {
    let horizon = now.saturating_sub(FOLD_HORIZON_MS);

    // Every zone with any hot row, across the two layers.
    let mut zone_ids: BTreeSet<u32> = BTreeSet::new();
    for r in ctx.db.hot_tiles().iter() {
        zone_ids.insert(r.zone_id);
    }
    for r in ctx.db.hot_things().iter() {
        zone_ids.insert(r.zone_id);
    }

    for zone_id in zone_ids {
        let mut zone = zones::baseline(ctx, zone_id, now);
        // The new cold row must be the latest cold version and ≥ the cells it
        // bakes in; seed from the baseline's own time so a fold never lands an
        // older-than-current cold row (which the reap would drop, losing the fold).
        let mut fold_time = valid_at_time(zone.valid_at);
        // Folded cells to delete only after the cold write confirms.
        let mut folded: Vec<(u8, u8)> = Vec::new(); // (layer, location)

        // Tiles: set the cell's u12 def (id 0 clears it), preserving the existing
        // reserved nibble — the hot row carries only the def, not reserved bits.
        for c in tiles_hist::cells_in_zone(ctx, zone_id, now) {
            if valid_at_time(c.valid_at) > horizon {
                continue; // still hot — leave as overlay.
            }
            let slot = &mut zone.tiles[c.location as usize];
            *slot = resonantdust_codec::packed::pack_tile(
                c.id,
                resonantdust_codec::packed::tile_reserved(*slot),
            );
            fold_time = fold_time.max(valid_at_time(c.valid_at));
            folded.push((LAYER_TILE, c.location));
        }
        // Things.
        for c in things_hist::cells_in_zone(ctx, zone_id, now) {
            if valid_at_time(c.valid_at) > horizon {
                continue;
            }
            zones::upsert_thing(&mut zone.things, c.location, c.rotation, c.id);
            fold_time = fold_time.max(valid_at_time(c.valid_at));
            folded.push((LAYER_THING, c.location));
        }

        if folded.is_empty() {
            continue;
        }

        // One cold version reconciles every folded cell in this zone, back-dated
        // to the latest folded cell's time (clamped ≥ the baseline's own time).
        zone.zone_id = zone_id;
        zones::write_at(ctx, zone, fold_time);

        // The cells are now baked into cold — drop every version of each.
        for (layer, location) in folded {
            match layer {
                LAYER_TILE => tiles_hist::delete_all(ctx, zone_id, location),
                LAYER_THING => things_hist::delete_all(ctx, zone_id, location),
                _ => {}
            }
        }
    }
}
