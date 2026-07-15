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
//!   `object_reference:32` is its per-server `hot_reference`.
//! - **COLD** (`REF_COLD`) — a settled object addressed by location (find-or-mint / Interact
//!   targets): `object_reference:32` is a geographic [`cold_reference`](crate::object)
//!   (`region:8 | zone:8 | tile:8 | layer:8`), qualified by `server_reference` (realm + shard).
//!   Realm rides `server_reference` — a shard is realm-scoped — so the full geographic address is
//!   `server_reference.realm . region . zone . tile`.
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

// ── COLD / positional entity — a geographic cold_reference ───────────────────────
//
// `REF_COLD | server_reference | cold_reference:32` — `cold_reference` (object.rs) is
// `region:8 | zone:8 | tile:8 | layer_reference:8`. Realm is NOT in the `cold_reference` (a
// shard is realm-scoped); it rides `server_reference`. So the full geographic address is
// `server_reference.realm . region . zone . tile`. The `REF_COLD` tag keeps it distinct from a
// hot handle even when the low bits coincide.

/// A **cold** (positional) `entity_reference`: `REF_COLD`, qualified by `server_reference` (its
/// realm + shard), addressing the settled object by `cold_reference` (region|zone|tile|layer).
/// Never minted — identity IS location.
pub fn pack_cold_entity(server_reference: u16, cold_reference: u32) -> u64 {
    pack_entity_reference(REF_COLD, server_reference, cold_reference)
}

/// The `cold_reference` (object.rs `region:8 | zone:8 | tile:8 | layer:8`) of a cold entity.
pub fn entity_ref_cold_reference(k: u64) -> u32 {
    entity_ref_object_reference(k)
}

/// True if this reference is a cold/positional target (its `reference_id` is `REF_COLD` /
/// `REF_POSITION`) — the ones the worker `find-or-mint`s. (Was: the old `entity_type` top bit.)
pub fn entity_ref_is_positional(k: u64) -> bool {
    matches!(entity_ref_reference_id(k), REF_COLD | REF_POSITION)
}

/// The full geographic `zone_id` (`realm | region | zone | 0`) of a cold entity — realm from
/// `server_reference`, region+zone from the `cold_reference`. This is what the (realm-scoped)
/// cold table is keyed by. (Meaningless for hot refs.)
pub fn entity_ref_zone_id(k: u64) -> u32 {
    let realm = server_ref_realm(entity_ref_server_reference(k)) as u32;
    let region_zone = crate::object::cold_ref_region_zone(entity_ref_cold_reference(k)) as u32;
    (realm << 24) | (region_zone << 8)
}

/// The in-zone cell `location` (`tile_reference`) of a cold entity (meaningless for hot refs).
pub fn entity_ref_location(k: u64) -> u8 {
    crate::object::cold_ref_tile(entity_ref_cold_reference(k))
}

/// The `layer_id` of a cold entity (meaningless for hot refs).
pub fn entity_ref_layer(k: u64) -> u8 {
    crate::object::layer_ref_layer_id(crate::object::cold_ref_layer_reference(entity_ref_cold_reference(k)))
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
        use crate::object::{pack_cold_reference, pack_layer_reference, pack_position_reference};
        // realm 1 (server_reference), region(2,3) zone(4,5) tile(6,7) type 3 layer 2.
        let server = pack_server_reference(0x11, 0);
        let cr = pack_cold_reference(0x23, 0x45, pack_position_reference(6, 7), pack_layer_reference(3, 2));
        let k = pack_cold_entity(server, cr);
        assert_eq!(entity_ref_reference_id(k), REF_COLD);
        assert_eq!(entity_ref_cold_reference(k), cr);
        assert!(entity_ref_is_positional(k), "cold is positional");
        // zone_id reconstructs realm|region|zone|0 from server_reference.realm + cold_reference.
        assert_eq!(entity_ref_zone_id(k), (0x11u32 << 24) | (0x23u32 << 16) | (0x45u32 << 8));
        assert_eq!(entity_ref_location(k), pack_position_reference(6, 7));
        assert_eq!(entity_ref_layer(k), 2);
    }

    #[test]
    fn hot_and_cold_never_collide() {
        // Distinct reference_id ⇒ distinct keys even when the low bits coincide — the
        // single-space uniqueness the priority order needs (hot server|object vs cold position).
        let hot = pack_hot_entity(0x0042, 7);
        let cold = pack_cold_entity(0x0042, 7);
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
