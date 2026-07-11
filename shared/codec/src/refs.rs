//! The generalized reference layouts — `server_reference`, `entity_reference`,
//! `action_reference` — for the tick-pipeline generalization
//! (`docs/pipeline-generalization.md`).
//!
//! These supersede the two-tag `entity_key` in [`packed`](crate::packed): one `u64`
//! entity space discriminated by an 8-bit `entity_type` (minted vs positional), a `u16`
//! server reference (type + id), and a `u16` action reference (pipeline `data_type` +
//! `action_id`). They are added **alongside** the existing packers during migration; the
//! `packed` `entity_key`/`object_reference` helpers stay until every consumer is moved
//! over, then are removed.
//!
//! ```text
//! server_reference: u16 = server_type:6 | server_id:10
//! entity_reference: u64 = entity_type:8 | <56 bits, minted or positional>
//!   minted     (entity_type & 0x80 == 0): entity_id:32 | mint_server:16 | reserved:8
//!   positional (entity_type & 0x80 != 0): zone_id:32   | location:8 | layer:8 | reserved:8
//! action_reference: u16 = data_type:6 | action_id:10
//! ```
//!
//! Uniqueness is coordination-free: a minted entity's `mint_server` is globally unique
//! and permanent, `entity_id` is that server's monotonic counter; a positional entity's
//! `zone_id` is world-global. So every `entity_reference` is unique across the whole
//! system, which is what keeps `priority = hash(tic, entity_reference)` a single total
//! order spanning all classes (breaking cross-class dependency cycles — see
//! `docs/simulation.md`).

// ── server_reference ──────────────────────────────────────────────────────────
//
// u16 = server_type:6 | server_id:10. The `(type, id)` pair is globally unique because
// each type hands out its own `server_id` range. Names any process or database.

const SERVER_TYPE_SHIFT: u16 = 10;
const SERVER_ID_MASK: u16 = 0x03FF; // low 10 bits
const SERVER_TYPE_MASK: u16 = 0x3F; // 6 bits

/// The "no server" reference (`server_type == 0`, `server_id == 0`) — the fence
/// `SERVER_NONE`, and the system/self actor on injected events.
pub const SERVER_REF_NONE: u16 = 0;

/// Reserved "no server" type.
pub const SERVER_TYPE_NONE: u8 = 0;
/// The metronome (`server_master`).
pub const SERVER_TYPE_MASTER: u8 = 1;
/// A resolution worker (`server_worker`, formerly `server_simulation`).
pub const SERVER_TYPE_WORKER: u8 = 2;
/// Client I/O (`server_edge`).
pub const SERVER_TYPE_EDGE: u8 = 3;

/// `server_type >= SERVER_TYPE_DB_BASE` names a **database** server — one a worker can
/// open a `DbConnection` to and subscribe. The guard for `actor_server_reference`
/// routing: a cross-shard actor read asserts the reference is DB-backed before mapping
/// it to a connection. (Process types live below the base; DB types at/above it.)
pub const SERVER_TYPE_DB_BASE: u8 = 32;
/// An object-shard database.
pub const SERVER_TYPE_OBJECT_DB: u8 = 32;
/// A zone-shard database.
pub const SERVER_TYPE_ZONE_DB: u8 = 33;
/// A tile-shard database.
pub const SERVER_TYPE_TILE_DB: u8 = 34;

/// Compose a `server_reference` from its type and per-type id. `server_type` is masked
/// to 6 bits (≤ 64 types), `server_id` to 10 (≤ 1024 per type).
pub fn pack_server_reference(server_type: u8, server_id: u16) -> u16 {
    (((server_type as u16) & SERVER_TYPE_MASK) << SERVER_TYPE_SHIFT) | (server_id & SERVER_ID_MASK)
}

/// The `server_type` (0..64) of a `server_reference`.
pub fn server_ref_type(r: u16) -> u8 {
    ((r >> SERVER_TYPE_SHIFT) & SERVER_TYPE_MASK) as u8
}

/// The per-type `server_id` (0..1024) of a `server_reference`.
pub fn server_ref_id(r: u16) -> u16 {
    r & SERVER_ID_MASK
}

/// Does this reference name a database server (subscribable — has a `DbConnection`)?
pub fn server_ref_is_db(r: u16) -> bool {
    server_ref_type(r) >= SERVER_TYPE_DB_BASE
}

// ── entity_reference ──────────────────────────────────────────────────────────
//
// u64, top byte = entity_type. The 32-bit field at shift 24 is shared by both layouts
// (entity_id for minted, zone_id for positional); they diverge in the middle 16 bits.
// The reserved low byte is headroom.

const ENTITY_TYPE_SHIFT: u64 = 56;
const ENTITY_TYPE_MASK: u64 = 0xFF;
/// The 32-bit field just below the type byte — `entity_id` (minted) or `zone_id`
/// (positional).
const ENTITY_ID_SHIFT: u64 = 24;
const ENTITY_ID_MASK: u64 = 0xFFFF_FFFF;
const ENTITY_MINT_SERVER_SHIFT: u64 = 8; // minted: bits 8–23
const ENTITY_MINT_SERVER_MASK: u64 = 0xFFFF;
const ENTITY_LOCATION_SHIFT: u64 = 16; // positional: bits 16–23
const ENTITY_LAYER_SHIFT: u64 = 8; //     positional: bits 8–15
const ENTITY_BYTE_MASK: u64 = 0xFF;

/// Top bit of `entity_type` selects the payload layout: set = positional, clear =
/// minted. 128 minted types, 128 positional.
const ENTITY_TYPE_POSITIONAL_BIT: u8 = 0x80;

/// Reserved "no entity" type.
pub const ENTITY_TYPE_NONE: u8 = 0;
// minted (0x00–0x7F) — allocated an id by their minting server
/// A demo object (Phase-A moving circle).
pub const ENTITY_TYPE_DEMO: u8 = 1;
/// A player-controlled object.
pub const ENTITY_TYPE_PLAYER: u8 = 2;
/// A pawn (mobile agent).
pub const ENTITY_TYPE_PAWN: u8 = 3;
/// A free/mobile object.
pub const ENTITY_TYPE_OBJECT: u8 = 4;
/// A hot (actively-simulated) tile, before it packs into a zone.
pub const ENTITY_TYPE_TILE: u8 = 5;
// positional (0x80–0xFF) — identity IS location, never minted
/// A zone cell `(zone_id, location, layer)` — the cold store's positional entity.
pub const ENTITY_TYPE_ZONE_CELL: u8 = 0x80;
// A whole-zone AGGREGATE entity (the tile grid, the cold thing list) is keyed by a
// [`zone_reference`](pack_zone_reference), not an `entity_type`-tagged reference — it
// lives in its own DB, so it needs no type tag to stay unique. See below.

// Zone-cell layers — the `layer` byte of a positional entity lets multiple cells stack
// on one `(zone_id, location)` (floor + wall + affixed things). A starting palette;
// content can define more (up to 255).
/// Floor layer.
pub const LAYER_FLOOR: u8 = 0;
/// Wall layer.
pub const LAYER_WALL: u8 = 1;
/// Affixed-thing layer.
pub const LAYER_THING: u8 = 2;

/// A minted `entity_reference`: `entity_type:8 | entity_id:32 | mint_server:16 |
/// reserved:8`. Unique by construction — `mint_server` is globally unique, `entity_id`
/// its monotonic counter.
pub fn pack_minted_entity(entity_type: u8, entity_id: u32, mint_server: u16) -> u64 {
    ((entity_type as u64) << ENTITY_TYPE_SHIFT)
        | ((entity_id as u64) << ENTITY_ID_SHIFT)
        | ((mint_server as u64) << ENTITY_MINT_SERVER_SHIFT)
}

/// A positional `entity_reference`: `entity_type:8 | zone_id:32 | location:8 | layer:8 |
/// reserved:8`. Unique because `zone_id` is world-global; never minted (no allocation).
pub fn pack_positional_entity(entity_type: u8, zone_id: u32, location: u8, layer: u8) -> u64 {
    ((entity_type as u64) << ENTITY_TYPE_SHIFT)
        | ((zone_id as u64) << ENTITY_ID_SHIFT)
        | ((location as u64) << ENTITY_LOCATION_SHIFT)
        | ((layer as u64) << ENTITY_LAYER_SHIFT)
}

/// The `entity_type` (top byte) of an `entity_reference`.
pub fn entity_ref_type(k: u64) -> u8 {
    ((k >> ENTITY_TYPE_SHIFT) & ENTITY_TYPE_MASK) as u8
}

/// True if `entity_type` uses the positional layout (its top bit is set).
pub fn entity_type_is_positional(entity_type: u8) -> bool {
    entity_type & ENTITY_TYPE_POSITIONAL_BIT != 0
}

/// True if this reference is a positional (zone-cell) entity.
pub fn entity_ref_is_positional(k: u64) -> bool {
    entity_type_is_positional(entity_ref_type(k))
}

/// The minted `entity_id` (meaningless for positional refs — use [`entity_ref_zone_id`]).
pub fn entity_ref_id(k: u64) -> u32 {
    ((k >> ENTITY_ID_SHIFT) & ENTITY_ID_MASK) as u32
}

/// The minting `server_reference` of a minted entity (meaningless for positional refs).
pub fn entity_ref_mint_server(k: u64) -> u16 {
    ((k >> ENTITY_MINT_SERVER_SHIFT) & ENTITY_MINT_SERVER_MASK) as u16
}

/// The `zone_id` of a positional entity — the same slot as [`entity_ref_id`], read as a
/// zone (meaningless for minted refs).
pub fn entity_ref_zone_id(k: u64) -> u32 {
    ((k >> ENTITY_ID_SHIFT) & ENTITY_ID_MASK) as u32
}

/// The `location` of a positional entity (meaningless for minted refs).
pub fn entity_ref_location(k: u64) -> u8 {
    ((k >> ENTITY_LOCATION_SHIFT) & ENTITY_BYTE_MASK) as u8
}

/// The `layer` of a positional entity (meaningless for minted refs).
pub fn entity_ref_layer(k: u64) -> u8 {
    ((k >> ENTITY_LAYER_SHIFT) & ENTITY_BYTE_MASK) as u8
}

// ── zone_reference ──────────────────────────────────────────────────────────
//
// u64 = server_id:16 | reserved:16 | zone_id:32. A whole-zone AGGREGATE entity — a
// zone's tile grid, or its cold thing list — keyed by its globally-unique u32 `zone_id`,
// widened to the u64 the pipeline's `actor_key`/`target_key` need so a zone can be an
// actor/target in `event_log` events. `server_id` records which server minted (wrote) the
// row — provenance/consistency, spare for now. Distinct from the entity_type-discriminated
// `entity_reference`: a zone aggregate lives in its own DB (tiles / cold-things), so it
// needs no type tag to stay unique — its `zone_id` already is.

const ZONE_REF_SERVER_SHIFT: u64 = 48;
const ZONE_REF_ZONE_MASK: u64 = 0xFFFF_FFFF;

/// Pack a `zone_reference`: `server_id:16 | reserved:16 | zone_id:32`. Unique per zone
/// (`zone_id` is world-global); `server_id` records the minting server.
pub fn pack_zone_reference(server_id: u16, zone_id: u32) -> u64 {
    ((server_id as u64) << ZONE_REF_SERVER_SHIFT) | (zone_id as u64)
}

/// The minting `server_id` of a `zone_reference`.
pub fn zone_ref_server(r: u64) -> u16 {
    (r >> ZONE_REF_SERVER_SHIFT) as u16
}

/// The `zone_id` of a `zone_reference`.
pub fn zone_ref_zone_id(r: u64) -> u32 {
    (r & ZONE_REF_ZONE_MASK) as u32
}

// ── action_reference ──────────────────────────────────────────────────────────
//
// u16 = data_type:6 | action_id:10. `data_type` namespaces actions per pipeline so one
// worker resolves any pipeline by dispatching on it; DATA_TYPE_SHARED is reserved for
// cross-pipeline saga verbs meaning one thing everywhere.

const DATA_TYPE_SHIFT: u16 = 10;
const ACTION_ID_MASK: u16 = 0x03FF; // low 10 bits
const DATA_TYPE_MASK: u16 = 0x3F; // 6 bits

/// Reserved pipeline id for **shared/universal** actions — the saga verbs
/// (receive/ack/transfer, i.e. store/check_stored) and any cross-class action — so they
/// mean the same thing in every pipeline.
pub const DATA_TYPE_SHARED: u8 = 0;
/// The pawn pipeline.
pub const DATA_TYPE_PAWN: u8 = 1;
/// The object pipeline.
pub const DATA_TYPE_OBJECT: u8 = 2;
/// The zone pipeline (tiles + zone cells).
pub const DATA_TYPE_ZONE: u8 = 3;

/// Compose an `action_reference` from its pipeline `data_type` (≤ 64) and `action_id`
/// (≤ 1024 within that pipeline).
pub fn pack_action_reference(data_type: u8, action_id: u16) -> u16 {
    (((data_type as u16) & DATA_TYPE_MASK) << DATA_TYPE_SHIFT) | (action_id & ACTION_ID_MASK)
}

/// The pipeline `data_type` of an `action_reference`.
pub fn action_ref_data_type(r: u16) -> u8 {
    ((r >> DATA_TYPE_SHIFT) & DATA_TYPE_MASK) as u8
}

/// The per-pipeline `action_id` of an `action_reference`.
pub fn action_ref_id(r: u16) -> u16 {
    r & ACTION_ID_MASK
}

/// The pipeline (`data_type`) that resolves an entity of `entity_type` — many-to-one
/// (e.g. tiles and zone cells both resolve on the zone pipeline). The worker dispatches
/// which `Domain`/`apply_event` to run by this, and resolves a cross-read actor's domain
/// from the *actor's* type. A starting palette; content extends it. Unknown/none →
/// `DATA_TYPE_SHARED`.
pub fn entity_type_data_type(entity_type: u8) -> u8 {
    match entity_type {
        ENTITY_TYPE_PAWN => DATA_TYPE_PAWN,
        ENTITY_TYPE_DEMO | ENTITY_TYPE_PLAYER | ENTITY_TYPE_OBJECT => DATA_TYPE_OBJECT,
        ENTITY_TYPE_TILE | ENTITY_TYPE_ZONE_CELL => DATA_TYPE_ZONE,
        _ => DATA_TYPE_SHARED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reference_roundtrips() {
        for &(t, id) in &[(0u8, 0u16), (1, 1), (SERVER_TYPE_ZONE_DB, 7), (0x3F, 0x3FF)] {
            let r = pack_server_reference(t, id);
            assert_eq!(server_ref_type(r), t);
            assert_eq!(server_ref_id(r), id);
        }
        // type and id occupy disjoint bits — full type + full id.
        assert_eq!(pack_server_reference(0x3F, 0x3FF), 0xFFFF);
        assert_eq!(SERVER_REF_NONE, pack_server_reference(SERVER_TYPE_NONE, 0));
    }

    #[test]
    fn db_range_guard() {
        // Process servers are not DB-backed; database servers are.
        for t in [SERVER_TYPE_MASTER, SERVER_TYPE_WORKER, SERVER_TYPE_EDGE] {
            assert!(!server_ref_is_db(pack_server_reference(t, 5)));
        }
        for t in [SERVER_TYPE_OBJECT_DB, SERVER_TYPE_ZONE_DB, SERVER_TYPE_TILE_DB] {
            assert!(server_ref_is_db(pack_server_reference(t, 5)));
        }
    }

    #[test]
    fn minted_entity_roundtrips() {
        for &(t, id, srv) in &[(1u8, 1u32, 1u16), (ENTITY_TYPE_PAWN, 12345, 7), (0x7F, u32::MAX, u16::MAX)] {
            let k = pack_minted_entity(t, id, srv);
            assert_eq!(entity_ref_type(k), t);
            assert_eq!(entity_ref_id(k), id);
            assert_eq!(entity_ref_mint_server(k), srv);
            assert!(!entity_ref_is_positional(k), "minted types have a clear top bit");
        }
    }

    #[test]
    fn positional_entity_roundtrips() {
        for &(t, z, loc, lay) in &[
            (ENTITY_TYPE_ZONE_CELL, 0u32, 0u8, 0u8),
            (ENTITY_TYPE_ZONE_CELL, 0xABCD_1234, 200, 2),
            (0xFF, u32::MAX, 255, 255),
        ] {
            let k = pack_positional_entity(t, z, loc, lay);
            assert_eq!(entity_ref_type(k), t);
            assert_eq!(entity_ref_zone_id(k), z);
            assert_eq!(entity_ref_location(k), loc);
            assert_eq!(entity_ref_layer(k), lay);
            assert!(entity_ref_is_positional(k), "positional types have a set top bit");
        }
    }

    #[test]
    fn minted_and_positional_share_the_id_slot_but_never_collide() {
        // Same numeric 32-bit field, but distinct entity_types (minted vs positional top
        // bit) ⇒ distinct keys — the single-space uniqueness the priority order needs.
        let minted = pack_minted_entity(ENTITY_TYPE_OBJECT, 42, 7);
        let positional = pack_positional_entity(ENTITY_TYPE_ZONE_CELL, 42, 0, 0);
        assert_ne!(minted, positional);
        assert_ne!(entity_ref_type(minted), entity_ref_type(positional));
        // The 32-bit slot reads the same numeric value under either interpretation.
        assert_eq!(entity_ref_id(minted), entity_ref_zone_id(positional));
    }

    #[test]
    fn full_minted_fields_do_not_overlap() {
        // type:8 | id:32 | mint:16 | reserved:8 — all-ones in each field, disjoint.
        let k = pack_minted_entity(0xFF, u32::MAX, u16::MAX);
        assert_eq!(entity_ref_type(k), 0xFF);
        assert_eq!(entity_ref_id(k), u32::MAX);
        assert_eq!(entity_ref_mint_server(k), u16::MAX);
        assert_eq!(k, 0xFF_FF_FF_FF_FF_FF_FF_00);
    }

    #[test]
    fn zone_reference_roundtrips() {
        let r = pack_zone_reference(0xBEEF, 0xABCD_1234);
        assert_eq!(zone_ref_server(r), 0xBEEF);
        assert_eq!(zone_ref_zone_id(r), 0xABCD_1234);
        // server + zone occupy disjoint halves; the middle 16 reserved bits stay 0.
        assert_eq!(pack_zone_reference(u16::MAX, u32::MAX), 0xFFFF_0000_FFFF_FFFF);
    }

    #[test]
    fn action_reference_roundtrips() {
        for &(dt, id) in &[(0u8, 0u16), (DATA_TYPE_ZONE, 5), (0x3F, 0x3FF)] {
            let r = pack_action_reference(dt, id);
            assert_eq!(action_ref_data_type(r), dt);
            assert_eq!(action_ref_id(r), id);
        }
        assert_eq!(pack_action_reference(0x3F, 0x3FF), 0xFFFF);
    }

    #[test]
    fn entity_type_maps_to_pipeline() {
        assert_eq!(entity_type_data_type(ENTITY_TYPE_PAWN), DATA_TYPE_PAWN);
        assert_eq!(entity_type_data_type(ENTITY_TYPE_OBJECT), DATA_TYPE_OBJECT);
        assert_eq!(entity_type_data_type(ENTITY_TYPE_PLAYER), DATA_TYPE_OBJECT);
        assert_eq!(entity_type_data_type(ENTITY_TYPE_TILE), DATA_TYPE_ZONE);
        assert_eq!(entity_type_data_type(ENTITY_TYPE_ZONE_CELL), DATA_TYPE_ZONE);
        assert_eq!(entity_type_data_type(ENTITY_TYPE_NONE), DATA_TYPE_SHARED);
    }
}
