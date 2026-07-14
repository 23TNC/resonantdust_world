//! `server_reference` + `entity_reference` — the identity / addressing layer of the 0.2.3
//! reference model. Canonical shape:
//! `docs/components/shared/codec/design/reference-model.md`.
//!
//! ```text
//! server_reference : u16 = realm_id:8 | server_id:8                    (geographic)
//! object_reference : u32 = the universal handle; its variant named by `reference_id`
//! entity_reference : u64 = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32
//! ```
//!
//! `entity_reference` makes an `object_reference` globally unique + (optionally)
//! self-describing. The low 48 bits (`server_reference:16 | object_reference:32`) are exactly
//! the DSL word's qualified reference ([`crate::event_word`]) for the **hot** variant, so a hot
//! handle passes identically whether it rides a word or a full `entity_reference`.
//!
//! **Two identity classes share the `u64`, discriminated by `reference_id`:**
//! - **HOT** (`REF_HOT`) — a live/minted object: `server_reference` is its minting server, the
//!   `object_reference:32` is its per-server `hot_reference`. This is the clean new layout.
//! - **COLD** (`REF_COLD`) — a settled object addressed by location (find-or-mint / Interact
//!   targets). Its low 48 bits hold the **world-global** position `zone_id:32 | location:8 |
//!   layer:8` rather than `server|object`, because a world-global `zone_id:u32` doesn't fit the
//!   compressed `position_reference:32` — that compression is the world-geometry decision parked
//!   in `work/spacetime-rewrite/blockers.md` (B-2). This is the interim cold form until B-2.
//!
//! Pure integer math — no naming, no I/O. `type_id ↔ name` is the registry's job.

// ── server_reference : u16 = realm_id:8 | server_id:8 ────────────────────────────
//
// Geographic: all servers in a `realm` work together, so a `server_reference` names the realm
// without stating it separately — `(server_reference, object_reference)` is realm-unique with no
// dedicated realm field. There is no functional `server_type` here: the worker maps a
// `server_reference` to a DB connection via its `SHARDS` env list, not a type tag.

const SERVER_REALM_SHIFT: u16 = 8;
const SERVER_BYTE_MASK: u16 = 0xFF;

/// The "no server" reference (`realm 0`, `server 0`) — the fence `SERVER_NONE`, the system/self
/// actor on injected events, and an *unminted* handle (a fresh spawn not yet bound to a server;
/// the worker routes it to whichever shard processes its event).
pub const SERVER_REF_NONE: u16 = 0;

/// Compose a `server_reference`: `realm_id:8 | server_id:8`.
pub fn pack_server_reference(realm_id: u8, server_id: u8) -> u16 {
    ((realm_id as u16) << SERVER_REALM_SHIFT) | (server_id as u16)
}

/// The `realm_id` (high byte) of a `server_reference`.
pub fn server_ref_realm(r: u16) -> u8 {
    ((r >> SERVER_REALM_SHIFT) & SERVER_BYTE_MASK) as u8
}

/// The `server_id` (low byte) of a `server_reference` — the shard/server within its realm.
pub fn server_ref_server_id(r: u16) -> u8 {
    (r & SERVER_BYTE_MASK) as u8
}

// ── entity_reference : u64 ───────────────────────────────────────────────────────
//
// reserved:10 | reference_id:6 | server_reference:16 | object_reference:32.

const REFERENCE_ID_SHIFT: u64 = 48;
const REFERENCE_ID_MASK: u64 = 0x3F; // 6 bits, bits 48–53
const ENTITY_SERVER_SHIFT: u64 = 32;
const ENTITY_SERVER_MASK: u64 = 0xFFFF; // 16 bits, bits 32–47
const ENTITY_OBJECT_MASK: u64 = 0xFFFF_FFFF; // 32 bits, bits 0–31

// ── reference_id : which reference variant an object_reference is ────────────────
// Append-only (0 = none). Names the *reference variant* (hot/cold/position/event/server) — NOT
// the game-type (that's `definition_reference.type_id`, carried in the payload). The old
// `entity_type` conflated the two; the reference model separates them.
/// Reserved null/unset variant.
pub const REF_NONE: u8 = 0;
/// A live/minted object — `object_reference = hot_reference:32`, per-server. Can't be `unpack`ed.
pub const REF_HOT: u8 = 1;
/// A settled object addressed by location — `unpack`able to hot. Interim form carries the
/// world-global `zone_id:32 | location:8 | layer:8` in the low 48 (see module docs / B-2).
pub const REF_COLD: u8 = 2;
/// A bare location (any object has one). Same interim position layout as `REF_COLD`.
pub const REF_POSITION: u8 = 3;
/// An event row — `object_reference = event_reference:32` (what `ALIAS`/`AWAIT` carry).
pub const REF_EVENT: u8 = 4;
/// A server, as an object.
pub const REF_SERVER: u8 = 5;

/// Compose an `entity_reference` from its variant tag, qualifying server, and handle:
/// `reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`.
pub fn pack_entity_reference(reference_id: u8, server_reference: u16, object_reference: u32) -> u64 {
    (((reference_id as u64) & REFERENCE_ID_MASK) << REFERENCE_ID_SHIFT)
        | ((server_reference as u64) << ENTITY_SERVER_SHIFT)
        | (object_reference as u64)
}

/// The `reference_id` (variant tag) of an `entity_reference`.
pub fn entity_ref_reference_id(k: u64) -> u8 {
    ((k >> REFERENCE_ID_SHIFT) & REFERENCE_ID_MASK) as u8
}

/// The qualifying `server_reference` (bits 32–47) of a **hot** `entity_reference` — its minting
/// server. (Meaningless on a COLD ref, whose low 48 hold a position; guard with
/// [`entity_ref_is_positional`].) This is what the worker's `home_shard` routes by.
pub fn entity_ref_server_reference(k: u64) -> u16 {
    ((k >> ENTITY_SERVER_SHIFT) & ENTITY_SERVER_MASK) as u16
}

/// The `object_reference` (low 32) of an `entity_reference` — a hot object's `hot_reference`, an
/// event's `event_reference`, … per `reference_id`.
pub fn entity_ref_object_reference(k: u64) -> u32 {
    (k & ENTITY_OBJECT_MASK) as u32
}

/// A **hot** `entity_reference`: `REF_HOT`, minted by `server_reference`, handle `hot_reference`.
/// Globally unique — `server_reference` is unique and `hot_reference` is that server's monotonic
/// counter, so `(server_reference, hot_reference)` never collides (what keeps the priority order a
/// single total order across shards).
pub fn pack_hot_entity(server_reference: u16, hot_reference: u32) -> u64 {
    pack_entity_reference(REF_HOT, server_reference, hot_reference)
}

// ── COLD / positional entity (interim, world-global — see B-2) ───────────────────
//
// A cold/positional target's low 48 bits hold the world-global position `zone_id:32 |
// location:8 | layer:8`, NOT `server|object` — a world-global `zone_id:u32` doesn't fit the
// compressed `position_reference:32`. The variant tag (`REF_COLD`) sits in `reference_id` as
// usual, so hot and cold never collide even though cold overloads the low fields.

const COLD_ZONE_SHIFT: u64 = 16;
const COLD_LOCATION_SHIFT: u64 = 8;
const COLD_BYTE_MASK: u64 = 0xFF;
const COLD_ZONE_MASK: u64 = 0xFFFF_FFFF;

/// A **cold** (positional) `entity_reference` naming a settled object by its world-global
/// location: `REF_COLD | zone_id:32 | location:8 | layer:8`. Never minted (identity IS location).
pub fn pack_cold_entity(zone_id: u32, location: u8, layer: u8) -> u64 {
    ((REF_COLD as u64) << REFERENCE_ID_SHIFT)
        | ((zone_id as u64) << COLD_ZONE_SHIFT)
        | ((location as u64) << COLD_LOCATION_SHIFT)
        | (layer as u64)
}

/// True if this reference is a cold/positional target (its `reference_id` is `REF_COLD` /
/// `REF_POSITION`) — the ones the worker `find-or-mint`s. (Was: the old `entity_type` top bit.)
pub fn entity_ref_is_positional(k: u64) -> bool {
    matches!(entity_ref_reference_id(k), REF_COLD | REF_POSITION)
}

/// The world-global `zone_id` of a cold/positional entity (meaningless for hot refs).
pub fn entity_ref_zone_id(k: u64) -> u32 {
    ((k >> COLD_ZONE_SHIFT) & COLD_ZONE_MASK) as u32
}

/// The in-zone cell `location` of a cold/positional entity (meaningless for hot refs).
pub fn entity_ref_location(k: u64) -> u8 {
    ((k >> COLD_LOCATION_SHIFT) & COLD_BYTE_MASK) as u8
}

/// The `layer` of a cold/positional entity (meaningless for hot refs).
pub fn entity_ref_layer(k: u64) -> u8 {
    (k & COLD_BYTE_MASK) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reference_roundtrips() {
        for &(realm, id) in &[(0u8, 0u8), (1, 1), (7, 33), (0xFF, 0xFF)] {
            let r = pack_server_reference(realm, id);
            assert_eq!(server_ref_realm(r), realm);
            assert_eq!(server_ref_server_id(r), id);
        }
        // realm and server_id occupy disjoint bytes.
        assert_eq!(pack_server_reference(0xFF, 0xFF), 0xFFFF);
        assert_eq!(SERVER_REF_NONE, pack_server_reference(0, 0));
    }

    #[test]
    fn hot_entity_roundtrips() {
        for &(srv, obj) in &[(1u16, 1u32), (0xBEEF, 12345), (u16::MAX, u32::MAX)] {
            let k = pack_hot_entity(srv, obj);
            assert_eq!(entity_ref_reference_id(k), REF_HOT);
            assert_eq!(entity_ref_server_reference(k), srv);
            assert_eq!(entity_ref_object_reference(k), obj);
            assert!(!entity_ref_is_positional(k), "hot is not positional");
            // low 48 == the DSL word's qualified reference (server:16 | object:32).
            assert_eq!(k & 0xFFFF_FFFF_FFFF, ((srv as u64) << 32) | obj as u64);
        }
    }

    #[test]
    fn cold_entity_roundtrips() {
        for &(z, loc, lay) in &[(0u32, 0u8, 0u8), (0xABCD_1234, 200, 3), (u32::MAX, 255, 255)] {
            let k = pack_cold_entity(z, loc, lay);
            assert_eq!(entity_ref_reference_id(k), REF_COLD);
            assert_eq!(entity_ref_zone_id(k), z);
            assert_eq!(entity_ref_location(k), loc);
            assert_eq!(entity_ref_layer(k), lay);
            assert!(entity_ref_is_positional(k), "cold is positional");
        }
    }

    #[test]
    fn hot_and_cold_never_collide() {
        // Distinct reference_id ⇒ distinct keys even when the low bits coincide — the
        // single-space uniqueness the priority order needs (hot server|object vs cold position).
        let hot = pack_hot_entity(0x0042, 7);
        let cold = pack_cold_entity(0x42, 0, 7);
        assert_ne!(hot, cold);
        assert_ne!(entity_ref_reference_id(hot), entity_ref_reference_id(cold));
    }

    #[test]
    fn full_hot_fields_do_not_overlap() {
        // reference_id:6 | server:16 | object:32 — all-ones per field, disjoint; reserved:10 = 0.
        let k = pack_entity_reference(0x3F, u16::MAX, u32::MAX);
        assert_eq!(entity_ref_reference_id(k), 0x3F);
        assert_eq!(entity_ref_server_reference(k), u16::MAX);
        assert_eq!(entity_ref_object_reference(k), u32::MAX);
        assert_eq!(k, 0x003F_FFFF_FFFF_FFFF);
    }
}
