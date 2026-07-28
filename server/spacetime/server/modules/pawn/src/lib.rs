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

resonantdust_codec::entity_tables!(data: u8);

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

/// Mint + first write + promote, in one transaction, idempotent by `(event_reference, index)`.
/// The worker's `CREATE` arm is the only caller. The first row is absolute and `dirty=false` —
/// a fresh entity is its own singleton component, so nothing claimed it and nothing composes on
/// it this tic (every read is `< tic`).
#[spacetimedb::reducer]
pub fn spawn(
    ctx: &ReducerContext,
    worker: u8,
    tic: u16,
    event_reference: u32,
    index: u16,
    definition_reference: u32,
    position_reference: u32,
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
    if promote {
        entity_tables_upsert_state(ctx, &r, tic);
    }
    Ok(())
}
