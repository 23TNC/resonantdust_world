//! player_pawn — the players' row-carrier shard: player-pawns (`TYPE_PLAYER`), one row per
//! entity (work `2026-08-08-player-pawns` F1 — PAWN'S STAMP, re-instantiated).
//!
//! Players play AS their player-pawn (exactly one per player today, minted at login; every
//! npc module plays as one too), so this module is deliberately a COPY of `pawn`: the same
//! shared [`resonantdust_codec::entity_tables!`] machinery (`clock` / `entity_state_log` /
//! `entity_state` + `init` / `bump` / `claim` / `write` / `gc`), the same `spawn_log` mint
//! machinery, the same `payload` sidecar and `needs` / `inventory` sub-tables — every pawn
//! method works unchanged on a player-pawn. What differs is ONLY:
//!
//! - the minted reference's server byte — `TYPE_PLAYER` (`0x40 | server_id`), the nibble the
//!   whole event system routes by (VARIABLES.md §Player-pawns);
//! - the fan — rows go to the OWNING session, not zone subscribers (F6; the zone keys are
//!   maintained by the stamp but v1 player-pawns never move, so they are inert);
//! - the lanes — bands/conditions/emotions evaluate as on pawns; the DEATH lane stays off
//!   BY CONSTRUCTION (player defs author no `can_die`), not by any switch here (I8).
//!
//! Shapes: `docs/TABLES.md` §player_pawn. The linkage (player ↔ player-pawn + ACTIVE) lives
//! in the `players` auth DB, never here.

use spacetimedb::{ReducerContext, Table};

use resonantdust_codec::object::{position_macro, position_micro, TYPE_PLAYER};
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference, SERVER_REF_NONE};
use resonantdust_codec::status::{pack_status, STATE_FLAG_PROMOTE, STATE_OPEN, STATE_PROMOTED};
use resonantdust_codec::uid::pack_state_uid;

resonantdust_codec::entity_tables!(state_hook: payload_follow_state, data: u8);

// ── payload — the growable per-entity opcode stream, SLAVED to state ────────────────────────
//
// Identical to pawn's (human-pawns P0/F5): an open-ended sidecar pair the state claim locks;
// every write rides a state-write transaction. v1 player-pawns carry TRAIT entries (their def's
// non-constant binds) and condition entries; PART entries are meaningless until the future
// character/avatar arc renders one.

/// The write-history sidecar of `entity_state_log` — one row per payload-carrying state write.
#[spacetimedb::table(accessor = payload_log, public)]
pub struct PayloadLog {
    /// `state_uid` = `reserved:16 | entity_reference:32 | tic:16` — the state write it rode.
    #[primary_key]
    pub uid: u64,
    #[index(btree)]
    pub entity_reference: u32,
    pub tic: u16,
    pub payload: Vec<u32>,
}

/// The composed, client-visible sidecar of `entity_state` — the entity's CURRENT payload.
#[spacetimedb::table(accessor = payload, public)]
pub struct Payload {
    #[primary_key]
    pub entity_reference: u32,
    /// The zone-subscription key — slaved by the stamp; inert while player-pawns never move.
    #[index(btree)]
    pub macro_position_reference: u16,
    /// The tic of the last payload CONTENT change.
    pub tic: u16,
    pub payload: Vec<u32>,
}

/// The `entity_tables!` state hook: every `entity_state` upsert drags the entity's `payload`,
/// `needs`, and `inventory` rows into the entity's zone, in the SAME transaction — pawn's hook
/// verbatim. v1 player-pawns never hop zones, so this is exercised only by the future avatar arc.
fn payload_follow_state(ctx: &ReducerContext, r: &TargetState, _tic: u16) {
    if let Some(mut p) = ctx.db.payload().entity_reference().find(r.entity_reference) {
        if p.macro_position_reference != r.macro_position_reference {
            p.macro_position_reference = r.macro_position_reference;
            ctx.db.payload().entity_reference().update(p);
        }
    }
    let stale: Vec<Needs> = ctx
        .db
        .needs()
        .entity_reference()
        .filter(r.entity_reference)
        .filter(|n| n.macro_position_reference != r.macro_position_reference)
        .collect();
    for mut n in stale {
        n.macro_position_reference = r.macro_position_reference;
        ctx.db.needs().uid().update(n);
    }
    let stale_inv: Vec<Inventory> = ctx
        .db
        .inventory()
        .entity_reference()
        .filter(r.entity_reference)
        .filter(|i| i.macro_position_reference != r.macro_position_reference)
        .collect();
    for mut i in stale_inv {
        i.macro_position_reference = r.macro_position_reference;
        ctx.db.inventory().uid().update(i);
    }
}

// ── needs — the per-(entity, need) row table (stat-model F2, pawn's shape) ──────────────

/// One need row: `uid = entity_reference:32 | need_key:16` (key = the packed row's low 16).
#[spacetimedb::table(accessor = needs, public)]
pub struct Needs {
    #[primary_key]
    pub uid: u64,
    #[index(btree)]
    pub entity_reference: u32,
    /// The zone-subscription key — slaved to the entity's zone (the state hook).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// The packed gameplay row: `value:16 | kind:12 | variant:4`.
    pub need: u32,
    pub set_tic: u16,
}

/// Upsert one need row (the `SET_NEED` body + the spawn mint path).
fn upsert_need_row(ctx: &ReducerContext, tic: u16, entity: u32, row: u32, zone: u16) {
    let uid = ((entity as u64) << 16) | (row as u64 & 0xFFFF);
    let r = Needs {
        uid,
        entity_reference: entity,
        macro_position_reference: zone,
        need: row,
        set_tic: tic,
    };
    if ctx.db.needs().uid().find(uid).is_some() {
        ctx.db.needs().uid().update(r);
    } else {
        ctx.db.needs().insert(r);
    }
}

/// The entity's current zone key — its promoted state row, else its payload row, else 0.
fn entity_zone(ctx: &ReducerContext, entity: u32) -> u16 {
    ctx.db
        .entity_state()
        .entity_reference()
        .find(entity)
        .map(|s| s.macro_position_reference)
        .or_else(|| {
            ctx.db.payload().entity_reference().find(entity).map(|p| p.macro_position_reference)
        })
        .unwrap_or(0)
}

// ── inventory — the per-(entity, slot) item table (inventory F2, pawn's shape) ────────────
//
// Rides the stamp UNUSED until something wants player storage (TABLES.md §player_pawn).

/// One held item: `uid = entity_reference:32 | slot:8`.
#[spacetimedb::table(accessor = inventory, public)]
pub struct Inventory {
    #[primary_key]
    pub uid: u64,
    #[index(btree)]
    pub entity_reference: u32,
    /// The zone-subscription key — slaved to the entity's zone (the state hook).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// 0-based; `inv_add` fills the first free slot.
    pub slot: u8,
    /// The held thing's `definition_reference`.
    pub item: u32,
    /// RESERVED (0 today) — the item-as-entity successor parks the entity id here.
    pub state: u32,
}

/// The encoding-domain slot bound (VARIABLES.md: the need's 0..16 domain).
const INVENTORY_SLOT_BOUND: u8 = 16;

/// `INV_ADD` — insert the item at the FIRST FREE slot. Pawn's reducer verbatim.
#[spacetimedb::reducer]
pub fn inv_add(
    ctx: &ReducerContext,
    _worker: u8,
    _tic: u16,
    entity_reference: u32,
    item: u32,
) -> Result<(), String> {
    let taken: Vec<u8> =
        ctx.db.inventory().entity_reference().filter(entity_reference).map(|r| r.slot).collect();
    let Some(slot) = (0..INVENTORY_SLOT_BOUND).find(|s| !taken.contains(s)) else {
        return Err(format!("inv_add: no free slot on {entity_reference:#010x}"));
    };
    let zone = entity_zone(ctx, entity_reference);
    ctx.db.inventory().insert(Inventory {
        uid: ((entity_reference as u64) << 8) | slot as u64,
        entity_reference,
        macro_position_reference: zone,
        slot,
        item,
        state: 0,
    });
    Ok(())
}

/// `INV_REMOVE` — delete one slot's row. Silent on absence.
#[spacetimedb::reducer]
pub fn inv_remove(
    ctx: &ReducerContext,
    _worker: u8,
    _tic: u16,
    entity_reference: u32,
    slot: u8,
) -> Result<(), String> {
    let uid = ((entity_reference as u64) << 8) | slot as u64;
    if ctx.db.inventory().uid().find(uid).is_some() {
        ctx.db.inventory().uid().delete(uid);
    }
    Ok(())
}

// ── needs & conditions — payload-entry verbs (needs-moodlets F7, pawn's shape) ────────────

/// Shared splice: apply `edit` to the CURRENT payload, then write the log row + the
/// zone-keyed projection. Pawn's helper verbatim.
fn write_payload_entry(ctx: &ReducerContext, tic: u16, entity: u32, edit: impl FnOnce(&mut Vec<u32>)) {
    let existing = ctx.db.payload().entity_reference().find(entity);
    let mut words = existing.as_ref().map(|p| p.payload.clone()).unwrap_or_default();
    edit(&mut words);
    let zone = ctx
        .db
        .entity_state()
        .entity_reference()
        .find(entity)
        .map(|s| s.macro_position_reference)
        .or(existing.as_ref().map(|p| p.macro_position_reference))
        .unwrap_or(0);
    let uid = pack_state_uid(entity, tic);
    let log = PayloadLog { uid, entity_reference: entity, tic, payload: words.clone() };
    if ctx.db.payload_log().uid().find(uid).is_some() {
        ctx.db.payload_log().uid().update(log);
    } else {
        ctx.db.payload_log().insert(log);
    }
    let row = Payload { entity_reference: entity, macro_position_reference: zone, tic, payload: words };
    if existing.is_some() {
        ctx.db.payload().entity_reference().update(row);
    } else {
        ctx.db.payload().insert(row);
    }
}

/// `SET_NEED` — upsert one `needs` sub-table row (stat-model F2/F4). A zeroed player need
/// bands and conditions; it NEVER removes the carrier — the death lane is off by construction
/// (the sweep lives in the worker and player defs author no `can_die`).
#[spacetimedb::reducer]
pub fn set_need(
    ctx: &ReducerContext,
    _worker: u8,
    tic: u16,
    entity_reference: u32,
    row: u32,
) -> Result<(), String> {
    let zone = entity_zone(ctx, entity_reference);
    upsert_need_row(ctx, tic, entity_reference, row, zone);
    Ok(())
}

/// `GRANT_CONDITION` — upsert one stored (timed) condition row. Pawn's reducer verbatim.
#[spacetimedb::reducer]
pub fn grant_condition(
    ctx: &ReducerContext,
    _worker: u8,
    tic: u16,
    entity_reference: u32,
    row: u32,
) -> Result<(), String> {
    write_payload_entry(ctx, tic, entity_reference, |p| {
        resonantdust_codec::payload::upsert_condition(p, row, tic);
    });
    Ok(())
}

/// `remove` — delete the entity's CURRENT rows (state, payload, needs, inventory), logs stay.
/// Kept for parity with pawn (the stamp's lifecycle is complete), but NOTHING calls it in v1:
/// a player-pawn lives as long as its player's account (I8 — no death lane reaches here).
#[spacetimedb::reducer]
pub fn remove(ctx: &ReducerContext, _worker: u8, _tic: u16, entity_reference: u32) -> Result<(), String> {
    if let Some(row) = ctx.db.entity_state().entity_reference().find(entity_reference) {
        ctx.db.entity_state().entity_reference().delete(row.entity_reference);
    }
    if let Some(row) = ctx.db.payload().entity_reference().find(entity_reference) {
        ctx.db.payload().entity_reference().delete(row.entity_reference);
    }
    let uids: Vec<u64> = ctx
        .db
        .needs()
        .entity_reference()
        .filter(entity_reference)
        .map(|n| n.uid)
        .collect();
    for uid in uids {
        ctx.db.needs().uid().delete(uid);
    }
    let inv_uids: Vec<u64> = ctx
        .db
        .inventory()
        .entity_reference()
        .filter(entity_reference)
        .map(|i| i.uid)
        .collect();
    for uid in inv_uids {
        ctx.db.inventory().uid().delete(uid);
    }
    Ok(())
}

// ── spawn — server-minted player-pawn ids ───────────────────────────────────────────────

/// The replay ledger: `(event_reference, index) → minted entity_reference`. Pawn's shape.
#[spacetimedb::table(accessor = spawn_log, public)]
pub struct SpawnLog {
    /// `reserved:16 | event_reference:32 | index:16`.
    #[primary_key]
    pub spawn_uid: u64,
    #[index(btree)]
    pub event_reference: u32,
    /// Which spawn within the event's program (composition order).
    pub index: u16,
    /// The minted player-pawn id.
    pub entity_reference: u32,
}

/// The mint counter. Same `SPAWN_BASE` posture as pawn's — the TOP half of the 24-bit object
/// space (there is no legacy client-minted band HERE, but symmetry keeps the two shards'
/// object spaces reading identically).
#[spacetimedb::table(accessor = spawn_counter)]
pub struct SpawnCounter {
    #[primary_key]
    pub id: u8,
    pub next: u32,
}

const OBJECT_REF_MASK: u32 = 0x00FF_FFFF;
const SPAWN_BASE: u32 = 0x0080_0000;

/// Mint the next `player_pawn_reference`: `TYPE_PLAYER` server byte (`0x40`), counter object.
fn next_player_pawn_reference(ctx: &ReducerContext) -> u32 {
    let n = ctx.db.spawn_counter().id().find(0).map(|c| c.next).unwrap_or(SPAWN_BASE);
    ctx.db.spawn_counter().id().delete(0);
    ctx.db.spawn_counter().insert(SpawnCounter { id: 0, next: n.wrapping_add(1) & OBJECT_REF_MASK });
    pack_entity_reference(pack_server_reference(TYPE_PLAYER, 0), n & OBJECT_REF_MASK)
}

/// Mint + first write + promote — AND the sidecars — in one transaction, idempotent by
/// `(event_reference, index)`. Pawn's spawn verbatim; the caller composes `payload` (TRAIT
/// entries — a player-pawn's def binds) and `needs` (packed full-value rows) from the corpus,
/// because this module holds none. For the login mint the "event" is the login funnel's
/// dedup key, not a world event — the ledger semantics are identical.
#[spacetimedb::reducer]
pub fn spawn(
    ctx: &ReducerContext,
    worker: u8,
    tic: u16,
    event_reference: u32,
    index: u16,
    definition_reference: u32,
    position_reference: u32,
    payload: Vec<u32>,
    needs: Vec<u32>,
    data: u8,
    promote: bool,
) -> Result<(), String> {
    let spawn_uid = ((event_reference as u64) << 16) | index as u64;
    if ctx.db.spawn_log().spawn_uid().find(spawn_uid).is_some() {
        return Ok(()); // replay — already spawned
    }
    let entity = next_player_pawn_reference(ctx);
    ctx.db.spawn_log().insert(SpawnLog { spawn_uid, event_reference, index, entity_reference: entity });

    let status_flags = if promote { STATE_FLAG_PROMOTE } else { 0 };
    let phase = if promote { STATE_PROMOTED } else { STATE_OPEN };
    let r = TargetState {
        entity_reference: entity,
        definition_reference,
        macro_position_reference: position_macro(position_reference),
        micro_position_reference: position_micro(position_reference),
        data,
        promote,
    };
    ctx.db.entity_state_log().insert(EntityStateLog {
        uid: pack_state_uid(entity, tic),
        entity_reference: entity,
        tic,
        worker_reference: worker,
        observer_reference: SERVER_REF_NONE,
        dirty: false,
        definition_reference,
        macro_position_reference: r.macro_position_reference,
        micro_position_reference: r.micro_position_reference,
        data,
        status: pack_status(status_flags, phase),
    });
    if !payload.is_empty() {
        ctx.db.payload_log().insert(PayloadLog {
            uid: pack_state_uid(entity, tic),
            entity_reference: entity,
            tic,
            payload: payload.clone(),
        });
        ctx.db.payload().insert(Payload {
            entity_reference: entity,
            macro_position_reference: r.macro_position_reference,
            tic,
            payload,
        });
    }
    for row in needs {
        upsert_need_row(ctx, tic, entity, row, r.macro_position_reference);
    }
    if promote {
        entity_tables_upsert_state(ctx, &r, tic);
    }
    Ok(())
}
