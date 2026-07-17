//! tile — the **cold** shard for a zone's ground tiles (`TYPE_BIOME_TILE`).
//!
//! One row per `(zone, biome, layer)`: the dense 16×16 = 256 `kind_reference`s, indexed by
//! `tile_reference` (0..256). A tile carries only **what kind** it is — no per-entry `tile_reference`
//! (that's the index) and no `data`, which is what lets a tile be a `u16` where a thing needs a
//! `u32`. A zone spanning several biomes is several rows (one per `subtype`); cells outside a row's
//! biome are `0`.
//!
//! Cold is static-ish: worldgen seeds it, the edge subscribes it per zone, the client draws the
//! ground. Mutation (chop/place) comes later via the `state`/`state_log` overlay; this increment is
//! storage + `seed` + the zone-keyed subscription, no tics.
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference` = `macro_position:16 | subtype_id:12 |
//! layer_id:4`), `docs/TABLES.md`. **`type_id` is this module** (`TYPE_BIOME_TILE`), so it's off the
//! row — reconstruct with `type_reference = TYPE_BIOME_TILE | subtype_id`.

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::object::{
    pack_cold_row_reference, pack_definition_reference, pack_layer_reference, pack_position_reference,
    pack_type_reference, TYPE_BIOME_TILE,
};
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference, OBJECT_REF_MAX, SERVER_REF_NONE};
use resonantdust_codec::status::{pack_status, STATE_FLAG_PROMOTE, STATE_PROMOTED};
use resonantdust_codec::uid::pack_state_uid;

// The shared tic-composition overlay: `clock` / `state_log` / `state` + `init` / `bump` / `claim` /
// `write` / `gc`. A cold cell mutates by minting a `state_log` row here (never rewriting the baseline
// `cold_tile`); GC folds it back. Same machinery as `data_shard`, so cold rides the whole pipeline.
resonantdust_codec::tick_pipeline!();

/// This cold shard's `server_reference` (`type_id = TYPE_BIOME_TILE`, server_id 0) — the high byte of
/// every `entity_reference` it mints on unpack. Const stopgap (matches `event_shard`); F2 makes it
/// master-assigned once a family has more than one shard.
const SERVER_REFERENCE: u8 = pack_server_reference(TYPE_BIOME_TILE, 0);

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

/// A zone is 16×16 tiles.
const TILES_PER_ZONE: usize = 256;

// ── cold_tile — one zone-layer-biome's dense ground ─────────────────────────────────

#[table(accessor = cold_tile, public)]
pub struct ColdTile {
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
    /// 256 `kind_reference`s, one per tile (index = `tile_reference`); cells outside this biome are
    /// `0`. Dense.
    pub tiles: Vec<u16>,
}

/// Seed (or overwrite) a zone-layer-biome's ground. This is trusted server-side **generation**
/// (worldgen), not a player mutation — player edits go through the overlay. `type_id` is the module.
/// Idempotent: re-seeding a `(zone, subtype, layer)` overwrites deterministically.
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tiles: Vec<u16>,
) -> Result<(), String> {
    if tiles.len() != TILES_PER_ZONE {
        return Err(format!("a zone has {TILES_PER_ZONE} tiles, got {}", tiles.len()));
    }
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    let row = ColdTile {
        cold_row_reference,
        macro_position_reference: macro_position,
        subtype_id,
        layer_id,
        tiles,
    };
    if ctx.db.cold_tile().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.cold_tile().cold_row_reference().update(row);
    } else {
        ctx.db.cold_tile().insert(row);
    }
    Ok(())
}

/// **Mutate a ground cell** to `kind_reference` via the overlay: mint (or reuse) an `entity_reference`
/// for the cell and write a settled, promoted `state` row — a **cold mutation** that never touches the
/// baseline `cold_tile`. The client composites this over the baseline (proven in P3). Idempotent by
/// position (one entity per cold cell), so a re-issue reuses the entity and just re-writes the value.
///
/// This is the direct mint+write primitive that proves the path end-to-end. The **event-driven**
/// `UNPACK` routing (edge → orchestrator → worker, deterministic-from-event id) and the **GC fold**
/// back into the baseline are the remaining P4 steps — see `docs/intent/world-storage/`.
#[reducer]
pub fn set_tile(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_reference: u8,
    kind_reference: u16,
) -> Result<(), String> {
    let layer_reference = pack_layer_reference(TYPE_BIOME_TILE, layer_id);
    let micro = ((tile_reference as u16) << 8) | layer_reference as u16;
    let position = pack_position_reference(macro_position, micro);
    let definition = pack_definition_reference(pack_type_reference(TYPE_BIOME_TILE, subtype_id), kind_reference);
    let tic = ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0);
    // Reuse the cell's entity if already unpacked, else mint a fresh one.
    let entity = ctx
        .db
        .state()
        .iter()
        .find(|s| s.position_reference == position)
        .map(|s| s.entity_reference)
        .unwrap_or_else(|| mint(ctx));
    // Settled, promoted composition slot (the source of truth).
    let uid = pack_state_uid(entity, tic);
    ctx.db.state_log().uid().delete(uid);
    ctx.db.state_log().insert(StateLog {
        uid,
        entity_reference: entity,
        tic,
        worker_reference: SERVER_REF_NONE,
        observer_reference: SERVER_REF_NONE,
        dirty: false,
        definition_reference: definition,
        position_reference: position,
        data: 0,
        status: pack_status(STATE_FLAG_PROMOTE, STATE_PROMOTED),
    });
    // The client-visible override.
    let row = State {
        entity_reference: entity,
        macro_position_reference: macro_position,
        tic,
        definition_reference: definition,
        position_reference: position,
        data: 0,
    };
    if ctx.db.state().entity_reference().find(entity).is_some() {
        ctx.db.state().entity_reference().update(row);
    } else {
        ctx.db.state().insert(row);
    }
    Ok(())
}
