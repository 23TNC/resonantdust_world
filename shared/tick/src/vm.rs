//! The event-DSL interpreter — the purpose-built stack machine the worker runs over a row's
//! `actions: Vec<u64>` (S1; `docs/spacetime-implementation/s1-interpreter.md`,
//! `docs/spacetime-tables/event-dsl.md`).
//!
//! It decodes words (`op_code` + `payload`) onto a value stack and applies verbs to the
//! target's [`EntityState`]. **Verb effects live in [`crate::domain`]** — the *same* shared
//! functions the edge (validate) and client (predict) call, so prediction matches execution
//! ([risks.md](../../docs/spacetime-tables/risks.md) A2). This is NOT `shared/dsl` (a
//! text/visual VM); the only thing borrowed is the abstract postfix-dispatch shape.
//!
//! Hot path only (MOVE / SPAWN): these read no other entity, so `run` needs no operand
//! reader. Action data (`[u64; 2]`) rides as `LITERAL` words — two per `u64` (lo then hi) —
//! so the whole program stays in the uniform word model. Cross-entity reads (DAMAGE, …) and
//! control flow (AWAIT/SKIP/FAIL) land with S5.

use crate::domain::{self, EntityState, Event};
use resonantdust_codec::event_word::{
    pack_word, word_op_code, word_payload, ACTION_MOVE, ACTION_SPAWN, OP_ACTION, OP_ALIAS,
    OP_LITERAL, OP_OBJECT,
};

/// Map a codec `action_reference` id to this build's [`domain`] action code. The codec ids
/// are the canonical wire palette; `domain`'s `ACTION_*` are the effect implementations.
fn domain_action(codec_action: u32) -> u16 {
    match codec_action {
        ACTION_MOVE => domain::ACTION_MOVE,
        ACTION_SPAWN => domain::ACTION_SPAWN,
        _ => 0, // unknown verb → no-op effect
    }
}

/// Run a row's `actions` program over `base`, producing the target's new [`EntityState`].
/// Hot path: `LITERAL`s push action data; an `ACTION` word pops its data and applies the
/// verb via [`domain::apply_event`]. `OBJECT`/`ALIAS` are accepted but not yet consumed
/// (operand reads + await are S5).
pub fn run(actions: &[u64], base: EntityState) -> EntityState {
    let mut stack: Vec<u64> = Vec::new();
    let mut state = base;
    for &w in actions {
        match word_op_code(w) {
            OP_LITERAL => stack.push(word_payload(w) as u64),
            OP_OBJECT => stack.push(word_payload(w) as u64), // reserved (operand reads: S5)
            OP_ACTION => {
                let action = domain_action(word_payload(w));
                let d1 = pop_u64(&mut stack);
                let d0 = pop_u64(&mut stack);
                let ev = Event { action, actor_key: 0, data: [d0, d1] };
                state = domain::apply_event(state, &ev, None);
            }
            OP_ALIAS => { /* await — resolved at the worker level; no-op in a pure run */ }
            _ => {}
        }
    }
    state
}

/// Pop a `u64` pushed as two `LITERAL`s: `lo` (deeper) then `hi` (top).
fn pop_u64(stack: &mut Vec<u64>) -> u64 {
    let hi = stack.pop().unwrap_or(0);
    let lo = stack.pop().unwrap_or(0);
    (hi << 32) | (lo & 0xFFFF_FFFF)
}

// ── encoders (used by the edge to build word streams — S4) ──────────────────────

fn lit(v: u32) -> u64 {
    pack_word(OP_LITERAL, 0, v)
}

/// Encode `[u64; 2]` action data + the verb as a word stream: `lo,hi` per data word, then
/// the `ACTION`.
fn encode_action(action: u32, data: [u64; 2]) -> Vec<u64> {
    vec![
        lit(data[0] as u32),
        lit((data[0] >> 32) as u32),
        lit(data[1] as u32),
        lit((data[1] >> 32) as u32),
        pack_word(OP_ACTION, 0, action),
    ]
}

/// A `MOVE` program (edge-side builder).
pub fn encode_move(zone_id: u32, location: u8, rotation: u8, offset: u8) -> Vec<u64> {
    encode_action(ACTION_MOVE, domain::pack_move(zone_id, location, rotation, offset))
}

/// A `SPAWN` program (edge-side builder).
pub fn encode_spawn(kind: u16, zone_id: u32, location: u8, rotation: u8, offset: u8) -> Vec<u64> {
    encode_action(ACTION_SPAWN, domain::pack_spawn(kind, zone_id, location, rotation, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The equivalence oracle: a `MOVE` program run over a base equals a direct
    /// `apply_event(MOVE)` with the same params — i.e. the encode/decode roundtrip is lossless.
    #[test]
    fn move_matches_direct_apply() {
        let base = EntityState { kind: 7, zone_id: 3, location: 0x00, rotation: 0, offset: 0, data: [0, 0] };
        let (zone, loc, rot, off) = (3u32, 0x11u8, 0u8, 5u8);
        let got = run(&encode_move(zone, loc, rot, off), base);
        let want = domain::apply_event(
            base,
            &Event { action: domain::ACTION_MOVE, actor_key: 0, data: domain::pack_move(zone, loc, rot, off) },
            None,
        );
        assert_eq!(got, want);
        assert_eq!(got.location, 0x11);
        assert_eq!(got.zone_id, 3);
    }

    #[test]
    fn spawn_matches_direct_apply() {
        let base = EntityState::default();
        let got = run(&encode_spawn(42, 3, 0x22, 1, 0), base);
        let want = domain::apply_event(
            base,
            &Event { action: domain::ACTION_SPAWN, actor_key: 0, data: domain::pack_spawn(42, 3, 0x22, 1, 0) },
            None,
        );
        assert_eq!(got, want);
        assert_eq!(got.kind, 42, "spawn sets kind (non-tombstone)");
        assert_eq!(got.location, 0x22);
    }

    #[test]
    fn u64_data_survives_the_lo_hi_split() {
        // a large zone_id exercises the >32-bit data word.
        let z = 0x00AB_CDEFu32;
        let got = run(&encode_move(z, 0x0F, 0, 0), EntityState { zone_id: z, ..EntityState::default() });
        assert_eq!(got.zone_id, z);
        assert_eq!(got.location, 0x0F);
    }
}
