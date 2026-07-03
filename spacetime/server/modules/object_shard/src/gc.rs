//! Recurring garbage-collection sweep.
//!
//! Runs every [`GC_INTERVAL_MS`] on a schedule seeded by [`init`]. Unlike the
//! `zone_shard` GC there is **no hot→cold fold** — the object shard has no cold
//! layer; loose things stay mobile and leave only by a settle transfer. Two jobs:
//!
//! 1. **Prior-version reap** — keep the latest version per `object_id`, drop the
//!    rest (loose things churn position heavily, so history piles up fast).
//! 2. **Presence reconcile** — recompute per-region counts from ground truth,
//!    correcting any drift the incremental `inc`/`dec` couldn't (snapshot restore,
//!    migration, bugs). Also run on `init`, treating counts as dirty on load.

use spacetimedb::{reducer, table, ReducerContext, ScheduleAt, Table, TimeDuration};

use crate::free_things;
use crate::presence;

/// Sweep interval. 10 minutes (matches `zone_shard`).
const GC_INTERVAL_MS: i64 = 10 * 60 * 1_000;

/// Recurring schedule. Single row, `ScheduleAt::Interval`, seeded by [`init`].
#[table(accessor = gc_schedule, scheduled(gc_sweep))]
pub struct GcSchedule {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub scheduled_at: ScheduleAt,
}

/// Module init — runs once on fresh publish. Seeds the recurring GC schedule and
/// recomputes presence from ground truth (dirty-on-load self-heal).
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
    presence::recount(ctx);
}

/// Periodic sweep: reap prior versions, then reconcile presence.
#[reducer]
pub fn gc_sweep(ctx: &ReducerContext, _row: GcSchedule) -> Result<(), String> {
    free_things::reap_prior(ctx);
    presence::recount(ctx);
    Ok(())
}
