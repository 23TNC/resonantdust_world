//! The packed word of an event's `actions: Vec<u64>` stack program — the event DSL's
//! wire unit. Canonical design: `docs/components/server/spacetime/modules/shard/design/event-dsl.md`.
//!
//! Each `u64` is one stack instruction. The `op_code` (top nibble) says what the word
//! *is*; the low 48 bits are a clean `server_reference:16 | payload:32` qualified
//! reference, extractable with a plain mask — the `op_code`/reserved tag sits on top so
//! it never disturbs them.
//!
//! ```text
//! u64 word (high → low)
//! ┌──────────┬──────────────┬───────────────────┬────────────────────────┐
//! │op_code:4 │ reserved:12  │ server_reference:16│      payload : 32      │
//! └──────────┴──────────────┴───────────────────┴────────────────────────┘
//!  bits 60–63   48–59            32–47                bits 0–31
//! ```
//!
//! `op_code` (named `op_code`, *not* `kind`, so it never reads as the object model's
//! `kind_reference`):
//! - `LITERAL` — payload is an immediate `u32`; push the value.
//! - `OBJECT`  — payload is an `object_reference` (a `cold_reference` or `hot_reference`),
//!   qualified by `server_reference`; push the reference.
//! - `ACTION`  — payload is an `action_reference` (the verb); `server_reference` is the
//!   issuer; pop the verb's operands and run it.
//! - `ALIAS`   — payload is an `event_reference` (a row), its shard in `server_reference`;
//!   push it for `AWAIT` to test.
//!
//! The specific verb (`MOVE` vs `INSPECT`) lives in an `ACTION` word's payload; the
//! `op_code` only says "this word is an action".

// ── the word frame ─────────────────────────────────────────────────────────────

const OP_CODE_SHIFT: u64 = 60;
const OP_CODE_MASK: u64 = 0xF; // 4 bits, bits 60–63
const SERVER_REFERENCE_SHIFT: u64 = 32;
const SERVER_REFERENCE_MASK: u64 = 0xFFFF; // 16 bits, bits 32–47
const PAYLOAD_MASK: u64 = 0xFFFF_FFFF; // 32 bits, bits 0–31
// bits 48–59 are reserved (12 bits), left 0.

/// `op_code = LITERAL`: payload is an immediate `u32` constant — push its value.
pub const OP_LITERAL: u8 = 0;
/// `op_code = OBJECT`: payload is a `u32` object_reference — push the (qualified) reference.
pub const OP_OBJECT: u8 = 1;
/// `op_code = ACTION`: payload is a `u32` action_reference — pop the verb's args + execute.
pub const OP_ACTION: u8 = 2;
/// `op_code = ALIAS`: payload is a `u32` event_reference (a row) — push it for `AWAIT`.
pub const OP_ALIAS: u8 = 3;

/// Compose a word: `op_code:4 | reserved:12 | server_reference:16 | payload:32`. `op_code`
/// is masked to 4 bits; the reserved 12 stay 0.
pub fn pack_word(op_code: u8, server_reference: u16, payload: u32) -> u64 {
    (((op_code as u64) & OP_CODE_MASK) << OP_CODE_SHIFT)
        | ((server_reference as u64) << SERVER_REFERENCE_SHIFT)
        | (payload as u64)
}

/// The `op_code` (top nibble) of a word.
pub fn word_op_code(w: u64) -> u8 {
    ((w >> OP_CODE_SHIFT) & OP_CODE_MASK) as u8
}

/// The `server_reference` (bits 32–47) of a word — the shard the payload lives on / the
/// issuer, per `op_code`. `(w >> 32) & 0xFFFF`, undisturbed by the tag.
pub fn word_server_reference(w: u64) -> u16 {
    ((w >> SERVER_REFERENCE_SHIFT) & SERVER_REFERENCE_MASK) as u16
}

/// The `payload` (low 32 bits) of a word — a literal / object_reference / action_reference /
/// event_reference per `op_code`. `w & 0xFFFF_FFFF`, undisturbed by the tag.
pub fn word_payload(w: u64) -> u32 {
    (w & PAYLOAD_MASK) as u32
}

// ── action_reference op-id palette ─────────────────────────────────────────────
//
// The `payload` of an `OP_ACTION` word. Append-only — a new verb goes last so stored
// programs never renumber. `0` is the null/unset sentinel. Verb *effects* live in a
// shared crate (worker/edge/client share them — `event-dsl.md`); these are just the ids.

/// Reserved null/unset action.
pub const ACTION_NONE: u32 = 0;
/// Move a hot pawn to a tile.
pub const ACTION_MOVE: u32 = 1;
/// Materialize an entity from nothing (mint a fresh hot object).
pub const ACTION_SPAWN: u32 = 2;
/// Act on a hot object (read/interact).
pub const ACTION_INSPECT: u32 = 3;
/// Settle a hot object back to cold + compact (`docs/components/server/spacetime/modules/shard/intent/hot-cold.md`).
pub const ACTION_PACK: u32 = 4;
/// Block this row on an `ALIAS` (event_reference) completing, `LITERAL` tic budget.
pub const ACTION_AWAIT: u32 = 5;
/// Forward-only branch: skip a `LITERAL` word-count (the `? :` encoding, D1).
pub const ACTION_SKIP: u32 = 6;
/// Abort this row / branch.
pub const ACTION_FAIL: u32 = 7;
/// Deal damage to the target: reads an `OBJECT` actor operand (its hp) + a `LITERAL` amount; a
/// *live* actor subtracts the amount from the target's hp. The first actor-reading verb.
pub const ACTION_DAMAGE: u32 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_roundtrips() {
        for &(op, s, p) in &[
            (OP_LITERAL, 0u16, 0u32),
            (OP_OBJECT, 1, 1),
            (OP_ACTION, 0xABCD, 0x0123_4567),
            (OP_ALIAS, 0xFFFF, 0xFFFF_FFFF),
            (0xF, 0x8000, 0x8000_0000),
        ] {
            let w = pack_word(op, s, p);
            assert_eq!(word_op_code(w), op & 0xF);
            assert_eq!(word_server_reference(w), s);
            assert_eq!(word_payload(w), p);
        }
    }

    #[test]
    fn all_ones_per_field_disjoint() {
        // op_code:4 | reserved:12(=0) | server:16 | payload:32
        assert_eq!(pack_word(0xF, 0xFFFF, 0xFFFF_FFFF), 0xF000_FFFF_FFFF_FFFF);
        // op_code alone
        assert_eq!(pack_word(0xF, 0, 0), 0xF000_0000_0000_0000);
        // server alone
        assert_eq!(pack_word(0, 0xFFFF, 0), 0x0000_FFFF_0000_0000);
        // payload alone
        assert_eq!(pack_word(0, 0, 0xFFFF_FFFF), 0x0000_0000_FFFF_FFFF);
    }

    #[test]
    fn low_48_bits_are_a_clean_qualified_reference() {
        // The tag on top must not disturb the low 48 bits: they are exactly
        // `server_reference:16 | payload:32`, plain-mask extractable regardless of op_code.
        let s = 0xBEEFu16;
        let p = 0xDEAD_C0DEu32;
        for op in 0u8..16 {
            let w = pack_word(op, s, p);
            assert_eq!(w & 0xFFFF_FFFF, p as u64, "payload = w & 0xFFFF_FFFF");
            assert_eq!((w >> 32) & 0xFFFF, s as u64, "server = (w>>32) & 0xFFFF");
        }
    }

    #[test]
    fn op_codes_and_actions_are_distinct() {
        let ops = [OP_LITERAL, OP_OBJECT, OP_ACTION, OP_ALIAS];
        for (i, &o) in ops.iter().enumerate() {
            assert_eq!(o as usize, i, "op_codes contiguous from 0");
            assert!((o as u64) <= OP_CODE_MASK, "fits the 4-bit tag");
        }
        let actions = [
            ACTION_NONE,
            ACTION_MOVE,
            ACTION_SPAWN,
            ACTION_INSPECT,
            ACTION_PACK,
            ACTION_AWAIT,
            ACTION_SKIP,
            ACTION_FAIL,
        ];
        for (i, &a) in actions.iter().enumerate() {
            assert_eq!(a as usize, i, "action ids contiguous, append-only from 0");
        }
    }
}
