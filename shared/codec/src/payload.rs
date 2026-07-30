//! The pawn **payload** opcode stream — `pawn.payload/payload_log.payload : Vec<u32>`.
//! Authoritative shape: `docs/TABLES.md § payload_log + payload`.
//!
//! The command-buffer encoding (human-pawns P0, user F1): a flat stream of entries, each a
//! header `opcode:16 | count:16` followed by `count` operand words. A reader skips unknown
//! opcodes by their `count`, so the stream grows new opcodes (inventory, stats, needs, …)
//! without breaking old readers. Opcode ids are APPEND-ONLY.

/// `PART slot definition_reference` — the FULL def a part slot draws (body = slot 0,
/// head = slot 1). An equip verb later swaps a slot by rewriting its entry.
pub const PAYLOAD_OP_PART: u32 = 1;

/// Compose one entry's header word: `opcode:16 | count:16`.
pub fn payload_header(opcode: u32, count: u16) -> u32 {
    (opcode << 16) | count as u32
}

/// One `PART` entry, ready to splice into a payload stream.
pub fn part_entry(slot: u8, definition_reference: u32) -> [u32; 3] {
    [payload_header(PAYLOAD_OP_PART, 2), slot as u32, definition_reference]
}

/// Decode a payload stream's `PART` entries → `(slot, definition_reference)` pairs, in stream
/// order. Unknown opcodes are skipped by their `count` (forward-compatible); a malformed tail
/// (an entry's count running past the end) stops the scan — everything decoded before it stands.
pub fn payload_parts(payload: &[u32]) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let opcode = header >> 16;
        let count = (header & 0xFFFF) as usize;
        let end = i + 1 + count;
        if end > payload.len() {
            break; // malformed tail — keep what decoded
        }
        if opcode == PAYLOAD_OP_PART && count == 2 {
            out.push((payload[i + 1] as u8, payload[i + 2]));
        }
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
