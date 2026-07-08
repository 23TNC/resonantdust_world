//! `pawns` — one row per pawn (a villager, a wolf, …).
//!
//! A pawn is an object-shard citizen, like a [free thing](crate::free_things):
//! region-free, carrying its own position (`zone_id` + `location` + sub-tile
//! `offset`), so moving — even across a region boundary — is a plain row update,
//! never a cross-shard transfer. It's the forward-looking reason object shards
//! exist (docs/object-shard.md).
//!
//! A pawn carries **more** than a free thing:
//!   * `owner_id` — the player (`players::player_id`) that owns it. An owner drives
//!     its pawns through the client/npc command path, exactly as it drives itself.
//!   * `data` — an opaque `Vec<u64>` of per-pawn state slots (health, hunger, job,
//!     …), interpreted by the content/DSL. A blob rather than named columns so
//!     pawn stats grow without a schema migration.
//! plus the free-thing columns it shares (`object_id`, `zone_id`, `location`,
//! `rotation`, `id`, `offset`).
//!
//! Bitemporal like the rest of the game: `valid_at` PK, history per `object_id`,
//! reaped to the latest. Pawns share the `object_id` allocator with free things,
//! so an id resolves to exactly one object, and [`presence`](crate::presence)
//! counts both. There is no hot→cold fold and no settle transfer — a pawn is born
//! free and stays free.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::packed::{pack_valid_at, region_of, valid_at_time, DEF_ID_MAX};

use crate::free_things::next_object_id;
use crate::presence;
use crate::sequence;
use crate::time::now_ms;

/// One pawn version. Free-thing columns + `owner_id` + a `data` blob.
#[table(accessor = pawns, public)]
pub struct Pawn {
    /// Bitemporal version stamp: `(time_ms u48 << 16) | seq`. Rows for one
    /// `object_id` form a history; the latest is the current state.
    #[primary_key]
    pub valid_at: u64,
    /// Stable instance handle — allocated once ([`next_object_id`], shared with
    /// free things) and kept across every move and version.
    #[index(btree)]
    pub object_id: u64,
    /// The player that owns this pawn (`players::player_id`); `0` = unowned/world.
    #[index(btree)]
    pub owner_id: u32,
    /// Zone the pawn is currently in. Changes freely as it moves (even across a
    /// region boundary — a plain update, not a transfer).
    #[index(btree)]
    pub zone_id: u32,
    /// Tile cell within the zone (`y << 4 | x`, `0..256`) — the coarse position.
    pub location: u8,
    /// Rotation / facing. Stored raw.
    pub rotation: u8,
    /// What-kind def id (the pawn's species/appearance — villager, wolf, …), in
    /// the u12 thing-def namespace.
    pub id: u16,
    /// Sub-tile position within the cell (`x_off:4 | y_off:4`, 1/16-tile steps).
    pub offset: u8,
    /// Opaque per-pawn state slots (health, hunger, current job, …), interpreted
    /// by the content/DSL. A blob so pawn stats can grow without a schema change.
    pub data: Vec<u64>,
}

// ── bitemporal primitives (keyed by `object_id`) ─────────────────────────────

/// The version of `object_id` current at `time_ms` (max `valid_at_time ≤ time_ms`).
pub fn prior_at(ctx: &ReducerContext, object_id: u64, time_ms: u64) -> Option<Pawn> {
    ctx.db
        .pawns()
        .object_id()
        .filter(object_id)
        .filter(|r| valid_at_time(r.valid_at) <= time_ms)
        .max_by_key(|r| valid_at_time(r.valid_at))
}

/// The version of `object_id` current at wall-clock now.
pub fn latest(ctx: &ReducerContext, object_id: u64) -> Option<Pawn> {
    prior_at(ctx, object_id, now_ms(ctx))
}

/// Delete every version of `object_id` at exactly `time_ms` (same-(object, time)
/// purge [`write_at`] does — "last write at this (object, ms) wins").
fn delete_at(ctx: &ReducerContext, object_id: u64, time_ms: u64) {
    let pks: Vec<u64> = ctx
        .db
        .pawns()
        .object_id()
        .filter(object_id)
        .filter(|r| valid_at_time(r.valid_at) == time_ms)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.pawns().valid_at().delete(pk);
    }
}

/// Delete every version of `object_id`, regardless of time — the remove path.
pub fn delete_all(ctx: &ReducerContext, object_id: u64) {
    let pks: Vec<u64> = ctx
        .db
        .pawns()
        .object_id()
        .filter(object_id)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.pawns().valid_at().delete(pk);
    }
}

/// Delete every version of `object_id` except the one at `keep_valid_at` (its PK)
/// — reset a pawn to just its current row before committing a fresh motion path,
/// so completed-path history + superseded future don't accumulate between GC
/// sweeps. Mirrors [`free_things::retain_only`](crate::free_things::retain_only).
pub fn retain_only(ctx: &ReducerContext, object_id: u64, keep_valid_at: u64) {
    let pks: Vec<u64> = ctx
        .db
        .pawns()
        .object_id()
        .filter(object_id)
        .filter(|r| r.valid_at != keep_valid_at)
        .map(|r| r.valid_at)
        .collect();
    for pk in pks {
        ctx.db.pawns().valid_at().delete(pk);
    }
}

/// Stamp `valid_at = (time_ms, fresh seq)` and write, purging any same-(object,
/// time) row first. The single write entry point; presence is the caller's job.
fn write_at(ctx: &ReducerContext, mut row: Pawn, time_ms: u64) -> Pawn {
    delete_at(ctx, row.object_id, time_ms);
    row.valid_at = pack_valid_at(time_ms, sequence::next_sequence(ctx));
    ctx.db.pawns().insert(row)
}

/// Reap every non-latest version, keeping the max-`valid_at` row per `object_id`.
pub fn reap_prior(ctx: &ReducerContext) {
    use std::collections::HashMap;
    let mut latest_by_obj: HashMap<u64, u64> = HashMap::new();
    for r in ctx.db.pawns().iter() {
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
    for r in ctx.db.pawns().iter() {
        if latest_by_obj.get(&r.object_id) != Some(&r.valid_at) {
            to_delete.push(r.valid_at);
        }
    }
    for v in to_delete {
        ctx.db.pawns().valid_at().delete(v);
    }
}

/// Shared insert/move core: write a new version of `object_id`, adjusting
/// `presence` for a create (no prior) or a region change. Rejects an out-of-range
/// `id` (u12 thing-def width). `pub(crate)` so a future carry/attach path can land
/// a pawn through the same presence-maintaining route.
#[allow(clippy::too_many_arguments)]
pub(crate) fn place(
    ctx: &ReducerContext,
    object_id: u64,
    owner_id: u32,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
    data: Vec<u64>,
    time_ms: u64,
) -> Result<(), String> {
    if id > DEF_ID_MAX {
        return Err(format!("object_shard: pawn def id {id} exceeds u12 max {DEF_ID_MAX}"));
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
        Pawn {
            valid_at: 0,
            object_id,
            owner_id,
            zone_id,
            location,
            rotation,
            id,
            offset,
            data,
        },
        time_ms,
    );
    Ok(())
}

// ── write surface (gate-facing reducers) ─────────────────────────────────────
//
// The gate is the authority; the shard trusts its args. Each write takes the
// gate-resolved `now_ms: u64`. Presence is maintained transactionally with the
// row (same posture as free_things).

/// Spawn a brand-new pawn owned by `owner_id` — allocates a fresh `object_id`
/// (shared allocator) and inserts its first version. The gate learns the id by
/// observing the inserted row on its subscription.
#[allow(clippy::too_many_arguments)]
#[reducer]
pub fn spawn_pawn(
    ctx: &ReducerContext,
    now_ms: u64,
    owner_id: u32,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
    data: Vec<u64>,
) -> Result<(), String> {
    let object_id = next_object_id(ctx);
    place(ctx, object_id, owner_id, zone_id, location, rotation, id, offset, data, now_ms)
}

/// Idempotent upsert of a pawn at an **explicit** `object_id` — the bulk-load /
/// seed path. Inserts if new, moves if it exists.
#[allow(clippy::too_many_arguments)]
#[reducer]
pub fn place_pawn(
    ctx: &ReducerContext,
    now_ms: u64,
    object_id: u64,
    owner_id: u32,
    zone_id: u32,
    location: u8,
    rotation: u8,
    id: u16,
    offset: u8,
    data: Vec<u64>,
) -> Result<(), String> {
    if object_id == 0 {
        return Err("object_shard: object_id 0 is the reserved \"none\" sentinel".to_string());
    }
    place(ctx, object_id, owner_id, zone_id, location, rotation, id, offset, data, now_ms)
}

/// Move an existing pawn — new zone/cell/rotation/offset, preserving its
/// `owner_id`, `id`, and `data`. Crossing a region boundary is just this update.
/// Errors if the pawn is unknown.
#[reducer]
pub fn move_pawn(
    ctx: &ReducerContext,
    now_ms: u64,
    object_id: u64,
    zone_id: u32,
    location: u8,
    rotation: u8,
    offset: u8,
) -> Result<(), String> {
    let prev = latest(ctx, object_id)
        .ok_or_else(|| format!("object_shard: move of unknown pawn {object_id}"))?;
    place(
        ctx, object_id, prev.owner_id, zone_id, location, rotation, prev.id, offset, prev.data,
        now_ms,
    )
}

/// Update a pawn's `data` blob (its stats/state), preserving its current position,
/// owner, and kind. Writes a new version at `now_ms`. Errors if unknown.
#[reducer]
pub fn set_pawn_data(
    ctx: &ReducerContext,
    now_ms: u64,
    object_id: u64,
    data: Vec<u64>,
) -> Result<(), String> {
    let prev = latest(ctx, object_id)
        .ok_or_else(|| format!("object_shard: set_data on unknown pawn {object_id}"))?;
    place(
        ctx,
        object_id,
        prev.owner_id,
        prev.zone_id,
        prev.location,
        prev.rotation,
        prev.id,
        prev.offset,
        data,
        now_ms,
    )
}

/// Remove a pawn entirely (drops every version), decrementing presence.
/// Idempotent — removing an absent pawn is a no-op.
#[reducer]
pub fn remove_pawn(ctx: &ReducerContext, object_id: u64) -> Result<(), String> {
    if let Some(prev) = latest(ctx, object_id) {
        presence::dec(ctx, region_of(prev.zone_id));
        delete_all(ctx, object_id);
    }
    Ok(())
}
