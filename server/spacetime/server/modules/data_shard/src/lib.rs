//! data_shard — the composition slots (`state_log`) and the client-visible latest (`state`).
//! Shapes: `docs/TABLES.md`. Flow: `docs/intent/spacetime-again/`. The orchestrator calls `claim`,
//! the worker calls `write`, the master calls `bump` / `gc`. This module and `event_shard` never
//! call each other.

use spacetimedb::{reducer, table, ReducerContext, SpacetimeType, Table};

use resonantdust_codec::object::position_macro;
use resonantdust_codec::refs::SERVER_REF_NONE;
use resonantdust_codec::status::{pack_status, status_has_flag, STATE_FLAG_PROMOTE, STATE_OPEN, STATE_PROMOTED};
use resonantdust_codec::tic::{tic_after, tic_before};
use resonantdust_codec::uid::pack_state_uid;

// ── the tic clock (same shape as every shard; the master bumps it in lockstep) ─────

#[table(accessor = clock, public)]
pub struct Clock {
    #[primary_key]
    pub id: u8,
    pub master_tic: u16,
}

#[reducer]
pub fn bump(ctx: &ReducerContext, master_tic: u16) -> Result<(), String> {
    ctx.db.clock().id().delete(0);
    ctx.db.clock().insert(Clock { id: 0, master_tic });
    Ok(())
}

#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    if ctx.db.clock().id().find(0).is_none() {
        ctx.db.clock().insert(Clock { id: 0, master_tic: 0 });
    }
}

// ── the payload — the reference model's three orthogonal references ─────────────────

/// One target's composed value, passed to [`write`]. `entity_reference` selects the slot.
#[derive(SpacetimeType, Clone)]
pub struct TargetState {
    pub entity_reference: u32,
    pub definition_reference: u32,
    pub position_reference: u32,
    pub data: u8,
    /// If set, promote this target to `state` once settled (the program ran `PROMOTE_STATE`).
    pub promote: bool,
}

// ── state_log — per (entity, tic) composition slot ─────────────────────────────────

#[table(accessor = state_log, public)]
pub struct StateLog {
    /// `state_uid` = `reserved:16 | entity_reference:32 | tic:16`. Entity-major.
    #[primary_key]
    pub uid: u64,
    #[index(btree)]
    pub entity_reference: u32,
    #[index(btree)]
    pub tic: u16,
    /// The worker that writes this row (its component's owner). `SERVER_REF_NONE` = none.
    #[index(btree)]
    pub worker_reference: u8,
    /// The worker that reads this row as the base for its next-tic work. No lease — a re-`claim`
    /// overwrites it, which is the eviction.
    #[index(btree)]
    pub observer_reference: u8,
    /// `true` = work pending; `false` = settled (one worker per component → binary, not a count).
    pub dirty: bool,
    pub definition_reference: u32,
    pub position_reference: u32,
    pub data: u8,
    /// `state_status` — `flags:4 | status:4` (`PROMOTE` / `PROMOTED`).
    pub status: u8,
}

// ── state — client-visible latest ──────────────────────────────────────────────────

#[table(accessor = state, public)]
pub struct State {
    #[primary_key]
    pub entity_reference: u32,
    /// The zone-subscription key — the payload's `position_reference` high half, kept as its own
    /// column because a subscription filters on columns, not expressions.
    #[index(btree)]
    pub macro_position_reference: u16,
    pub tic: u16,
    pub definition_reference: u32,
    pub position_reference: u32,
    pub data: u8,
}

// ── claim — the orchestrator stands up a component's slots ──────────────────────────

/// For each entity: create its `(E, tic)` slot (`dirty`), stamp `worker_reference`; find E's
/// most-recent row `< tic` and stamp `observer_reference` on it (the base the worker reads). No
/// lease — worker liveness is the orchestrator's, and a re-`claim` overwrites the stamp.
#[reducer]
pub fn claim(ctx: &ReducerContext, entities: Vec<u32>, tic: u16, worker: u8) -> Result<(), String> {
    for e in entities {
        let uid = pack_state_uid(e, tic);
        // Stamp the observer on the immediate-previous existing row (serially latest tic < this).
        if let Some(prev) = previous_row(ctx, e, tic) {
            let mut p = prev;
            p.observer_reference = worker;
            ctx.db.state_log().uid().update(p);
        }
        // Create-or-restamp this slot.
        match ctx.db.state_log().uid().find(uid) {
            Some(mut row) => {
                row.worker_reference = worker;
                row.dirty = true;
                ctx.db.state_log().uid().update(row);
            }
            None => {
                ctx.db.state_log().insert(StateLog {
                    uid,
                    entity_reference: e,
                    tic,
                    worker_reference: worker,
                    observer_reference: SERVER_REF_NONE,
                    dirty: true,
                    definition_reference: 0,
                    position_reference: 0,
                    data: 0,
                    status: pack_status(0, STATE_OPEN),
                });
            }
        }
    }
    Ok(())
}

/// The serially-latest existing `state_log` row for `entity` strictly before `tic`, if any.
fn previous_row(ctx: &ReducerContext, entity: u32, tic: u16) -> Option<StateLog> {
    ctx.db
        .state_log()
        .entity_reference()
        .filter(entity)
        .filter(|r| tic_before(r.tic, tic))
        .reduce(|a, b| if tic_after(b.tic, a.tic) { b } else { a })
}

// ── write — the worker commits absolute finals ─────────────────────────────────────

/// Write each target's **absolute** composed value for `tic`. Idempotent: a replay recomputes the
/// same value from the immutable base, and an already-`!dirty` row is skipped. Only the assigned
/// worker may write (the fence). A `promote` target lands in `state` once, on settle.
///
/// `worker` is passed and checked against `worker_reference` — a trusted-server fence. Mapping the
/// SpacetimeDB caller identity to a `server_reference` is a later hardening.
#[reducer]
pub fn write(ctx: &ReducerContext, worker: u8, tic: u16, results: Vec<TargetState>) -> Result<(), String> {
    for r in results {
        let uid = pack_state_uid(r.entity_reference, tic);
        let mut row = ctx
            .db
            .state_log()
            .uid()
            .find(uid)
            .ok_or_else(|| format!("no slot for entity {:#010x} tic {tic}", r.entity_reference))?;
        if row.worker_reference != worker {
            return Err(format!("worker {worker} is not assigned slot {uid:#018x}"));
        }
        if !row.dirty {
            continue; // already written — a replay
        }
        row.definition_reference = r.definition_reference;
        row.position_reference = r.position_reference;
        row.data = r.data;
        row.dirty = false;
        if r.promote {
            row.status = pack_status(STATE_FLAG_PROMOTE, STATE_OPEN);
        }
        let status = row.status;
        ctx.db.state_log().uid().update(row);

        // Opt-in promotion: the program asked, and the slot is now settled.
        if status_has_flag(status, STATE_FLAG_PROMOTE)
            && resonantdust_codec::status::status_phase(status) != STATE_PROMOTED
        {
            upsert_state(ctx, &r, tic);
            let mut promoted = ctx.db.state_log().uid().find(uid).expect("just wrote");
            promoted.status = pack_status(STATE_FLAG_PROMOTE, STATE_PROMOTED);
            ctx.db.state_log().uid().update(promoted);
        }
    }
    Ok(())
}

fn upsert_state(ctx: &ReducerContext, r: &TargetState, tic: u16) {
    let macro_position = position_macro(r.position_reference);
    let row = State {
        entity_reference: r.entity_reference,
        macro_position_reference: macro_position,
        tic,
        definition_reference: r.definition_reference,
        position_reference: r.position_reference,
        data: r.data,
    };
    if ctx.db.state().entity_reference().find(r.entity_reference).is_some() {
        ctx.db.state().entity_reference().update(row);
    } else {
        ctx.db.state().insert(row);
    }
}

// ── gc — the master drops old settled rows (never the latest per entity) ────────────

/// Delete `!dirty` rows that are **not** the latest for their entity and older than `horizon`. The
/// latest per entity is never reaped — it is every future tic's base.
#[reducer]
pub fn gc(ctx: &ReducerContext, horizon: u16) -> Result<(), String> {
    // Latest tic per entity (serial max), so we never drop a base.
    use std::collections::HashMap;
    let mut latest: HashMap<u32, u16> = HashMap::new();
    for row in ctx.db.state_log().iter() {
        latest
            .entry(row.entity_reference)
            .and_modify(|t| {
                if tic_after(row.tic, *t) {
                    *t = row.tic;
                }
            })
            .or_insert(row.tic);
    }
    let doomed: Vec<u64> = ctx
        .db
        .state_log()
        .iter()
        .filter(|r| {
            !r.dirty
                && latest.get(&r.entity_reference) != Some(&r.tic)
                && tic_before(r.tic, horizon)
        })
        .map(|r| r.uid)
        .collect();
    for uid in doomed {
        ctx.db.state_log().uid().delete(uid);
    }
    Ok(())
}
