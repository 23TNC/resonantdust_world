//! The composite row keys of the rebuild's shards — `state_uid` and `event_uid`.
//! Authoritative layout: `docs/VARIABLES.md`.
//!
//! Both are `u64` PKs that *are* their identity (not surrogates), so the fields read straight off
//! the key. A subscription still filters on the separate columns (`entity_reference`, `tic`,
//! `macro_position_reference`) — a packed key isn't filterable — but a reducer that has the `uid`
//! reads the parts here without a second lookup.

// ── state_uid : u64 = reserved:16 | entity_reference:32 | tic:16 ─────────────────
//
// Entity-major: `entity_reference` above `tic`, so one entity's slots are contiguous (what the
// per-entity block scan needs). `tic` is low because it wraps — no key order can give a wrapping
// tic a sound range, so nothing is lost by putting it there. See `docs/notes/tables.md`.

const STATE_ENTITY_SHIFT: u64 = 16;
const STATE_ENTITY_MASK: u64 = 0xFFFF_FFFF;
const STATE_TIC_MASK: u64 = 0xFFFF;

/// Compose a `state_uid` from its `(entity, tic)` — the `state_log` PK.
pub fn pack_state_uid(entity_reference: u32, tic: u16) -> u64 {
    ((entity_reference as u64) << STATE_ENTITY_SHIFT) | (tic as u64)
}

/// The `entity_reference` (bits 16–47) of a `state_uid`.
pub fn state_uid_entity_reference(uid: u64) -> u32 {
    ((uid >> STATE_ENTITY_SHIFT) & STATE_ENTITY_MASK) as u32
}

/// The `tic` (low 16) of a `state_uid`.
pub fn state_uid_tic(uid: u64) -> u16 {
    (uid & STATE_TIC_MASK) as u16
}

// ── event_uid : u64 = macro_position_reference:16 | event_tic:16 | event_reference:32 ──
//
// The PK of the settled `event` log — one row per zone an event's targets occupy, so
// `event_reference` alone is not unique here; the `uid` is. `macro_position_reference` on top makes
// a zone's events contiguous, though the client filters by the separate column.

const EVENT_MACRO_SHIFT: u64 = 48;
const EVENT_TIC_SHIFT: u64 = 32;
const EVENT_U16_MASK: u64 = 0xFFFF;
const EVENT_REF_MASK: u64 = 0xFFFF_FFFF;

/// Compose an `event_uid` — the `event` PK.
pub fn pack_event_uid(macro_position_reference: u16, event_tic: u16, event_reference: u32) -> u64 {
    ((macro_position_reference as u64) << EVENT_MACRO_SHIFT)
        | ((event_tic as u64) << EVENT_TIC_SHIFT)
        | (event_reference as u64)
}

/// The `macro_position_reference` (bits 48–63) of an `event_uid`.
pub fn event_uid_macro_position(uid: u64) -> u16 {
    ((uid >> EVENT_MACRO_SHIFT) & EVENT_U16_MASK) as u16
}

/// The `event_tic` (bits 32–47) of an `event_uid`.
pub fn event_uid_event_tic(uid: u64) -> u16 {
    ((uid >> EVENT_TIC_SHIFT) & EVENT_U16_MASK) as u16
}

/// The `event_reference` (low 32) of an `event_uid`.
pub fn event_uid_event_reference(uid: u64) -> u32 {
    (uid & EVENT_REF_MASK) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_uid_roundtrips() {
        for &(e, t) in &[(0u32, 0u16), (1, 1), (0x00AB_CDEF, 0x1234), (u32::MAX, u16::MAX)] {
            let uid = pack_state_uid(e, t);
            assert_eq!(state_uid_entity_reference(uid), e);
            assert_eq!(state_uid_tic(uid), t);
        }
    }

    #[test]
    fn state_uid_fields_are_disjoint_and_reserved_stays_zero() {
        // entity:32 | tic:16 fill bits 0..48; reserved:16 (48..64) stays 0.
        let uid = pack_state_uid(u32::MAX, u16::MAX);
        assert_eq!(uid, 0x0000_FFFF_FFFF_FFFF);
        assert_eq!(pack_state_uid(u32::MAX, 0), 0x0000_FFFF_FFFF_0000);
        assert_eq!(pack_state_uid(0, u16::MAX), 0x0000_0000_0000_FFFF);
    }

    #[test]
    fn event_uid_roundtrips() {
        for &(m, t, e) in &[(0u16, 0u16, 0u32), (0x1122, 0x3344, 0x5566_7788), (u16::MAX, u16::MAX, u32::MAX)] {
            let uid = pack_event_uid(m, t, e);
            assert_eq!(event_uid_macro_position(uid), m);
            assert_eq!(event_uid_event_tic(uid), t);
            assert_eq!(event_uid_event_reference(uid), e);
        }
    }

    #[test]
    fn event_uid_fields_are_disjoint_and_exhaust_the_word() {
        // macro:16 | tic:16 | event:32 exhaust the u64.
        assert_eq!(pack_event_uid(u16::MAX, u16::MAX, u32::MAX), u64::MAX);
        assert_eq!(pack_event_uid(u16::MAX, 0, 0), 0xFFFF_0000_0000_0000);
        assert_eq!(pack_event_uid(0, u16::MAX, 0), 0x0000_FFFF_0000_0000);
        assert_eq!(pack_event_uid(0, 0, u32::MAX), 0x0000_0000_FFFF_FFFF);
    }
}
