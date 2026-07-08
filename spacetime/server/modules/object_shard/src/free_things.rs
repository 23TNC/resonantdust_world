//! `free_things` — one row per loose thing.
//!
//! A loose thing is the object-shard analogue of a `zone_shard` `hot_thing`
//! (`zone_id`, `location`, `rotation`, `id`) **plus** a `u8` sub-tile `offset`
//! (`resonantdust_codec::packed`: `x_off:4 | y_off:4`, 1/16-tile steps). The
//! offset is why loose things need their own representation: affixed things are
//! tile-snapped, a loose thing sits anywhere in its tile so it can move smoothly.
//!
//! Unlike a hot cell — keyed by `(zone_id, location)`, one thing per cell — a
//! loose thing has a **stable `object_id`** instance handle. With a sub-tile
//! offset several loose things can share a tile, so `location` can't identify one;
//! the `object_id` (allocated once, preserved across every move/version) is how a
//! move or a remove addresses a specific thing. It's also the handle a future
//! settle-transfer carries back into a zone shard. (This is the minimal resolution
//! of docs/object-shard.md's open identity question; a `parent`/attachment field
//! for pawn-carry lands later.)
//!
//! Bitemporal like the rest of the game: `valid_at` PK, history per `object_id`,
//! reaped to the latest version. There is **no hot→cold fold** — loose things move
//! freely and only leave the shard by an explicit settle transfer.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::packed::{pack_valid_at, region_of, valid_at_time, DEF_ID_MAX};

use crate::presence;
use crate::sequence;
use crate::time::now_ms;

/// One loose thing. The `hot_thing` columns + a stable `object_id` + `offset`.
#[table(accessor = free_things, public)]
pub struct FreeThing {
    /// Bitemporal version stamp: `(time_ms u48 << 16) | seq`. Rows for one
    /// `object_id` form a history; the latest is the current state.
    #[primary_key]
    pub valid_at: u64,
    /// Stable instance handle — allocated once ([`next_object_id`]) and kept
    /// across every move and version. Non-unique here (one per version row).
    #[index(btree)]
    pub object_id: u64,
    /// Zone the thing is currently in. Changes freely as it moves (even across a
    /// region boundary — that's a plain update, not a transfer).
    #[index(btree)]
    pub zone_id: u32,
    /// Tile cell within the zone (`y << 4 | x`, `0..256`) — the coarse position.
    pub location: u8,
    /// Rotation. Stored raw; masked to 2 bits if/when settled into a packed thing.
    pub rotation: u8,
    /// What-kind: the u12 def id (same namespace as a packed thing's `object_id`),
    /// so a settle can drop it straight into a zone's thing slot.
    pub id: u16,
    /// Sub-tile position within the cell — `x_off:4 | y_off:4`, 1/16-tile steps
    /// (`resonantdust_codec::packed::pack_offset`). Tile-snapped affixed things
    /// have no equivalent; this is the loose thing's extra degree of freedom.
    pub offset: u8,
}

/// Monotonic u64 allocator for `object_id`s. Single-row counter, PK `0` — the
/// same one-row read-modify-write as [`crate::sequence`], but u64 and never
/// wrapping in practice. `0` is reserved as the "none" sentinel, so the first
/// allocation hands out `1`.
#[table(accessor = object_id_counter)]
pub struct ObjectIdCounter {
    #[primary_key]
    pub id: u8,
    pub next: u64,
}

/// Allocate a fresh `object_id` (≥ 1). Lazy-seeds on first call.
pub fn next_object_id(ctx: &ReducerContext) -> u64 {
    if let Some(counter) = ctx.db.object_id_counter().id().find(0) {
        let allocated = counter.next;
        ctx.db.object_id_counter().id().delete(0);
        ctx.db.object_id_counter().insert(ObjectIdCounter {
            id: 0,
            next: allocated.saturating_add(1),
        });
        allocated
    } else {
        // First allocation returns 1 (0 reserved as "none"); next hands out 2.
        ctx.db.object_id_counter().insert(ObjectIdCounter { id: 0, next: 2 });
        1
    }
}

// ── bitemporal primitives (keyed by `object_id`) ─────────────────────────────

/// The version of `object_id` current at `time_ms` (max `valid_at_time ≤ time_ms`).
pub fn prior_at(ctx: &ReducerContext, object_id: u64, time_ms: u64) -> Option<FreeThing> {
    ctx.db
        .free_things()
        .object_id()
        .filter(object_id)
        .filter(|r| valid_at_time(r.valid_at) <= time_ms)
        .max_by_key(|r| valid_at_time(r.valid_at))
}

/// The version of `object_id` current at wall-clock now.
pub fn latest(ctx: &ReducerContext, object_id: u64) -> Option<FreeThing> {
    prior_at(ctx, object_id, now_ms(ctx))
}

/// Delete every version of `object_id` at exactly `time_ms` (the same-time purge
/// [`write_at`] does — "last write at this (object, ms) wins").
fn delete_at(ctx: &ReducerContext, object_id: u64, time_ms: u64) {
    let pks: Vec<u64> = ctx
        .db
        .free_things()
        .object_id()
        .filter(object_id)
        .filter(|r| valid_at_time(r.valid_at) == time_ms)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.free_things().valid_at().delete(pk);
    }
}

/// Delete every version of `object_id` except the one at `keep_valid_at` (its PK).
/// Used to reset a mover to just its current row before committing a fresh path,
/// so completed-path history and any superseded future don't accumulate between
/// the 10-minute GC sweeps — which would bloat every new subscriber's initial
/// burst and the client's sort/interpolation work.
pub fn retain_only(ctx: &ReducerContext, object_id: u64, keep_valid_at: u64) {
    let pks: Vec<u64> = ctx
        .db
        .free_things()
        .object_id()
        .filter(object_id)
        .filter(|r| r.valid_at != keep_valid_at)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.free_things().valid_at().delete(pk);
    }
}

/// Delete every version of `object_id`, regardless of time — the remove path.
pub fn delete_all(ctx: &ReducerContext, object_id: u64) {
    let pks: Vec<u64> = ctx
        .db
        .free_things()
        .object_id()
        .filter(object_id)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.free_things().valid_at().delete(pk);
    }
}

/// Stamp `valid_at = (time_ms, fresh seq)` and write, purging any same-(object,
/// time) row first. The single write entry point; presence is the caller's job.
fn write_at(ctx: &ReducerContext, mut row: FreeThing, time_ms: u64) -> FreeThing {
    delete_at(ctx, row.object_id, time_ms);
    row.valid_at = pack_valid_at(time_ms, sequence::next_sequence(ctx));
    ctx.db.free_things().insert(row)
}

/// Reap every non-latest version, keeping the max-`valid_at` row per `object_id`.
pub fn reap_prior(ctx: &ReducerContext) {
    use std::collections::HashMap;
    let mut latest_by_obj: HashMap<u64, u64> = HashMap::new();
    for r in ctx.db.free_things().iter() {
        latest_by_obj
            .entry(r.object_id)
            .and_modify(|m| {
                if r.valid_at > *m {
                    *m = r.valid_at;
                }
            })
            .or_insert(r.valid_at);
    }
    let mut to_delete: Vec<u64> = Vec::new();
    for r in ctx.db.free_things().iter() {
        if latest_by_obj.get(&r.object_id) != Some(&r.valid_at) {
            to_delete.push(r.valid_at);
        }
    }
    for v in to_delete {
        ctx.db.free_things().valid_at().delete(v);
    }
}

// ── write surface (gate-facing reducers) ─────────────────────────────────────
//
// The gate is the authority; the shard trusts its args (same posture as
// zone_shard). Each write takes the gate-resolved `now_ms: u64`. Presence is
// maintained transactionally with the row, so a crash rolls back both together.

/// Shared insert/move core: write a new version of `object_id`, adjusting
/// `presence` for a create (no prior) or a region change. Rejects an out-of-range
/// `id` (must fit the u12 thing-def width so it can settle back into a zone slot).
/// `pub(crate)` so the transfer protocol ([`crate::transfer`]) can land a received
/// thing through the same presence-maintaining path.
pub(crate) fn place(
    ctx: &ReducerContext,
    object_id: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
    time_ms: u64,
) -> Result<(), String> {
    if id > DEF_ID_MAX {
        return Err(format!(
            "object_shard: def id {id} exceeds u12 max {DEF_ID_MAX}"
        ));
    }
    let new_region = region_of(zone_id);
    match latest(ctx, object_id) {
        None => presence::inc(ctx, new_region),
        Some(prev) => {
            let old_region = region_of(prev.zone_id);
            if old_region != new_region {
                presence::dec(ctx, old_region);
                presence::inc(ctx, new_region);
            }
        }
    }
    write_at(
        ctx,
        FreeThing {
            valid_at: 0,
            object_id,
            zone_id,
            location,
            rotation,
            id,
            offset,
        },
        time_ms,
    );
    Ok(())
}

/// Spawn a brand-new loose thing — allocates a fresh `object_id` and inserts its
/// first version. The gate learns the allocated id by observing the inserted row
/// on its subscription (reducers can't return it).
#[reducer]
pub fn create_free_thing(
    ctx: &ReducerContext,
    now_ms: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
) -> Result<(), String> {
    let object_id = next_object_id(ctx);
    place(ctx, object_id, zone_id, location, rotation, id, offset, now_ms)
}

/// Idempotent upsert of a loose thing at an **explicit** `object_id` — the
/// bulk-load / seed path ("populated like a zone shard"), and the future
/// settle-transfer receive entry point. Inserts if new, moves if it exists.
#[reducer]
pub fn place_free_thing(
    ctx: &ReducerContext,
    now_ms: u64,
    object_id: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
) -> Result<(), String> {
    if object_id == 0 {
        return Err("object_shard: object_id 0 is the reserved \"none\" sentinel".to_string());
    }
    place(ctx, object_id, zone_id, location, rotation, id, offset, now_ms)
}

/// Move an existing loose thing — new zone/cell/rotation/offset, preserving its
/// `id` (what-kind). Crossing a region boundary is just this update; presence
/// shifts from the old region to the new one. Errors if the object is unknown.
#[reducer]
pub fn move_free_thing(
    ctx: &ReducerContext,
    now_ms: u64,
    object_id: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    offset: u8,
) -> Result<(), String> {
    let prev = latest(ctx, object_id)
        .ok_or_else(|| format!("object_shard: move of unknown object {object_id}"))?;
    place(ctx, object_id, zone_id, location, rotation, prev.id, offset, now_ms)
}

/// Remove a loose thing entirely (drops every version), decrementing presence.
/// Idempotent — removing an absent object is a no-op.
#[reducer]
pub fn remove_free_thing(ctx: &ReducerContext, object_id: u64) -> Result<(), String> {
    if let Some(prev) = latest(ctx, object_id) {
        presence::dec(ctx, region_of(prev.zone_id));
        delete_all(ctx, object_id);
    }
    Ok(())
}
