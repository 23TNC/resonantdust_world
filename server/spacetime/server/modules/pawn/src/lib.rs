//! pawn — the hot mover shard: pawns (`TYPE_PAWN`), one row per entity.
//!
//! The core is the shared [`resonantdust_codec::entity_tables!`] machinery — the same stamp as
//! `data_shard` (which stays the hot catch-all; first-pawns F1): the `clock` /
//! `entity_state_log` / `entity_state` tables and the `init` / `bump` / `claim` / `write` / `gc`
//! reducers. The orchestrator routes `TYPE_PAWN` claims here, the worker composes + `write`s
//! here, the master `bump`s / `gc`s, the edge subscribes `entity_state` per zone.
//!
//! On top of the stamp: the **spawn machinery** for `CREATE` (`docs/ACTIONS.md` §CREATE,
//! first-pawns F4) — `spawn` mints a server-owned pawn id, records it in `spawn_log` keyed
//! `(event_reference, index)`, and writes the pawn's first `entity_state_log` row (+ the
//! `entity_state` promote) in ONE transaction, so a replayed event finds the log row and
//! never double-spawns. Shapes: `docs/TABLES.md`. Flow: `docs/intent/spacetime-again/`.

use spacetimedb::{ReducerContext, Table};

use resonantdust_codec::object::{position_macro, position_micro, TYPE_PAWN};
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference, SERVER_REF_NONE};
use resonantdust_codec::status::{pack_status, STATE_FLAG_PROMOTE, STATE_OPEN, STATE_PROMOTED};
use resonantdust_codec::uid::pack_state_uid;

resonantdust_codec::entity_tables!(state_hook: payload_follow_state, data: u8);

// ── payload — the growable per-pawn opcode stream, SLAVED to state (human-pawns P0/F5) ──────
//
// A pawn's open-ended state (parts, and later inventory/stats/needs/…) lives in a SIDECAR
// pair, not on the entity rows — so movement hops never copy it. Encoding: the opcode
// stream `resonantdust_codec::payload` owns (`opcode:16 | count:16` + operands; `PART = 1`);
// this module stores it opaquely and never decodes it. SLAVED: neither table is ever
// claimed — the entity's state claim is the lock, and every write here rides a state-write
// transaction (`spawn` now; equip verbs later). The `payload` row's zone key FOLLOWS its
// entity through the `state_hook` below, inside the same transaction as the state upsert.

/// The write-history sidecar of `entity_state_log` — one row per payload-carrying state
/// write (spawn / future equips; NOT movement). Same uid packing as the state log.
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

/// The composed, client-visible sidecar of `entity_state` — the pawn's CURRENT payload,
/// zone-keyed so the edge's per-zone subscription fans it with the state rows.
#[spacetimedb::table(accessor = payload, public)]
pub struct Payload {
    #[primary_key]
    pub entity_reference: u32,
    /// The zone-subscription key — slaved to the entity's `entity_state` zone (the hook).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// The tic of the last payload CONTENT change (a zone re-key keeps it).
    pub tic: u16,
    pub payload: Vec<u32>,
}

/// The `entity_tables!` state hook: every `entity_state` upsert drags the entity's `payload`
/// row into the entity's zone, in the SAME transaction — a zone-crossing hop is the one
/// movement that touches the sidecar, and only its key.
fn payload_follow_state(ctx: &ReducerContext, r: &TargetState, _tic: u16) {
    if let Some(mut p) = ctx.db.payload().entity_reference().find(r.entity_reference) {
        if p.macro_position_reference != r.macro_position_reference {
            p.macro_position_reference = r.macro_position_reference;
            ctx.db.payload().entity_reference().update(p);
        }
    }
}

// ── needs & conditions — payload-entry verbs (needs-moodlets F7) ──────────────────────
//
// `SET_NEED` / `GRANT_CONDITION` splice ONE entry into the pawn's payload sidecar. The
// MODULE composes (read current → upsert → write log + projection, one transaction), so
// the worker just relays the verb — no payload subscription joins its read set. The
// entity's claim serialises these with its movement writes (the verb's `obj` is a Write
// operand). Idempotent on replay: the same tic re-splices the same word, and the log row
// upserts by uid instead of double-inserting.

/// Shared splice: apply `edit` to the CURRENT payload, then write the log row + the
/// zone-keyed projection. The zone follows the entity's promoted state (the same key the
/// `state_hook` maintains on hops); a pawn with no state row keeps its existing payload
/// zone (or lands unkeyed 0 until promotion — the wolf is promoted at spawn).
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

/// `SET_NEED` — upsert one need's `(satisfaction, set_tic)` word (`set_tic` = this tic;
/// observers compute the current value from the corpus deplete rate, F4).
#[spacetimedb::reducer]
pub fn set_need(
    ctx: &ReducerContext,
    _worker: u8,
    tic: u16,
    entity_reference: u32,
    need_id: u32,
    satisfaction: u32,
) -> Result<(), String> {
    write_payload_entry(ctx, tic, entity_reference, |p| {
        resonantdust_codec::payload::upsert_need(p, need_id as u8, satisfaction as u8, tic);
    });
    Ok(())
}

/// `GRANT_CONDITION` — upsert one stored (timed) condition grant (`grant_tic` = this tic;
/// expiry is DERIVED from the corpus duration, never stored; a re-grant refreshes).
#[spacetimedb::reducer]
pub fn grant_condition(
    ctx: &ReducerContext,
    _worker: u8,
    tic: u16,
    entity_reference: u32,
    condition_id: u32,
) -> Result<(), String> {
    write_payload_entry(ctx, tic, entity_reference, |p| {
        resonantdust_codec::payload::upsert_condition(p, condition_id as u8, tic);
    });
    Ok(())
}

// ── spawn — server-minted pawn ids (`CREATE`) ───────────────────────────────────────

/// The replay ledger: `(event_reference, index) → minted entity_reference`. A `CREATE` replay
/// reads its row instead of re-minting (a 32-bit `event_reference` can't derive a 24-bit
/// `object_reference` — recorded, not computed).
#[spacetimedb::table(accessor = spawn_log, public)]
pub struct SpawnLog {
    /// `reserved:16 | event_reference:32 | index:16`.
    #[primary_key]
    pub spawn_uid: u64,
    #[index(btree)]
    pub event_reference: u32,
    /// Which `CREATE` within the event's program (composition order).
    pub index: u16,
    /// The minted pawn id.
    pub entity_reference: u32,
}

/// The mint counter. Starts at `SPAWN_BASE` — the TOP half of the 24-bit object space — so
/// server-minted ids never meet the legacy client-minted band (`selfEntity` = `player_id`,
/// npc `wolf_key` = small ints) while that stopgap still exists.
#[spacetimedb::table(accessor = spawn_counter)]
pub struct SpawnCounter {
    #[primary_key]
    pub id: u8,
    pub next: u32,
}

const OBJECT_REF_MASK: u32 = 0x00FF_FFFF;
const SPAWN_BASE: u32 = 0x0080_0000;

fn next_pawn_reference(ctx: &ReducerContext) -> u32 {
    let n = ctx.db.spawn_counter().id().find(0).map(|c| c.next).unwrap_or(SPAWN_BASE);
    ctx.db.spawn_counter().id().delete(0);
    ctx.db.spawn_counter().insert(SpawnCounter { id: 0, next: n.wrapping_add(1) & OBJECT_REF_MASK });
    pack_entity_reference(pack_server_reference(TYPE_PAWN, 0), n & OBJECT_REF_MASK)
}

/// Mint + first write + promote — AND the payload sidecar — in one transaction, idempotent by
/// `(event_reference, index)`. The worker's `CREATE` arm is the only caller. The first row is
/// absolute and `dirty=false` — a fresh entity is its own singleton component, so nothing
/// claimed it and nothing composes on it this tic (every read is `< tic`). An EMPTY `payload`
/// writes no sidecar rows at all (the wolf's common case stays free).
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
    promote: bool,
) -> Result<(), String> {
    let spawn_uid = ((event_reference as u64) << 16) | index as u64;
    if ctx.db.spawn_log().spawn_uid().find(spawn_uid).is_some() {
        return Ok(()); // replay — already spawned
    }
    let entity = next_pawn_reference(ctx);
    ctx.db.spawn_log().insert(SpawnLog { spawn_uid, event_reference, index, entity_reference: entity });

    let status_flags = if promote { STATE_FLAG_PROMOTE } else { 0 };
    let phase = if promote { STATE_PROMOTED } else { STATE_OPEN };
    let r = TargetState {
        entity_reference: entity,
        definition_reference,
        macro_position_reference: position_macro(position_reference),
        micro_position_reference: position_micro(position_reference),
        data: 0,
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
        data: 0,
        status: pack_status(status_flags, phase),
    });
    // The payload sidecar — SLAVED: written here, inside the spawn's state-write transaction
    // (F5). Empty payloads write nothing.
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
    if promote {
        entity_tables_upsert_state(ctx, &r, tic);
    }
    Ok(())
}
