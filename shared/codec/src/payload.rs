//! The pawn **payload** opcode stream — `pawn.payload/payload_log.payload : Vec<u32>`.
//! Authoritative shape: `docs/TABLES.md § payload_log + payload`.
//!
//! The command-buffer encoding (human-pawns P0, user F1): a flat stream of entries, each a
//! header `opcode:16 | count:16` followed by `count` operand words. A reader skips unknown
//! opcodes by their `count`, so the stream grows new opcodes (inventory, equips, …) without
//! breaking old readers. Opcode ids are APPEND-ONLY, and a reshape RETIRES its value and
//! claims a new one (stat-model I1) — a retired value is never reused, so pre-reshape rows
//! read as unknown entries and are skipped whole.
//!
//! TRAIT/CONDITION rows are the packed gameplay form (stat-model F1):
//! `data:16 | kind:12 | variant:4` ([`crate::object::pack_gameplay_row`]). NEEDS left the
//! payload for the `needs` sub-table (stat-model F2) — need churn fans alone.

/// `PART slot definition_reference` — the FULL def a part slot draws (body = slot 0,
/// head = slot 1). An equip verb later swaps a slot by rewriting its entry.
pub const PAYLOAD_OP_PART: u32 = 1;

// RETIRED opcode values (never reuse): 2 = the f32-era NEED entry (needs moved to the
// `needs` sub-table, stat-model F2); 3 = the f32-era CONDITION entry (`def_ref · grant_tic`,
// replaced by the packed-row shape below).

/// `CONDITION row · written_tic:16` — one STORED (timed) condition (stat-model F1/F3). The
/// row is `remaining_at_write:16 | kind:12 | variant:4`: remaining-now =
/// `remaining_at_write − (now − written_tic)`, expiry DERIVED at read, never stored; a
/// re-grant refreshes the row. DERIVED (band) conditions never appear here — they are
/// computed from need rows by every observer.
pub const PAYLOAD_OP_CONDITION: u32 = 4;

/// `TRAIT row` — one trait/skill the pawn carries (stat-model F1/F5). The row is
/// `level:16 | kind:12 | variant:4`, level ≥ 1 (level 0 = the trait absent — such a row is
/// never stored). Minted at CREATE from the thing def's bindings (F11); static until the
/// grant/revoke verbs (a recorded successor).
pub const PAYLOAD_OP_TRAIT: u32 = 5;

/// Compose one entry's header word: `opcode:16 | count:16`.
pub fn payload_header(opcode: u32, count: u16) -> u32 {
    (opcode << 16) | count as u32
}

/// One `PART` entry, ready to splice into a payload stream.
pub fn part_entry(slot: u8, definition_reference: u32) -> [u32; 3] {
    [payload_header(PAYLOAD_OP_PART, 2), slot as u32, definition_reference]
}

/// One `CONDITION` entry: `[header(2), packed_row, written_tic]`.
pub fn condition_entry(row: u32, written_tic: u16) -> [u32; 3] {
    [payload_header(PAYLOAD_OP_CONDITION, 2), row, written_tic as u32]
}

/// One `TRAIT` entry: `[header(1), packed_row]`.
pub fn trait_entry(row: u32) -> [u32; 2] {
    [payload_header(PAYLOAD_OP_TRAIT, 1), row]
}

/// Decode a payload stream's `PART` entries → `(slot, definition_reference)` pairs, in stream
/// order. Unknown opcodes are skipped by their `count` (forward-compatible); a malformed tail
/// (an entry's count running past the end) stops the scan — everything decoded before it stands.
pub fn payload_parts(payload: &[u32]) -> Vec<(u8, u32)> {
    entries(payload, PAYLOAD_OP_PART, 2).map(|ops| (ops[0] as u8, ops[1])).collect()
}

/// Decode a payload's `CONDITION` entries → `(packed_row, written_tic)`, stream order.
/// Tolerant like [`payload_parts`]: unknown opcodes skip by count, a malformed tail stops,
/// and an entry whose count is not this layout's is IGNORED (an old-shape row reads as "no
/// row", never as garbage — the re-mint posture, stat-model I1).
pub fn payload_conditions(payload: &[u32]) -> Vec<(u32, u16)> {
    entries(payload, PAYLOAD_OP_CONDITION, 2).map(|ops| (ops[0], ops[1] as u16)).collect()
}

/// Decode a payload's `TRAIT` entries → packed rows, stream order.
pub fn payload_traits(payload: &[u32]) -> Vec<u32> {
    entries(payload, PAYLOAD_OP_TRAIT, 1).map(|ops| ops[0]).collect()
}

/// Upsert one stored-condition row: identity is the row's LOW 16 (kind|variant), so a
/// re-grant with a different `remaining_at_write` still refreshes the same entry.
pub fn upsert_condition(payload: &mut Vec<u32>, row: u32, written_tic: u16) {
    upsert(payload, PAYLOAD_OP_CONDITION, row & 0xFFFF, 0xFFFF, &condition_entry(row, written_tic));
}

/// Upsert one trait row by its LOW 16 — a level change rewrites the same entry.
pub fn upsert_trait(payload: &mut Vec<u32>, row: u32) {
    upsert(payload, PAYLOAD_OP_TRAIT, row & 0xFFFF, 0xFFFF, &trait_entry(row));
}

/// Iterate the OPERAND SLICES of every `opcode` entry with exactly `count` operands. Entries
/// of the same opcode with a different count are skipped whole — the layout-version guard.
fn entries(payload: &[u32], opcode: u32, count: usize) -> impl Iterator<Item = &[u32]> + '_ {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let n = (header & 0xFFFF) as usize;
        let end = i + 1 + n;
        if end > payload.len() {
            break; // malformed tail — keep what decoded
        }
        if header >> 16 == opcode && n == count {
            out.push(&payload[i + 1..end]);
        }
        i = end;
    }
    out.into_iter()
}

/// Rewrite the entry of `opcode` (at this layout's arity) whose FIRST operand matches `key`
/// under `mask`, else append `entry`. An old-shape entry (same opcode, different count) is
/// left alone — the reader ignores it and the fresh entry wins.
fn upsert(payload: &mut Vec<u32>, opcode: u32, key: u32, mask: u32, entry: &[u32]) {
    let count = entry.len() - 1;
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let n = (header & 0xFFFF) as usize;
        let end = i + 1 + n;
        if end > payload.len() {
            break;
        }
        if header >> 16 == opcode && n == count && payload[i + 1] & mask == key {
            payload[i..end].copy_from_slice(entry);
            return;
        }
        i = end;
    }
    payload.extend_from_slice(entry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::pack_gameplay_row;

    const QUENCHED: u32 = 0x8002_0030; // gameplay/condition/quenched — a registry ref
    const THIRSTY: u32 = 0x8002_0010;
    const WALKS: u32 = 0x8003_0020; // gameplay/trait/walks

    #[test]
    fn condition_rows_refresh_by_key_across_remaining_changes() {
        // stat-model F3: identity is kind|variant — a re-grant carries a DIFFERENT
        // remaining_at_write in the same word and must still rewrite the same entry.
        let mut p = Vec::new();
        upsert_condition(&mut p, pack_gameplay_row(QUENCHED, 3600), 500);
        let len_once = p.len();
        upsert_condition(&mut p, pack_gameplay_row(QUENCHED, 3600), 800);
        assert_eq!(p.len(), len_once, "a re-grant rewrites in place");
        upsert_condition(&mut p, pack_gameplay_row(THIRSTY, 100), 810);
        assert_eq!(
            payload_conditions(&p),
            vec![(pack_gameplay_row(QUENCHED, 3600), 800), (pack_gameplay_row(THIRSTY, 100), 810)]
        );
    }

    #[test]
    fn trait_rows_upsert_by_key_so_a_level_change_rewrites() {
        let mut p = Vec::from(part_entry(0, 0x3001_0027));
        upsert_trait(&mut p, pack_gameplay_row(WALKS, 1));
        upsert_trait(&mut p, pack_gameplay_row(WALKS, 2)); // the skill levels up
        assert_eq!(payload_traits(&p), vec![pack_gameplay_row(WALKS, 2)]);
        assert_eq!(payload_parts(&p), vec![(0, 0x3001_0027)], "PART entries survive");
    }

    #[test]
    fn retired_opcode_values_read_as_no_rows() {
        // Pre-reshape entries under the RETIRED values 2 (NEED) and 3 (old CONDITION) must
        // be skipped whole — never decoded by the new readers (stat-model I1).
        let mut p = vec![
            payload_header(2, 3), 0x8001_0010, 42.5f32.to_bits(), 900, // old NEED
            payload_header(3, 2), QUENCHED, 500,                       // old CONDITION
        ];
        assert_eq!(payload_conditions(&p), Vec::<(u32, u16)>::new());
        assert_eq!(payload_traits(&p), Vec::<u32>::new());
        // ... and fresh entries land beside them untouched.
        upsert_condition(&mut p, pack_gameplay_row(QUENCHED, 3600), 700);
        assert_eq!(payload_conditions(&p), vec![(pack_gameplay_row(QUENCHED, 3600), 700)]);
        assert_eq!(p[0], payload_header(2, 3), "the old entries are untouched");
    }

    #[test]
    fn two_part_entries_round_trip() {
        let mut p = Vec::new();
        p.extend_from_slice(&part_entry(0, 0x3001_0027)); // body def
        p.extend_from_slice(&part_entry(1, 0x3001_002B)); // head def
        assert_eq!(payload_parts(&p), vec![(0, 0x3001_0027), (1, 0x3001_002B)]);
    }

    #[test]
    fn unknown_opcodes_are_skipped_by_count() {
        // [future-op count=3 a b c] [PART slot def] — the reader hops the unknown entry.
        let mut p = vec![payload_header(0xBEEF, 3), 1, 2, 3];
        p.extend_from_slice(&part_entry(1, 42));
        assert_eq!(payload_parts(&p), vec![(1, 42)]);
    }

    #[test]
    fn a_malformed_tail_keeps_the_decoded_prefix() {
        let mut p = Vec::from(part_entry(0, 7));
        p.push(payload_header(PAYLOAD_OP_PART, 2)); // claims 2 operands, stream ends
        p.push(9);
        assert_eq!(payload_parts(&p), vec![(0, 7)]);
    }

    #[test]
    fn empty_payload_decodes_empty() {
        assert_eq!(payload_parts(&[]), Vec::<(u8, u32)>::new());
    }
}
