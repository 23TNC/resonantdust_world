//! `server_reference` + `entity_reference` — the identity / addressing layer of the 0.2.3
//! reference model. Authoritative shape: `docs/VARIABLES.md`.
//!
//! ```text
//! server_reference       : u8  = type_id:4 | server_id:4
//! object_reference       : u24 = an opaque per-server minted id
//! entity_reference       : u32 = server_reference:8 | object_reference:24
//! realm_server_reference : u16 = realm_reference:8 | server_reference:8   (cross-realm only)
//! ```
//!
//! **The reference carries no type tag — the server does.** A server serves objects of one
//! `type_id`, and that nibble is the high half of its `server_reference`, so an
//! `entity_reference` says what it is by saying where it lives. This is why there's no
//! `reference_id` field: the variants it used to discriminate no longer exist as separate
//! reference types.
//!
//! **Not realm-unique.** An `entity_reference` is unique within a realm — `(server_reference,
//! object_reference)` is `(who minted it, its counter)`. Realm is deliberately *not* in the
//! u32. Crossing a realm boundary, prepend the `realm_reference:8` and pass a
//! [`pack_realm_server_reference`] instead; within a realm (the overwhelmingly common path) it
//! costs nothing.
//!
//! Pure integer math — no naming, no I/O. `type_id ↔ name` is the registry's job.

// ── server_reference : u8 = type_id:4 | server_id:4 ──────────────────────────────
//
// 16 types × 16 servers per type. `type_id` is the object type this server serves
// (`crate::object`'s `TYPE_*`), which is what lets a bare `entity_reference` be self-describing.
//
// Allocation convention: hand out `server_reference`s in **increments of 16**, i.e. walk the
// u8 by 0x10 — 0x10, 0x20, 0x30 … That holds `server_id` at 0 and advances `type_id`, so servers
// spread across the type space rather than piling into one type. A second server for a type
// takes `server_id = 1` (0x11, 0x21, …).
//
// Non-data servers (gateway, edge …) may collide with these ids: they don't address objects, so
// a shared id space costs nothing.

const SERVER_REF_TYPE_SHIFT: u8 = 4;
const SERVER_REF_NIBBLE_MASK: u8 = 0xF;

/// The "no server" reference (`type_id 0` = `TYPE_NONE`, `server_id 0`) — the fence
/// `SERVER_NONE`, the system/self actor on injected events, and an *unminted* handle (a fresh
/// spawn not yet bound to a server; the worker routes it to whichever shard processes its event).
pub const SERVER_REF_NONE: u8 = 0;

/// Compose a `server_reference`: `type_id:4 | server_id:4`. `const` so a shard can name its own
/// `SERVER_REFERENCE` in a `const`.
pub const fn pack_server_reference(type_id: u8, server_id: u8) -> u8 {
    ((type_id & SERVER_REF_NIBBLE_MASK) << SERVER_REF_TYPE_SHIFT) | (server_id & SERVER_REF_NIBBLE_MASK)
}

/// The `type_id` (high nibble) — the object type this server serves.
pub fn server_ref_type_id(r: u8) -> u8 {
    (r >> SERVER_REF_TYPE_SHIFT) & SERVER_REF_NIBBLE_MASK
}

/// The `server_id` (low nibble) — which server of that type, within the realm.
pub fn server_ref_server_id(r: u8) -> u8 {
    r & SERVER_REF_NIBBLE_MASK
}

// ── realm_server_reference : u16 = realm_reference:8 | server_reference:8 ────────
//
// The cross-realm form. A bare `server_reference` is realm-scoped — realm is a functional unit
// and every server in one works together — so only traffic that leaves a realm pays these 8 bits.
// (This is what the old u16 `server_reference` was: `realm_id:8 | server_id:8`. It kept realm in
// every reference, everywhere, to serve the rare cross-realm case.)

const REALM_SERVER_SHIFT: u16 = 8;
const REALM_SERVER_BYTE_MASK: u16 = 0xFF;

/// Compose a `realm_server_reference`: `realm_reference:8 | server_reference:8`.
pub fn pack_realm_server_reference(realm_reference: u8, server_reference: u8) -> u16 {
    ((realm_reference as u16) << REALM_SERVER_SHIFT) | (server_reference as u16)
}

/// The `realm_reference` (high byte, `x:4 | y:4`) of a `realm_server_reference`.
pub fn realm_server_ref_realm(r: u16) -> u8 {
    ((r >> REALM_SERVER_SHIFT) & REALM_SERVER_BYTE_MASK) as u8
}

/// The `server_reference` (low byte, `type_id:4 | server_id:4`) of a `realm_server_reference`.
pub fn realm_server_ref_server_reference(r: u16) -> u8 {
    (r & REALM_SERVER_BYTE_MASK) as u8
}

// ── entity_reference : u32 = server_reference:8 | object_reference:24 ────────────
//
// The whole identity. `object_reference` is opaque — a minting server's monotonic counter, no
// interior. `(server_reference, object_reference)` is `(who minted it, which one)`, so it never
// collides within a realm: a server only ever mints its own ids.

const ENTITY_SERVER_SHIFT: u32 = 24;
const ENTITY_SERVER_MASK: u32 = 0xFF;
/// The widest `object_reference` — 24 bits, ~16.7M objects per server.
pub const OBJECT_REF_MAX: u32 = 0x00FF_FFFF;

/// Compose an `entity_reference`: `server_reference:8 | object_reference:24`.
///
/// `object_reference` is masked to 24 bits — a caller that overruns [`OBJECT_REF_MAX`] wraps
/// rather than corrupting the `server_reference` above it.
pub fn pack_entity_reference(server_reference: u8, object_reference: u32) -> u32 {
    ((server_reference as u32) << ENTITY_SERVER_SHIFT) | (object_reference & OBJECT_REF_MAX)
}

/// The qualifying `server_reference` — the server that minted this entity, and (via its
/// `type_id`) what type the entity is. This is what a worker routes by.
pub fn entity_ref_server_reference(k: u32) -> u8 {
    ((k >> ENTITY_SERVER_SHIFT) & ENTITY_SERVER_MASK) as u8
}

/// The `object_reference` (low 24) — the minting server's opaque handle.
pub fn entity_ref_object_reference(k: u32) -> u32 {
    k & OBJECT_REF_MAX
}

/// The entity's `type_id`, read off its server. The reference carries no type of its own; the
/// server it lives on is the answer.
pub fn entity_ref_type_id(k: u32) -> u8 {
    server_ref_type_id(entity_ref_server_reference(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reference_roundtrips() {
        for &(type_id, server_id) in &[(0u8, 0u8), (1, 1), (7, 3), (0xF, 0xF)] {
            let r = pack_server_reference(type_id, server_id);
            assert_eq!(server_ref_type_id(r), type_id);
            assert_eq!(server_ref_server_id(r), server_id);
        }
        // The two nibbles are disjoint and exhaust the byte.
        assert_eq!(pack_server_reference(0xF, 0xF), 0xFF);
        assert_eq!(SERVER_REF_NONE, pack_server_reference(0, 0));
        // Over-range args are masked to their own nibble, never bleed into the other.
        assert_eq!(pack_server_reference(0xFF, 0), 0xF0);
        assert_eq!(pack_server_reference(0, 0xFF), 0x0F);
    }

    #[test]
    fn allocating_in_steps_of_16_walks_the_type_space() {
        // The allocation convention: +0x10 per new server holds server_id at 0 and advances
        // type_id, so the first 16 servers land one per type.
        for n in 0u8..16 {
            let r = n.wrapping_mul(16);
            assert_eq!(server_ref_type_id(r), n);
            assert_eq!(server_ref_server_id(r), 0);
        }
    }

    #[test]
    fn entity_reference_roundtrips() {
        for &(srv, obj) in &[(1u8, 1u32), (0xBE, 12345), (u8::MAX, OBJECT_REF_MAX)] {
            let k = pack_entity_reference(srv, obj);
            assert_eq!(entity_ref_server_reference(k), srv);
            assert_eq!(entity_ref_object_reference(k), obj);
            // The type is the server's — no tag on the reference itself.
            assert_eq!(entity_ref_type_id(k), server_ref_type_id(srv));
        }
    }

    #[test]
    fn fields_do_not_overlap() {
        // server:8 | object:24 — all-ones per field, disjoint, and they exhaust the u32.
        let k = pack_entity_reference(u8::MAX, OBJECT_REF_MAX);
        assert_eq!(entity_ref_server_reference(k), u8::MAX);
        assert_eq!(entity_ref_object_reference(k), OBJECT_REF_MAX);
        assert_eq!(k, u32::MAX);
    }

    #[test]
    fn an_over_range_object_reference_cannot_corrupt_the_server() {
        // 24-bit mask: the 25th bit and up are dropped, not carried into server_reference.
        let k = pack_entity_reference(0x42, u32::MAX);
        assert_eq!(entity_ref_server_reference(k), 0x42);
        assert_eq!(entity_ref_object_reference(k), OBJECT_REF_MAX);
    }

    #[test]
    fn same_object_id_on_different_servers_never_collides() {
        // Uniqueness within a realm rests entirely on the server nibbles: a server only mints
        // its own ids, so the pair (server, object) is unique without any realm field.
        assert_ne!(
            pack_entity_reference(pack_server_reference(1, 0), 7),
            pack_entity_reference(pack_server_reference(2, 0), 7)
        );
        assert_ne!(
            pack_entity_reference(pack_server_reference(1, 0), 7),
            pack_entity_reference(pack_server_reference(1, 1), 7)
        );
    }

    #[test]
    fn realm_server_reference_roundtrips() {
        let srv = pack_server_reference(3, 2);
        for &realm in &[0u8, 0x12, 0xFF] {
            let r = pack_realm_server_reference(realm, srv);
            assert_eq!(realm_server_ref_realm(r), realm);
            assert_eq!(realm_server_ref_server_reference(r), srv);
        }
        assert_eq!(pack_realm_server_reference(0xFF, 0xFF), 0xFFFF);
    }
}
