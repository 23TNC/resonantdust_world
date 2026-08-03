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

/// `NEED word` — one need's row, word = `need_id:8 | satisfaction:8 | set_tic:16`
/// (needs-moodlets F7). Satisfaction is quantised `0..=255` (`255` = full `1.0`); nothing
/// ever ticks it — observers compute `satisfaction_at(tic)` from the corpus `deplete`
/// rate (F4), so the entry rewrites only when something HAPPENS (mint, drink, a drill's
/// forced set). One entry per need; `SET_NEED` upserts by `need_id`.
pub const PAYLOAD_OP_NEED: u32 = 2;

/// `MOODLET word` — one STORED (timed) moodlet grant, word = `moodlet_id:8 | 0:8 |
/// grant_tic:16` (needs-moodlets F2/F7). Expiry is DERIVED — `grant_tic + duration`
/// from the corpus — never stored; a re-grant refreshes the timer by rewriting the
/// entry. Conditional (band) moodlets never appear here — they are derived from NEED
/// entries by every observer.
pub const PAYLOAD_OP_MOODLET: u32 = 3;

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

/// Pack one need's payload word: `need_id:8 | satisfaction:8 | set_tic:16`.
pub fn need_word(need_id: u8, satisfaction: u8, set_tic: u16) -> u32 {
    ((need_id as u32) << 24) | ((satisfaction as u32) << 16) | set_tic as u32
}

/// Pack one stored-moodlet word: `moodlet_id:8 | 0:8 | grant_tic:16`.
pub fn moodlet_word(moodlet_id: u8, grant_tic: u16) -> u32 {
    ((moodlet_id as u32) << 24) | grant_tic as u32
}

/// Decode a payload's `NEED` entries → `(need_id, satisfaction, set_tic)`, stream order.
/// Tolerant like [`payload_parts`]: unknown opcodes skip by count, a malformed tail stops.
pub fn payload_needs(payload: &[u32]) -> Vec<(u8, u8, u16)> {
    scan(payload, PAYLOAD_OP_NEED)
        .map(|w| ((w >> 24) as u8, (w >> 16) as u8, w as u16))
        .collect()
}

/// Decode a payload's `MOODLET` entries → `(moodlet_id, grant_tic)`, stream order.
pub fn payload_moodlets(payload: &[u32]) -> Vec<(u8, u16)> {
    scan(payload, PAYLOAD_OP_MOODLET).map(|w| ((w >> 24) as u8, w as u16)).collect()
}

/// Upsert one need's entry in place: rewrite the word whose `need_id` matches, else append
/// a fresh `NEED` entry. The pawn module's `set_need` reducer is the writer (F7).
pub fn upsert_need(payload: &mut Vec<u32>, need_id: u8, satisfaction: u8, set_tic: u16) {
    upsert(payload, PAYLOAD_OP_NEED, need_id, need_word(need_id, satisfaction, set_tic));
}

/// Upsert one stored-moodlet grant: a re-grant of the same moodlet refreshes its timer.
pub fn upsert_moodlet(payload: &mut Vec<u32>, moodlet_id: u8, grant_tic: u16) {
    upsert(payload, PAYLOAD_OP_MOODLET, moodlet_id, moodlet_word(moodlet_id, grant_tic));
}

/// Iterate every operand word of `opcode`'s entries (an entry may carry several words —
/// readers stay layout-tolerant even though the writers emit one word per entry).
fn scan(payload: &[u32], opcode: u32) -> impl Iterator<Item = u32> + '_ {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let count = (header & 0xFFFF) as usize;
        let end = i + 1 + count;
        if end > payload.len() {
            break;
        }
        if header >> 16 == opcode {
            out.extend_from_slice(&payload[i + 1..end]);
        }
        i = end;
    }
    out.into_iter()
}

/// Rewrite the first word of `opcode` whose top byte is `id`, else append `[header(1), word]`.
fn upsert(payload: &mut Vec<u32>, opcode: u32, id: u8, word: u32) {
    let mut i = 0usize;
    while i < payload.len() {
        let header = payload[i];
        let count = (header & 0xFFFF) as usize;
        let end = i + 1 + count;
        if end > payload.len() {
            break;
        }
        if header >> 16 == opcode {
            for w in &mut payload[i + 1..end] {
                if (*w >> 24) as u8 == id {
                    *w = word;
                    return;
                }
            }
        }
        i = end;
    }
    payload.push(payload_header(opcode, 1));
    payload.push(word);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_entries_upsert_and_decode() {
        // needs-moodlets F7: a mint writes thirst full; a later set REWRITES the same
        // word (no growth); a second need appends without touching the first — and PART
        // entries pass through untouched.
        let mut p = Vec::from(part_entry(0, 0x3001_0027));
        upsert_need(&mut p, 1, 255, 100);
        let len_once = p.len();
        upsert_need(&mut p, 1, 40, 900);
        assert_eq!(p.len(), len_once, "an upsert of the same need rewrites in place");
        upsert_need(&mut p, 2, 200, 901);
        assert_eq!(payload_needs(&p), vec![(1, 40, 900), (2, 200, 901)]);
        assert_eq!(payload_parts(&p), vec![(0, 0x3001_0027)], "PART entries survive");
    }

    #[test]
    fn moodlet_grants_refresh_by_id() {
        let mut p = Vec::new();
        upsert_moodlet(&mut p, 3, 500);
        upsert_moodlet(&mut p, 3, 800); // re-grant → refreshed timer, same entry
        upsert_moodlet(&mut p, 4, 810);
        assert_eq!(payload_moodlets(&p), vec![(3, 800), (4, 810)]);
        assert_eq!(payload_needs(&p), Vec::<(u8, u8, u16)>::new());
    }

    #[test]
    fn need_word_packs_the_documented_lanes() {
        let w = need_word(0x0A, 0x80, 0x1234);
        assert_eq!(w, 0x0A80_1234);
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
