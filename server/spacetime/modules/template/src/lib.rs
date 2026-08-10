//! Module template — the smallest SpacetimeDB module that exercises every piece
//! a real one needs. Copy the directory, rename the crate in `Cargo.toml`, set
//! the `database` in `spacetime.dev.json`, and replace the tables below.
//!
//! What it demonstrates, in the order you'll need it:
//!
//! | Piece | Where |
//! | --- | --- |
//! | a public table clients subscribe to | [`Entry`] |
//! | a private table only reducers see | [`Clock`] |
//! | one-shot setup on a fresh publish | [`init`] |
//! | a caller-invoked reducer | [`set_entry`] / [`clear_entry`] |
//! | a recurring scheduled reducer | [`tick`] |
//!
//! ## Conventions this file follows
//!
//! - **Upsert is delete-then-insert.** SpacetimeDB has no upsert primitive; the
//!   delete is a no-op when the row is absent. This is the house idiom for
//!   single-row keys.
//! - **Database names take no underscores.** A module directory may be
//!   `event_shard`, but its database is `resonantdust-dev-event-shard-0` — the
//!   daemon rejects underscores with "invalid characters in database name".
//!   `bin/st` hyphenates for you.
//! - **`init` runs once, on a fresh publish only.** A republish that keeps data
//!   does NOT re-run it. Anything `init` seeds (here: the [`Clock`] row driving
//!   [`tick`]) is gone for good if it's deleted out from under you — recover by
//!   republishing with a data wipe.
//! - **Reducers trust their arguments.** There is no session table here and
//!   `ctx.sender()` is not cross-referenced, so anyone who can reach the reducer
//!   endpoint can write anything. Authorization belongs to whatever fronts this
//!   module; keep it that way rather than half-validating here.

use spacetimedb::{reducer, table, ReducerContext, ScheduleAt, Table, TimeDuration};

/// How often [`tick`] fires once [`init`] has seeded the schedule.
const TICK_INTERVAL_MS: i64 = 1_000;

// ── tables ───────────────────────────────────────────────────────────────────

/// The one public table — `public` is what makes rows visible to subscribers.
/// Drop the keyword and clients see nothing, however correct the reducers are.
#[table(accessor = entries, public)]
pub struct Entry {
    /// Caller-supplied identity for the row. `#[primary_key]` gives the generated
    /// `.id()` accessor used for point lookup and delete below.
    #[primary_key]
    pub id: u32,
    /// A secondary index. Add these deliberately: each one costs write time and
    /// memory, and an unindexed filter is a full scan.
    #[index(btree)]
    pub kind: u16,
    /// Last-writer-wins payload. Real modules carry packed integers here rather
    /// than strings — see `docs/` for the wire layouts once they exist.
    pub value: String,
    /// Daemon time at write, microseconds since the epoch. Server-stamped from
    /// `ctx.timestamp` — never trust a caller-supplied clock.
    pub written_at: i64,
}

/// Private — a table is private unless it says `public`, so the omission below is
/// the declaration. Reducer-visible only, never subscribed. Also the schedule
/// row: `scheduled(tick)` makes the daemon call [`tick`] per its `scheduled_at`,
/// and requires the two fields named below verbatim.
#[table(accessor = clock, scheduled(tick))]
pub struct Clock {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    /// Count of [`tick`] fires since the last fresh publish. Proof-of-life: if
    /// this stops climbing, the schedule row is gone and the sweep is dead.
    pub ticks: u64,
}

// ── reducers ─────────────────────────────────────────────────────────────────

/// One-shot on a fresh publish. Seeds the recurring [`tick`] schedule.
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    ctx.db.clock().insert(Clock {
        scheduled_id: 0, // #[auto_inc] — the daemon assigns the real id
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_micros(TICK_INTERVAL_MS * 1_000)),
        ticks: 0,
    });
}

/// Insert or replace one [`Entry`]. Upsert = delete-then-insert; the delete is a
/// no-op when `id` is absent.
#[reducer]
pub fn set_entry(ctx: &ReducerContext, id: u32, kind: u16, value: String) -> Result<(), String> {
    ctx.db.entries().id().delete(id);
    ctx.db.entries().insert(Entry {
        id,
        kind,
        value,
        written_at: ctx.timestamp.to_micros_since_unix_epoch(),
    });
    Ok(())
}

/// Delete one [`Entry`]. Absent `id` is not an error — callers retry, and a
/// delete that fails on "already gone" turns a retry into a spurious failure.
#[reducer]
pub fn clear_entry(ctx: &ReducerContext, id: u32) -> Result<(), String> {
    ctx.db.entries().id().delete(id);
    Ok(())
}

/// Fires every [`TICK_INTERVAL_MS`]. Only the daemon may call a scheduled
/// reducer — the `ctx.sender` guard rejects a client invoking it directly, and
/// every scheduled reducer wants that guard.
#[reducer]
pub fn tick(ctx: &ReducerContext, arg: Clock) -> Result<(), String> {
    if ctx.sender() != ctx.identity() {
        return Err("tick is scheduled-only".into());
    }
    ctx.db.clock().scheduled_id().delete(arg.scheduled_id);
    ctx.db.clock().insert(Clock {
        ticks: arg.ticks + 1,
        ..arg
    });
    Ok(())
}
