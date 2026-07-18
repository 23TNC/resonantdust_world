//! thing — the **cold** shard for a zone's scattered things (trees, rocks, …) (`TYPE_BIOME_THING`).
//!
//! One row per `(zone, biome, layer)`: a **sparse** `Vec<u32>` of `kind_pos_reference`s
//! (`kind_reference:16 | tile_reference:8 | data:8`). Unlike a tile — which is only *what kind* and
//! whose position is its dense index — a thing carries *where in the zone* it sits and *its data*
//! (`rotation:2 | count:6`), so it needs the full `u32` and can't be indexed by position. Only
//! occupied cells appear; a zone spanning several biomes is several rows (one per `subtype`).
//!
//! Cold is static-ish: worldgen seeds it, the edge subscribes it per zone, the client draws the
//! scatter over the ground. Mutation goes through the `state`/`state_log` overlay (`set_thing` mints an override; `fold` PACKs it
//! back into the baseline) — never rewriting `cold_thing` in place.
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference`, `kind_pos_reference`), `docs/TABLES.md`.
//! **`type_id` is this module** (`TYPE_BIOME_THING`), so it's off the row.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::object::{
    def_kind_reference, def_subtype_id, kind_pos_ref_tile, pack_cold_row_reference,
    pack_definition_reference, pack_kind_pos_reference, pack_layer_reference, pack_position_reference,
    pack_type_reference, TYPE_BIOME_THING,
};
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference, OBJECT_REF_MAX, SERVER_REF_NONE};
use resonantdust_codec::status::{pack_status, STATE_FLAG_PROMOTE, STATE_PROMOTED};
use resonantdust_codec::uid::pack_state_uid;

// The shared tic-composition overlay: `clock` / `state_log` / `state` + `init` / `bump` / `claim` /
// `write` / `gc`. A cold cell mutates by minting a `state_log` row here (never rewriting the baseline
// `cold_thing`); GC folds it back. Same machinery as `data_shard`, so cold rides the whole pipeline.
resonantdust_codec::entity_tables!(definition_reference: u32, position_reference: u32, data: u8);

/// This cold shard's `server_reference` (`type_id = TYPE_BIOME_THING`, server_id 0) — the high byte of
/// every `entity_reference` it mints. Const stopgap; F2 makes it master-assigned at multi-shard.
const SERVER_REFERENCE: u8 = pack_server_reference(TYPE_BIOME_THING, 0);

/// The mint counter — `entity_reference = server_reference:8 | ++counter:24`. Single row, lazily seeded.
#[table(accessor = mint_counter)]
pub struct MintCounter {
    #[primary_key]
    pub id: u8,
    pub next: u32,
}

/// Allocate the next `entity_reference` for an unpacked cell.
fn mint(ctx: &ReducerContext) -> u32 {
    let n = ctx.db.mint_counter().id().find(0).map(|c| c.next).unwrap_or(1);
    ctx.db.mint_counter().id().delete(0);
    ctx.db.mint_counter().insert(MintCounter { id: 0, next: n.wrapping_add(1) & OBJECT_REF_MAX });
    pack_entity_reference(SERVER_REFERENCE, n)
}

// ── cold_thing — one zone-layer-biome's sparse scatter ──────────────────────────────

#[table(accessor = cold_thing, public)]
pub struct ColdThing {
    /// `cold_row_reference` = `macro_position:16 | subtype_id:12 | layer_id:4`.
    #[primary_key]
    pub cold_row_reference: u32,
    /// The zone — the client's subscription key (`WHERE macro_position_reference = <zone>`).
    #[index(btree)]
    pub macro_position_reference: u16,
    /// The biome (holds `u12`); searchable.
    #[index(btree)]
    pub subtype_id: u16,
    /// The layer (holds `u4`).
    pub layer_id: u8,
    /// The `tic` this baseline is current as of (set at `seed`, bumped by `fold`) — a cold row is a
    /// compressed `state` row, so `tic` orders it against a `state` override (most recent wins).
    pub tic: u16,
    /// Sparse `kind_pos_reference`s — one per occupied cell (`kind:16 | tile:8 | data:8`).
    pub things: Vec<u32>,
}

/// The cold shard's current tic (its `clock` mirror, bumped by the master).
fn now_tic(ctx: &ReducerContext) -> u16 {
    ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0)
}

/// Seed (or overwrite) a zone-layer-biome's scatter. Trusted server-side **generation** (worldgen),
/// not a player mutation — player edits go through the overlay. `type_id` is the module. Idempotent:
/// re-seeding a `(zone, subtype, layer)` overwrites deterministically. (Sparse, so no fixed length;
/// entries are expected to hold distinct `tile_reference`s — one thing per cell per layer — but that
/// isn't enforced here.)
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    things: Vec<u32>,
) -> Result<(), String> {
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    let row = ColdThing {
        cold_row_reference,
        macro_position_reference: macro_position,
        subtype_id,
        layer_id,
        tic: now_tic(ctx),
        things,
    };
    if ctx.db.cold_thing().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.cold_thing().cold_row_reference().update(row);
    } else {
        ctx.db.cold_thing().insert(row);
    }
    Ok(())
}

/// **Mutate/place a scatter thing** at a cell to `(kind_reference, data)` via the overlay: mint (or
/// reuse) an `entity_reference` and write a settled + promoted `state` override — never touching the
/// baseline `cold_thing`. `kind_reference == 0` **removes** the cell's thing. Idempotent by position.
/// The direct mint+write primitive; event-driven routing is the P4 follow-up.
#[reducer]
pub fn set_thing(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_reference: u8,
    kind_reference: u16,
    data: u8,
) -> Result<(), String> {
    let layer_reference = pack_layer_reference(TYPE_BIOME_THING, layer_id);
    let micro = ((tile_reference as u16) << 8) | layer_reference as u16;
    let position = pack_position_reference(macro_position, micro);
    // Keep the cell's biome — find the baseline row holding a thing at this cell (if any); else the
    // passed `subtype_id` (a placement onto an empty cell must be told the biome).
    let subtype_id = ctx
        .db
        .cold_thing()
        .macro_position_reference()
        .filter(macro_position)
        .find(|r| r.layer_id == layer_id && r.things.iter().any(|&e| kind_pos_ref_tile(e) == tile_reference))
        .map(|r| r.subtype_id)
        .unwrap_or(subtype_id);
    let definition = pack_definition_reference(pack_type_reference(TYPE_BIOME_THING, subtype_id), kind_reference);
    let tic = ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0);
    let entity = ctx
        .db
        .entity_state()
        .iter()
        .find(|s| s.position_reference == position)
        .map(|s| s.entity_reference)
        .unwrap_or_else(|| mint(ctx));
    let uid = pack_state_uid(entity, tic);
    ctx.db.entity_state_log().uid().delete(uid);
    ctx.db.entity_state_log().insert(EntityStateLog {
        uid,
        entity_reference: entity,
        tic,
        worker_reference: SERVER_REF_NONE,
        observer_reference: SERVER_REF_NONE,
        dirty: false,
        definition_reference: definition,
        position_reference: position,
        data,
        status: pack_status(STATE_FLAG_PROMOTE, STATE_PROMOTED),
    });
    let row = EntityState {
        entity_reference: entity,
        macro_position_reference: macro_position,
        tic,
        definition_reference: definition,
        position_reference: position,
        data,
    };
    if ctx.db.entity_state().entity_reference().find(entity).is_some() {
        ctx.db.entity_state().entity_reference().update(row);
    } else {
        ctx.db.entity_state().insert(row);
    }
    Ok(())
}

/// **GC fold (`PACK`)** — fold every settled `state` override back into the sparse baseline
/// `cold_thing`, then drop the `state` row + `state_log`. For each cell: replace any existing entry at
/// its `tile_reference`, then add the new `kind_pos_reference` (or, if `kind == 0`, just leave it
/// removed). The client sees no change (it read the override; now it reads the same from the baseline).
/// Only `!dirty` rows fold; the master's GC calls it on cadence.
#[reducer]
pub fn fold(ctx: &ReducerContext) -> Result<(), String> {
    // The baseline is current as of *now* once folded — `now_tic` is ≥ every settled override's tic,
    // so all of them become superseded (the client reads the baseline, not the stale override).
    let ftic = now_tic(ctx);
    let settled: Vec<EntityState> = ctx.db.entity_state().iter().collect();
    for s in settled {
        let log_settled = ctx
            .db
            .entity_state_log()
            .uid()
            .find(pack_state_uid(s.entity_reference, s.tic))
            .map(|l| !l.dirty)
            .unwrap_or(true);
        if !log_settled {
            continue;
        }
        let tile_reference = ((s.position_reference >> 8) & 0xFF) as u8;
        let layer_id = (s.position_reference & 0xF) as u8;
        let subtype_id = def_subtype_id(s.definition_reference);
        let kind_reference = def_kind_reference(s.definition_reference);
        let cold_row = pack_cold_row_reference(s.macro_position_reference, subtype_id, layer_id);
        match ctx.db.cold_thing().cold_row_reference().find(cold_row) {
            Some(mut row) => {
                row.things.retain(|&e| kind_pos_ref_tile(e) != tile_reference);
                if kind_reference != 0 {
                    row.things.push(pack_kind_pos_reference(kind_reference, tile_reference, s.data));
                }
                row.tic = ftic;
                ctx.db.cold_thing().cold_row_reference().update(row);
            }
            None if kind_reference != 0 => {
                // No baseline row for this biome yet — create it with the single entry.
                ctx.db.cold_thing().insert(ColdThing {
                    cold_row_reference: cold_row,
                    macro_position_reference: s.macro_position_reference,
                    subtype_id,
                    layer_id,
                    tic: ftic,
                    things: vec![pack_kind_pos_reference(kind_reference, tile_reference, s.data)],
                });
            }
            None => {}
        }
        ctx.db.entity_state().entity_reference().delete(s.entity_reference);
        ctx.db.entity_state_log().uid().delete(pack_state_uid(s.entity_reference, s.tic));
    }
    Ok(())
}
