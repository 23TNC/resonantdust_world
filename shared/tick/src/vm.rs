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
//! Verbs so far: `MOVE`/`SPAWN` (data-carrying, no read) and `DAMAGE` (actor-reading — an
//! `OBJECT` operand resolves to the actor's hp via [`Reads`], and a live actor's blow lands).
//! Action data (`[u64; 2]`) rides as `LITERAL` words — two per `u64` (lo then hi). Control flow:
//! `AWAIT` is a worker-level gate ([`await_gate`]); within a program, the **forward-only `SKIP`**
//! encodes `? then : else` ([`encode_if`]) and `FAIL` aborts — no backward jumps, so a program is
//! bounded. A program may carry **multiple action words** (a multi-verb row: e.g. `SPAWN` then
//! `MOVE`), applied left-to-right over the target's scratch state.

use crate::domain::{self, EntityState, Event};
use resonantdust_codec::event_word::{
    pack_word, word_op_code, word_payload, word_server_reference, ACTION_AWAIT, ACTION_DAMAGE,
    ACTION_FAIL, ACTION_MOVE, ACTION_SKIP, ACTION_SPAWN, OP_ACTION, OP_ALIAS, OP_LITERAL, OP_OBJECT,
};
use resonantdust_codec::refs::{entity_ref_object_reference, entity_ref_server_reference};

/// How the interpreter reads an `OBJECT` actor operand — mapped by the caller (the worker) from
/// the 48-bit `(server_reference, object_reference)` = `(mint_server, entity_id)` identity to
/// the actor's hp *at the read tic* (`≤ T−1`). Returns `0` for an absent / unsettled / dead
/// actor (all read as "no live actor"), which is what the read-rule + the `DAMAGE` void want.
pub trait Reads {
    fn actor_hp(&self, server_reference: u16, object_reference: u32) -> u64;
}

/// The no-op reader for programs that read no operand (the pure hot path, tests).
pub struct NoReads;
impl Reads for NoReads {
    fn actor_hp(&self, _s: u16, _o: u32) -> u64 {
        0
    }
}

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
/// `LITERAL`s push data; `OBJECT` pushes an actor read; an `ACTION` word pops its operands and
/// applies its verb. Control flow is **forward-only** (D1): `SKIP` pops a `LITERAL` word-count and
/// a boolean and jumps forward when the boolean is false (the `? then : else` primitive); `FAIL`
/// halts the program (the row aborts, producing no further effect). No backward jumps ⇒ bounded.
/// `ALIAS` (await) is resolved at the worker level — a no-op in a pure run.
pub fn run(actions: &[u64], base: EntityState, reads: &dyn Reads) -> EntityState {
    let mut stack: Vec<u64> = Vec::new();
    let mut state = base;
    let mut i = 0;
    while i < actions.len() {
        let w = actions[i];
        i += 1;
        match word_op_code(w) {
            OP_LITERAL => stack.push(word_payload(w) as u64),
            // An OBJECT actor operand resolves to the actor's hp (via the reader) — 0 if
            // absent/unsettled/dead. Actor-reading verbs (DAMAGE) consume it.
            OP_OBJECT => stack.push(reads.actor_hp(word_server_reference(w), word_payload(w))),
            OP_ACTION => match word_payload(w) {
                // Forward-only branch: pop count + condition; skip `count` words when the
                // condition is false (`skip-if-false`). An unconditional skip pushes `0` first.
                ACTION_SKIP => {
                    let count = stack.pop().unwrap_or(0) as usize;
                    let cond = stack.pop().unwrap_or(0);
                    if cond == 0 {
                        i = (i + count).min(actions.len());
                    }
                }
                // Abort the program: no further effects (the row's `FAIL` branch).
                ACTION_FAIL => break,
                // [OBJECT(actor→hp), LITERAL(amount), DAMAGE]: a LIVE actor's blow lands.
                ACTION_DAMAGE => {
                    let amount = stack.pop().unwrap_or(0);
                    let actor_hp = stack.pop().unwrap_or(0);
                    if actor_hp > 0 {
                        state.data[0] = state.data[0].saturating_sub(amount);
                    }
                }
                // Data-carrying verbs (MOVE/SPAWN): reconstruct the Event and apply.
                action => {
                    let d1 = pop_u64(&mut stack);
                    let d0 = pop_u64(&mut stack);
                    let ev = Event { action: domain_action(action), actor_key: 0, data: [d0, d1] };
                    state = domain::apply_event(state, &ev, None);
                }
            },
            OP_ALIAS => { /* await — resolved at the worker level; no-op in a pure run */ }
            _ => {}
        }
    }
    state
}

/// Encode `cond ? then : else` with the forward-only `SKIP` (D1). `cond` is a word stream that
/// leaves a boolean on the stack (nonzero = true — e.g. an `OBJECT` actor read, or `lit(1)`);
/// when it's false the guard `SKIP` jumps over the `then` block **and** the trailing unconditional
/// skip, landing in `else`. When true, `then` runs and the unconditional skip jumps over `else`.
pub fn encode_if(cond: Vec<u64>, then_block: Vec<u64>, else_block: Vec<u64>) -> Vec<u64> {
    let mut v = cond;
    // guard: cond false ⇒ skip the then-block + the 3-word unconditional skip that follows it.
    v.push(lit((then_block.len() + 3) as u32));
    v.push(pack_word(OP_ACTION, 0, ACTION_SKIP));
    v.extend(then_block);
    // after `then`, unconditionally skip the else-block (push `0` ⇒ skip-if-false always skips).
    v.push(lit(0));
    v.push(lit(else_block.len() as u32));
    v.push(pack_word(OP_ACTION, 0, ACTION_SKIP));
    v.extend(else_block);
    v
}

// ── await gate (S5 — control flow) ──────────────────────────────────────────────
//
// A row may open with an await gate: `[LITERAL(timeout_tics), ALIAS(event_reference),
// ACTION(AWAIT), <body…>]`. The *worker* evaluates the gate (checks the aliased row's
// completion, defers or fails), then runs the body via [`run`] — keeping `run` pure.

/// If `actions` opens with an await gate, return `(timeout_tics, await_event_reference, body)`;
/// else `None` (an unconditional program — run all of it). The `event_reference` is a `u32`,
/// carried in the `ALIAS` word's 32-bit payload (the reference model's `event_reference : u32`).
pub fn await_gate(actions: &[u64]) -> Option<(u32, u32, &[u64])> {
    if actions.len() >= 3
        && word_op_code(actions[0]) == OP_LITERAL
        && word_op_code(actions[1]) == OP_ALIAS
        && word_op_code(actions[2]) == OP_ACTION
        && word_payload(actions[2]) == ACTION_AWAIT
    {
        Some((word_payload(actions[0]), word_payload(actions[1]), &actions[3..]))
    } else {
        None
    }
}

/// Build an await-gated program: block on `await_ref` for `timeout` tics, then run `body`.
pub fn encode_await(await_ref: u32, timeout: u32, body: Vec<u64>) -> Vec<u64> {
    let mut v = vec![
        pack_word(OP_LITERAL, 0, timeout),
        pack_word(OP_ALIAS, 0, await_ref),
        pack_word(OP_ACTION, 0, ACTION_AWAIT),
    ];
    v.extend(body);
    v
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

/// A `DAMAGE` program: `[OBJECT(actor), LITERAL(amount), ACTION(DAMAGE)]`. The `actor`
/// entity_reference is carried as `(server_reference, object_reference)` in the OBJECT word (the
/// worker maps it back). Targets the victim (in the row's `targets`).
pub fn encode_damage(actor: u64, amount: u32) -> Vec<u64> {
    vec![
        pack_word(OP_OBJECT, entity_ref_server_reference(actor), entity_ref_object_reference(actor)),
        lit(amount),
        pack_word(OP_ACTION, 0, ACTION_DAMAGE),
    ]
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
        let got = run(&encode_move(zone, loc, rot, off), base, &NoReads);
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
        let got = run(&encode_spawn(42, 3, 0x22, 1, 0), base, &NoReads);
        let want = domain::apply_event(
            base,
            &Event { action: domain::ACTION_SPAWN, actor_key: 0, data: domain::pack_spawn(42, 3, 0x22, 1, 0) },
            None,
        );
        assert_eq!(got, want);
        assert_eq!(got.kind, 42, "spawn sets kind (non-tombstone)");
        assert_eq!(got.location, 0x22);
    }

    /// A stub reader returning a fixed hp for one actor id, 0 otherwise.
    struct StubReads {
        actor_id: u32,
        hp: u64,
    }
    impl Reads for StubReads {
        fn actor_hp(&self, _s: u16, o: u32) -> u64 {
            if o == self.actor_id {
                self.hp
            } else {
                0
            }
        }
    }

    #[test]
    fn damage_voids_when_actor_dead_lands_when_alive() {
        use resonantdust_codec::refs::pack_hot_entity;
        let actor = pack_hot_entity(1, 42);
        let victim = EntityState { data: [100, 0], ..EntityState::default() }; // hp 100
        // live actor (hp 30) → 100 − 25 = 75
        let live = StubReads { actor_id: 42, hp: 30 };
        assert_eq!(run(&encode_damage(actor, 25), victim, &live).data[0], 75);
        // dead actor (hp 0) → blow voided, hp unchanged
        let dead = StubReads { actor_id: 42, hp: 0 };
        assert_eq!(run(&encode_damage(actor, 25), victim, &dead).data[0], 100);
        // damage saturates at 0, never underflows
        assert_eq!(run(&encode_damage(actor, 999), victim, &live).data[0], 0);
    }

    #[test]
    fn skip_branch_picks_then_or_else() {
        // `cond ? move→0x22 : move→0x44` — the two branches land the pawn on different tiles.
        let then_b = encode_move(0, 0x22, 0, 0);
        let else_b = encode_move(0, 0x44, 0, 0);
        let base = EntityState { kind: 9, ..EntityState::default() };
        // cond TRUE (lit 1) → then-block runs, else skipped.
        let prog_t = encode_if(vec![lit(1)], then_b.clone(), else_b.clone());
        assert_eq!(run(&prog_t, base, &NoReads).location, 0x22, "true → then");
        // cond FALSE (lit 0) → then skipped, else-block runs.
        let prog_f = encode_if(vec![lit(0)], then_b.clone(), else_b.clone());
        assert_eq!(run(&prog_f, base, &NoReads).location, 0x44, "false → else");
    }

    #[test]
    fn skip_branch_condition_from_actor_read() {
        use resonantdust_codec::refs::pack_hot_entity;
        // `actor_alive ? move→0x22 : move→0x44` — the OBJECT read is the condition.
        let actor = pack_hot_entity(1, 42);
        let cond = vec![pack_word(OP_OBJECT, entity_ref_server_reference(actor), entity_ref_object_reference(actor))];
        let prog = encode_if(cond, encode_move(0, 0x22, 0, 0), encode_move(0, 0x44, 0, 0));
        let base = EntityState { kind: 9, ..EntityState::default() };
        // live actor (hp 30 ≠ 0) → then
        assert_eq!(run(&prog, base, &StubReads { actor_id: 42, hp: 30 }).location, 0x22);
        // dead actor (hp 0) → else
        assert_eq!(run(&prog, base, &StubReads { actor_id: 42, hp: 0 }).location, 0x44);
    }

    #[test]
    fn fail_halts_program() {
        // `move→0x22 ; FAIL ; move→0x44` — FAIL aborts before the second move.
        let mut prog = encode_move(0, 0x22, 0, 0);
        prog.push(pack_word(OP_ACTION, 0, ACTION_FAIL));
        prog.extend(encode_move(0, 0x44, 0, 0));
        let got = run(&prog, EntityState { kind: 9, ..EntityState::default() }, &NoReads);
        assert_eq!(got.location, 0x22, "FAIL halts — the post-FAIL move never applies");
    }

    #[test]
    fn multi_action_vector_applies_in_order() {
        // one program, two verbs: SPAWN (kind 42, loc 0x22) then MOVE (loc 0x33) — both land.
        let mut prog = encode_spawn(42, 0, 0x22, 0, 0);
        prog.extend(encode_move(0, 0x33, 0, 0));
        let got = run(&prog, EntityState::default(), &NoReads);
        assert_eq!(got.kind, 42, "spawn set the kind");
        assert_eq!(got.location, 0x33, "the later move won");
    }

    #[test]
    fn await_gate_wraps_and_unwraps() {
        let body = encode_move(0, 0x22, 0, 0);
        // encode_await(await_ref = 5, timeout = 7, body)
        let gated = encode_await(5, 7, body.clone());
        // an unconditional program has no gate…
        assert!(await_gate(&body).is_none());
        // …a gated one exposes (timeout, await_ref, body) and the body runs the same.
        let (timeout, await_ref, got_body) = await_gate(&gated).expect("gate");
        assert_eq!((timeout, await_ref), (7, 5));
        assert_eq!(got_body, &body[..]);
        let base = EntityState { kind: 9, ..EntityState::default() };
        assert_eq!(run(got_body, base, &NoReads), run(&body, base, &NoReads));
    }

    #[test]
    fn u64_data_survives_the_lo_hi_split() {
        // a large zone_id exercises the >32-bit data word.
        let z = 0x00AB_CDEFu32;
        let got = run(&encode_move(z, 0x0F, 0, 0), EntityState { zone_id: z, ..EntityState::default() }, &NoReads);
        assert_eq!(got.zone_id, z);
        assert_eq!(got.location, 0x0F);
    }
}
