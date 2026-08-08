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
/// row — AND its `needs` rows (stat-model F2: the sub-table is zone-keyed for the same
/// subscription reason) — into the entity's zone, in the SAME transaction. A zone-crossing
/// hop is the one movement that touches the sidecars, and only their keys.
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

// ── needs — the per-(pawn, need) row table (stat-model F2) ──────────────────────────────
//
// Needs churn on every sip and every re-stamp, so they fan ALONE instead of dragging the
// whole payload. One row per (entity, need kind|variant); the value is u16 FIXED-POINT on
// the need's authored domain, quantized ONCE by the composer (F4); `set_tic` is the lazy
// eval's anchor — nothing ever ticks the value. No log twin, deliberately (TABLES.md).

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

// ── inventory — the per-(pawn, slot) item table (inventory F2) ────────────────────────
//
// One row per HELD item; the row count is the pawn's FILLED slots, while the `inventory`
// NEED counts FREE slots (F1, the user's inversion) — never conflate the two (I2). Items
// are u32 definition_references; `state` is RESERVED for the item-as-entity successor
// (the minted item's entity id — its world row suppressed while held). Rows move ONLY
// through `inv_add`/`inv_remove`, and the worker writes the free-count SET_NEED in the
// same program (F3). No log twin, deliberately (TABLES.md).

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

/// The encoding-domain slot bound (VARIABLES.md: the need's 0..16 domain). The worker's
/// capacity gate is the NEED (`can_carry`); this is only the structural sanity limit.
const INVENTORY_SLOT_BOUND: u8 = 16;

/// `INV_ADD` — insert the item at the FIRST FREE slot. Err (no write) when every slot in
/// the structural bound is taken — the worker refuses via the need before it gets here,
/// so hitting this means the free-count and the rows diverged (I2/I3): loud, not silent.
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

/// `INV_REMOVE` — delete one slot's row. Silent on absence (a raced double-drop's second
/// completion already no-ops at the worker's re-validation; this is the last line).
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

/// `SET_NEED` — upsert one `needs` sub-table row (stat-model F2/F4). `row` is the packed
/// gameplay row `value:16 | kind:12 | variant:4`; the value is u16 fixed-point on the
/// need's authored domain, quantized ONCE by the composer; `set_tic` = this tic (the lazy
/// eval's anchor — observers compute the current value, F4).
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

/// `GRANT_CONDITION` — upsert one stored (timed) condition row (stat-model F1/F3). `row`
/// is `remaining_at_write:16 | kind:12 | variant:4`; `written_tic` = this tic; remaining
/// and expiry are DERIVED at read, never stored; a re-grant refreshes (upsert by the low
/// 16). The re-stamp law binds the COMPOSER (I12) — this reducer holds no corpus.
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

/// `remove` — DEATH's second half (food-chain F5): delete the entity's CURRENT rows —
/// `entity_state` (the subscription leave fans StateGone; clients drop the mover),
/// `payload`, every `needs` row, and every `inventory` row. The LOGS stay (history; the mint counter never
/// reuses ids, so a stale log can never resurrect anyone). Idempotent: removing an
/// absent entity is a silent success (a raced double-death no-ops — I3).
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
    // The dead drop everything (inventory F2): held items are NOT spilled — the death
    // spawn already yields meat; item-spill is a design decision for the item-as-entity
    // successor, not a default.
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

/// Mint + first write + promote — AND the sidecars — in one transaction, idempotent by
/// `(event_reference, index)`. The worker's `CREATE` arm is the only caller: it composes
/// `payload` (PART + TRAIT entries — stat-model F11: starting traits mint here) and `needs`
/// (packed full-value rows for the def's needs list) from the corpus, because this module
/// holds none. The first row is absolute and `dirty=false` — a fresh entity is its own
/// singleton component, so nothing claimed it and nothing composes on it this tic. An EMPTY
/// `payload` writes no sidecar rows at all.
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
    // `data` (spawn-authority I9): the minted row's byte — `facing | trip_serial`
    // packed by the WORKER (`pack_pawn_data(rotation, 0)`; the serial lane stays 0,
    // no chain exists yet). The request's rotation nibble seeds the RESTING pose.
    data: u8,
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
    // The needs sub-table mint (stat-model F2/F11): one row per packed full-value row the
    // worker composed from the def's needs list, stamped at the spawn tic.
    for row in needs {
        upsert_need_row(ctx, tic, entity, row, r.macro_position_reference);
    }
    if promote {
        entity_tables_upsert_state(ctx, &r, tic);
    }
    Ok(())
}
