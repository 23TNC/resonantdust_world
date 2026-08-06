//! The pawn **payload** opcode stream — `pawn.payload/payload_log.payload : Vec<u32>`.
//! Authoritative shape: `docs/TABLES.md § payload_log + payload`.
//!
//! The command-buffer encoding (human-pawns P0, user F1): a flat stream of entries, each a
//! header `opcode:16 | count:16` followed by `count` operand words. A reader skips unknown
//! opcodes by their `count`, so the stream grows new opcodes (inventory, stats, needs, …)
//! without breaking old readers. Opcode ids are APPEND-ONLY.
//!
//! NEED/CONDITION reshaped by `2026-08-06-interactions` (F1/F3/I1): ids are full u32
//! gameplay `definition_reference`s and satisfaction is an f32 bit pattern — NOT
//! read-compatible with the old packed-`u8`-id single words; dev pawns re-mint.

/// `PART slot definition_reference` — the FULL def a part slot draws (body = slot 0,
/// head = slot 1). An equip verb later swaps a slot by rewriting its entry.
pub const PAYLOAD_OP_PART: u32 = 1;

/// `NEED need:definition_reference · satisfaction (f32 BITS) · set_tic:16` — one need's row
/// (needs-moodlets F7, interactions I1). Satisfaction lives on the need's authored `min..max`
/// domain (interactions F7); nothing ever ticks it — observers compute `satisfaction_at(tic)`
/// from the corpus `deplete` rate (F4), so the entry rewrites only when something HAPPENS
/// (mint, drink, a drill's forced set). One entry per need; `SET_NEED` upserts by the def ref.
pub const PAYLOAD_OP_NEED: u32 = 2;

/// `CONDITION condition:definition_reference · grant_tic:16` — one STORED (timed) condition
/// grant (needs-moodlets F2/F7, interactions I1). Expiry is DERIVED — `grant_tic + duration`
/// from the corpus — never stored; a re-grant refreshes the timer by rewriting the entry.
/// DERIVED (band) conditions never appear here — they are computed from NEED entries by
/// every observer.
pub const PAYLOAD_OP_CONDITION: u32 = 3;

/// Compose one entry's header word: `opcode:16 | count:16`.
pub fn payload_header(opcode: u32, count: u16) -> u32 {
    (opcode << 16) | count as u32
}

/// One `PART` entry, ready to splice into a payload stream.
pub fn part_entry(slot: u8, definition_reference: u32) -> [u32; 3] {
    [payload_header(PAYLOAD_OP_PART, 2), slot as u32, definition_reference]
}

/// One `NEED` entry: `[header(3), need_ref, satisfaction.to_bits(), set_tic]`.
pub fn need_entry(need: u32, satisfaction: f32, set_tic: u16) -> [u32; 4] {
    [payload_header(PAYLOAD_OP_NEED, 3), need, satisfaction.to_bits(), set_tic as u32]
}

/// One `CONDITION` entry: `[header(2), condition_ref, grant_tic]`.
pub fn condition_entry(condition: u32, grant_tic: u16) -> [u32; 3] {
    [payload_header(PAYLOAD_OP_CONDITION, 2), condition, grant_tic as u32]
}

/// Decode a payload stream's `PART` entries → `(slot, definition_reference)` pairs, in stream
/// order. Unknown opcodes are skipped by their `count` (forward-compatible); a malformed tail
/// (an entry's count running past the end) stops the scan — everything decoded before it stands.
pub fn payload_parts(payload: &[u32]) -> Vec<(u8, u32)> {
    entries(payload, PAYLOAD_OP_PART, 2).map(|ops| (ops[0] as u8, ops[1])).collect()
}

/// Decode a payload's `NEED` entries → `(need_ref, satisfaction, set_tic)`, stream order.
/// Tolerant like [`payload_parts`]: unknown opcodes skip by count, a malformed tail stops,
/// and an entry whose count is not this layout's is IGNORED (an old-shape row reads as "no
/// row", never as garbage — the re-mint posture, interactions I1).
pub fn payload_needs(payload: &[u32]) -> Vec<(u32, f32, u16)> {
    entries(payload, PAYLOAD_OP_NEED, 3)
        .map(|ops| (ops[0], f32::from_bits(ops[1]), ops[2] as u16))
        .collect()
}

/// Decode a payload's `CONDITION` entries → `(condition_ref, grant_tic)`, stream order.
pub fn payload_conditions(payload: &[u32]) -> Vec<(u32, u16)> {
    entries(payload, PAYLOAD_OP_CONDITION, 2).map(|ops| (ops[0], ops[1] as u16)).collect()
}

/// Upsert one need's entry in place: rewrite the entry whose def ref matches, else append a
/// fresh `NEED` entry. The pawn module's `set_need` reducer is the writer (F7).
pub fn upsert_need(payload: &mut Vec<u32>, need: u32, satisfaction: f32, set_tic: u16) {
    upsert(payload, PAYLOAD_OP_NEED, need, &need_entry(need, satisfaction, set_tic));
}

/// Upsert one stored-condition grant: a re-grant of the same condition refreshes its timer.
pub fn upsert_condition(payload: &mut Vec<u32>, condition: u32, grant_tic: u16) {
    upsert(payload, PAYLOAD_OP_CONDITION, condition, &condition_entry(condition, grant_tic));
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

/// Rewrite the entry of `opcode` (at this layout's arity) whose FIRST operand is `key`, else
/// append `entry`. An old-shape entry (same opcode, different count) is left alone — the
/// reader ignores it and the fresh entry wins.
fn upsert(payload: &mut Vec<u32>, opcode: u32, key: u32, entry: &[u32]) {
    let count = entry.len() - 1;
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let n = (header & 0xFFFF) as usize;
        let end = i + 1 + n;
        if end > payload.len() {
            break;
        }
        if header >> 16 == opcode && n == count && payload[i + 1] == key {
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

    const THIRST: u32 = 0x8001_0010; // gameplay/need/thirst — an arbitrary registry ref
    const HUNGER: u32 = 0x8001_0020;
    const QUENCHED: u32 = 0x8002_0030; // gameplay/condition/quenched

    #[test]
    fn need_entries_upsert_and_decode() {
        // needs-moodlets F7: a mint writes thirst full; a later set REWRITES the same
        // entry (no growth); a second need appends without touching the first — and PART
        // entries pass through untouched.
        let mut p = Vec::from(part_entry(0, 0x3001_0027));
        upsert_need(&mut p, THIRST, 100.0, 100);
        let len_once = p.len();
        upsert_need(&mut p, THIRST, 40.5, 900);
        assert_eq!(p.len(), len_once, "an upsert of the same need rewrites in place");
        upsert_need(&mut p, HUNGER, 80.0, 901);
        assert_eq!(payload_needs(&p), vec![(THIRST, 40.5, 900), (HUNGER, 80.0, 901)]);
        assert_eq!(payload_parts(&p), vec![(0, 0x3001_0027)], "PART entries survive");
    }

    #[test]
    fn condition_grants_refresh_by_ref() {
        let mut p = Vec::new();
        upsert_condition(&mut p, QUENCHED, 500);
        upsert_condition(&mut p, QUENCHED, 800); // re-grant → refreshed timer, same entry
        upsert_condition(&mut p, QUENCHED + 1, 810);
        assert_eq!(payload_conditions(&p), vec![(QUENCHED, 800), (QUENCHED + 1, 810)]);
        assert_eq!(payload_needs(&p), Vec::<(u32, f32, u16)>::new());
    }

    #[test]
    fn satisfaction_round_trips_exact_bits() {
        // f32 BITS in the u32 lane (interactions F3) — including a negative deficit value.
        let mut p = Vec::new();
        upsert_need(&mut p, THIRST, -12.25, 7);
        assert_eq!(payload_needs(&p), vec![(THIRST, -12.25, 7)]);
    }

    #[test]
    fn an_old_shape_need_word_is_ignored_not_misread() {
        // The pre-interactions single-word NEED entry (count 1): the reader must skip it
        // whole (re-mint posture, I1), never decode its word as a def ref.
        let mut p = vec![payload_header(PAYLOAD_OP_NEED, 1), 0x0A80_1234];
        assert_eq!(payload_needs(&p), Vec::<(u32, f32, u16)>::new());
        // ... and an upsert appends the NEW shape beside it rather than corrupting it.
        upsert_need(&mut p, THIRST, 55.0, 3);
        assert_eq!(payload_needs(&p), vec![(THIRST, 55.0, 3)]);
        assert_eq!(p[0], payload_header(PAYLOAD_OP_NEED, 1), "the old entry is untouched");
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
