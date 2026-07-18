//! The event program — `event_log.actions : Vec<u32>`. Authoritative: `docs/ACTIONS.md`.
//!
//! A flat stream: `(action operand{arity(action)})*`. The action leads and its **arity** says how
//! many u32s follow; its **signature** says what each operand is. So an operand carries no tag — the
//! action already said. Every operand is a **literal** (no stack): a written target is always spelled
//! out, so the write set is known at grouping.
//!
//! This module is the one place that knows a verb's arity and per-operand kind. The event shard
//! scans it for the **write set** (grouping); the worker scans it for the **read set** (blocking) and
//! to run the program. Neither interprets a verb — the signature is a table lookup.

// ── palette ─────────────────────────────────────────────────────────────────────
// action_reference : u32. Append-only once a program is stored (nothing is yet).

/// Reserved null.
pub const ACTION_NONE: u32 = 0;
/// **Prefix modifier** (arity 0): set the promote bit for the **next** action, so its write targets
/// project to their client-visible table (`entity_state`/`overlay`) once settled. Executed
/// left-to-right — the next action writes the bit as part of its own result, so there's no post-pass
/// (see `docs/ACTIONS.md`). Replaces the old `PROMOTE_STATE` (which named a target explicitly).
pub const PROMOTE: u32 = 1;
/// Latch: project this event to `event` on settle. No operand. **Tabled** — revisited with movement.
pub const PROMOTE_EVENT: u32 = 2;
/// Mint a new entity at a position. Operands: `def` (imm), `position` (imm). Written target is the
/// minted id, not an operand.
pub const CREATE: u32 = 3;
/// Set an object's position absolutely. Operands: `obj` (write), `position` (imm).
pub const PLACE: u32 = 4;
/// Step an object one tile toward a destination, then queue the next hop. Operands: `obj`
/// (read+write), `dest` (imm).
pub const MOVE_TO: u32 = 5;
/// **Generate a zone's whole baseline row** through the pipeline (the event-driven `seed`, F12).
/// **Variable arity** (the only such verb): `INIT_ZONE cold_row type_id count item×count` — `cold_row`
/// (write — the target), `type_id` (imm — the cold shard), `count` (imm — how many `item` words
/// follow), then `count` `item`s (imm — the row's payload: tile = dense `kind_reference` by index,
/// thing = `kind_pos_reference` per occupied cell). The worker builds the shard's `Vec<DenseItem>` and
/// writes it to `entity_state_log` (`ColdBaseline` tier) + promotes. `PROMOTE INIT_ZONE …` makes it
/// client-visible.
pub const INIT_ZONE: u32 = 7;
/// **Override one cold cell** through the `overlay` tier. Operands: `cold_row` (write — the target,
/// `cold_row_reference` = `macro:16 | subtype:12 | layer:4`, spelled so grouping unions by it),
/// `type_id` (imm — which cold shard, `TYPE_BIOME_TILE`/`TYPE_BIOME_THING`; a cold_row has no type
/// nibble, so routing reads this — see [`target_routes`]), `tile_reference` (imm — the cell),
/// `kind_reference` (imm — the new kind, `0` = clear), `data` (imm — thing rotation/count; tiles
/// ignore). Concurrent `SET`s to one row **group** (shared `cold_row` target) so one worker composes
/// the whole overlay row — they can't race. Replaces the old per-cell `cold_entity_reference` SET.
pub const SET: u32 = 6;

/// What an operand is, for deriving the write/read sets. Only `entity_reference` operands matter to
/// the sets; `Imm` operands (numbers, positions, definitions) are neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    /// An immediate literal — a number, `position_reference`, or `definition_reference`. Not a target.
    Imm,
    /// An `entity_reference` the action reads (→ the worker blocks on it settling).
    Read,
    /// An `entity_reference` the action writes (→ its slot is grouped/claimed).
    Write,
    /// An `entity_reference` the action both reads and writes (in both sets).
    ReadWrite,
}

impl OperandKind {
    pub fn is_write(self) -> bool {
        matches!(self, OperandKind::Write | OperandKind::ReadWrite)
    }
    pub fn is_read(self) -> bool {
        matches!(self, OperandKind::Read | OperandKind::ReadWrite)
    }
}

/// The operand signature of an action, or `None` if the action id is unknown. Its length **is** the
/// arity — arity is not a separate table, it is `signature(a).len()`. An unknown action can't be
/// framed (its arity is unknown), which is why the stream reader errors on one.
pub fn signature(action: u32) -> Option<&'static [OperandKind]> {
    use OperandKind::*;
    Some(match action {
        ACTION_NONE => &[],
        PROMOTE => &[],              // prefix — no operand; it flags the NEXT action's write targets
        PROMOTE_EVENT => &[],
        CREATE => &[Imm, Imm],       // def, position — the write is the minted id, not an operand
        PLACE => &[Write, Imm],      // obj, position
        MOVE_TO => &[ReadWrite, Imm], // obj (reads its own position, writes the next), dest
        SET => &[Write, Imm, Imm, Imm, Imm], // cold_row, type_id, tile_reference, kind_reference, data
        _ => return None,
    })
}

/// The arity of an action (operand count), or `None` if unknown.
pub fn arity(action: u32) -> Option<usize> {
    signature(action).map(<[_]>::len)
}

// ── stream reader ─────────────────────────────────────────────────────────────

/// One decoded instruction: the action and a borrowed slice of its operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction<'a> {
    pub action: u32,
    pub operands: &'a [u32],
}

/// An error decoding a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramError {
    /// An action id with no signature — arity unknown, so the rest of the stream can't be framed.
    UnknownAction(u32),
    /// The stream ended mid-instruction — fewer operands than the action's arity.
    Truncated { action: u32, want: usize, got: usize },
}

/// Iterate a program's instructions. A wrong arity mis-frames everything after it and there is no
/// re-sync point, so a malformed stream yields an `Err` and the iterator stops — validate at the
/// door (the edge) and reject, rather than hand a worker a stream it can't frame.
pub struct Program<'a> {
    words: &'a [u32],
    pos: usize,
    done: bool,
}

/// Start decoding `words` as a program.
pub fn program(words: &[u32]) -> Program<'_> {
    Program { words, pos: 0, done: false }
}

impl<'a> Iterator for Program<'a> {
    type Item = Result<Instruction<'a>, ProgramError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.pos >= self.words.len() {
            return None;
        }
        let action = self.words[self.pos];
        // `INIT_ZONE` is the one **variable-arity** verb (F12): `cold_row type_id count item×count`, so
        // its arity is `3 + count` (the `count` word sits at `pos + 3`). Every other verb is fixed.
        let ar = if action == INIT_ZONE {
            match self.words.get(self.pos + 3) {
                Some(&count) => 3 + count as usize,
                None => {
                    self.done = true;
                    return Some(Err(ProgramError::Truncated {
                        action,
                        want: 3,
                        got: self.words.len() - self.pos - 1,
                    }));
                }
            }
        } else {
            match arity(action) {
                Some(a) => a,
                None => {
                    self.done = true;
                    return Some(Err(ProgramError::UnknownAction(action)));
                }
            }
        };
        let first = self.pos + 1;
        let end = first + ar;
        if end > self.words.len() {
            self.done = true;
            return Some(Err(ProgramError::Truncated {
                action,
                want: ar,
                got: self.words.len() - first,
            }));
        }
        self.pos = end;
        Some(Ok(Instruction { action, operands: &self.words[first..end] }))
    }
}

/// Every operand of matching `kind`, across a whole program. `Err` if the program doesn't parse.
fn collect_operands(
    words: &[u32],
    keep: impl Fn(OperandKind) -> bool,
) -> Result<Vec<u32>, ProgramError> {
    let mut out = Vec::new();
    for inst in program(words) {
        let inst = inst?;
        // INIT_ZONE is variable-arity (no fixed signature): `cold_row` (write) + `type_id`/`count`/
        // `item×count` (all imm). Only the `cold_row` target matters to the sets.
        if inst.action == INIT_ZONE {
            if keep(OperandKind::Write) {
                if let Some(cold_row) = inst.operands.first() {
                    out.push(*cold_row);
                }
            }
            continue;
        }
        let sig = signature(inst.action).expect("parsed, so known");
        for (op, &k) in inst.operands.iter().zip(sig) {
            if keep(k) {
                out.push(*op);
            }
        }
    }
    Ok(out)
}

/// The **write set** — every `entity_reference` the program writes, for grouping. Note `CREATE`'s
/// minted target is *not* here: a fresh entity is its own singleton component, handled separately.
pub fn write_targets(words: &[u32]) -> Result<Vec<u32>, ProgramError> {
    collect_operands(words, OperandKind::is_write)
}

/// The **read set** — every `entity_reference` the program reads, for blocking.
pub fn read_targets(words: &[u32]) -> Result<Vec<u32>, ProgramError> {
    collect_operands(words, OperandKind::is_read)
}

/// True if the program carries a `PROMOTE_EVENT` — latched at `queue`.
pub fn asks_promote_event(words: &[u32]) -> bool {
    program(words).any(|i| matches!(i, Ok(Instruction { action: PROMOTE_EVENT, .. })))
}

/// Where a write target routes to compose — the shard **and tier**. Hot targets (pawns, …) route by
/// the target's own `server_reference` nibble (the caller's `shard_of`); a **cold** target is a
/// `cold_row_reference`, which carries no type nibble, so its action names the shard's `type_id` and
/// whether it hits the baseline (`entity_state`, via `claim`/`write`) or the `overlay` tier (via
/// `claim_overlay`/`write_overlay`). See [`crate::action`] head + `docs/work/shard-tables/` (F11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Hot `data_shard` (or any entity-addressed shard) — route by the target's own nibble.
    Hot,
    /// Cold baseline (`entity_state`) on the shard with this `type_id` — `init_zone`/`PACK`.
    ColdBaseline { type_id: u8 },
    /// Cold override (`overlay`) on the shard with this `type_id` — `SET`.
    ColdOverlay { type_id: u8 },
}

/// Each write target paired with its [`Route`], in program order — the routing companion to
/// [`write_targets`] (same targets, same order). The grouper unions by the bare targets
/// (`write_targets`); the claimer + worker use this to pick the shard **and** the `claim`/`write` vs
/// `claim_overlay`/`write_overlay` reducer per target. `Err` if the program doesn't parse.
pub fn target_routes(words: &[u32]) -> Result<Vec<(u32, Route)>, ProgramError> {
    let mut out = Vec::new();
    for inst in program(words) {
        let inst = inst?;
        match inst.action {
            // SET cold_row type_id tile kind data — the cold-cell overlay verb (cold_row is Write).
            SET => {
                if let [cold_row, type_id, ..] = inst.operands {
                    out.push((*cold_row, Route::ColdOverlay { type_id: *type_id as u8 }));
                }
            }
            // INIT_ZONE cold_row type_id count item×count — the cold baseline verb (cold_row is Write).
            INIT_ZONE => {
                if let [cold_row, type_id, ..] = inst.operands {
                    out.push((*cold_row, Route::ColdBaseline { type_id: *type_id as u8 }));
                }
            }
            // Hot verbs: every Write/ReadWrite operand routes by its own nibble.
            _ => {
                let sig = signature(inst.action).expect("parsed, so known");
                for (op, &k) in inst.operands.iter().zip(sig) {
                    if k.is_write() {
                        out.push((*op, Route::Hot));
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A representative program: promote-then-place obj at pos, then move it to dest, promote the event.
    // PROMOTE  PLACE obj pos  MOVE_TO obj dest  PROMOTE_EVENT
    fn sample(obj: u32, pos: u32, dest: u32) -> Vec<u32> {
        vec![PROMOTE, PLACE, obj, pos, MOVE_TO, obj, dest, PROMOTE_EVENT]
    }

    #[test]
    fn arity_is_the_signature_length() {
        assert_eq!(arity(PLACE), Some(2));
        assert_eq!(arity(PROMOTE), Some(0));
        assert_eq!(arity(PROMOTE_EVENT), Some(0));
        assert_eq!(arity(MOVE_TO), Some(2));
        assert_eq!(arity(0xDEAD), None);
    }

    #[test]
    fn program_frames_each_instruction_by_arity() {
        let p = sample(0x11, 0x22, 0x33);
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 4);
        assert_eq!(insts[0], Instruction { action: PROMOTE, operands: &[] });
        assert_eq!(insts[1], Instruction { action: PLACE, operands: &[0x11, 0x22] });
        assert_eq!(insts[2], Instruction { action: MOVE_TO, operands: &[0x11, 0x33] });
        assert_eq!(insts[3], Instruction { action: PROMOTE_EVENT, operands: &[] });
    }

    #[test]
    fn write_and_read_sets_fall_out_of_the_scan() {
        let p = sample(0x11, 0x22, 0x33);
        // writes: PLACE.obj, MOVE_TO.obj — all 0x11 (PROMOTE is arity 0, no target)
        assert_eq!(write_targets(&p).unwrap(), vec![0x11, 0x11]);
        // reads: MOVE_TO.obj (ReadWrite)
        assert_eq!(read_targets(&p).unwrap(), vec![0x11]);
        assert!(asks_promote_event(&p));
    }

    #[test]
    fn set_frames_a_cold_row_target_routed_to_the_overlay() {
        use crate::object::TYPE_BIOME_TILE;
        // SET cold_row type_id tile kind data — arity 5; cold_row is the (only) write target.
        let cold_row = 0x0102_0030_u32; // macro 0x0102, subtype/layer in the low half
        let p = vec![SET, cold_row, TYPE_BIOME_TILE as u32, 0x40, 0x0088, 0x00];
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 1);
        assert_eq!(arity(SET), Some(5));
        // grouping sees the spelled cold_row; the type_id/tile/kind/data are Imm.
        assert_eq!(write_targets(&p).unwrap(), vec![cold_row]);
        assert_eq!(read_targets(&p).unwrap(), Vec::<u32>::new());
        // routing reads the action → the overlay tier on the tile shard.
        assert_eq!(
            target_routes(&p).unwrap(),
            vec![(cold_row, Route::ColdOverlay { type_id: TYPE_BIOME_TILE })]
        );
    }

    #[test]
    fn init_zone_frames_variable_arity_and_routes_to_the_baseline() {
        use crate::object::TYPE_BIOME_TILE;
        let cold_row = 0x0102_0030_u32;
        // PROMOTE  INIT_ZONE cold_row type_id count=3 [a,b,c]  — then a trailing PROMOTE_EVENT to prove
        // the variable arity frames the *next* instruction correctly.
        let p = vec![PROMOTE, INIT_ZONE, cold_row, TYPE_BIOME_TILE as u32, 3, 0xAA, 0xBB, 0xCC, PROMOTE_EVENT];
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 3);
        assert_eq!(insts[0].action, PROMOTE);
        assert_eq!(insts[1], Instruction { action: INIT_ZONE, operands: &[cold_row, TYPE_BIOME_TILE as u32, 3, 0xAA, 0xBB, 0xCC] });
        assert_eq!(insts[2].action, PROMOTE_EVENT); // framed correctly after the variable payload
        assert_eq!(write_targets(&p).unwrap(), vec![cold_row]); // only cold_row, not the items
        assert_eq!(
            target_routes(&p).unwrap(),
            vec![(cold_row, Route::ColdBaseline { type_id: TYPE_BIOME_TILE })]
        );
    }

    #[test]
    fn init_zone_truncated_before_its_count_errors() {
        // INIT_ZONE needs at least cold_row, type_id, count — this stops before count.
        let p = vec![INIT_ZONE, 0x11, 0x01];
        assert!(write_targets(&p).is_err());
    }

    #[test]
    fn target_routes_marks_hot_verbs_hot() {
        // PROMOTE PLACE obj pos  MOVE_TO obj dest  PROMOTE_EVENT — both writes are hot (route by nibble).
        let p = sample(0x11, 0x22, 0x33);
        assert_eq!(target_routes(&p).unwrap(), vec![(0x11, Route::Hot), (0x11, Route::Hot)]);
    }

    #[test]
    fn create_target_is_not_in_the_write_set() {
        // CREATE def pos — both operands Imm; the minted id is not an operand.
        let p = vec![CREATE, 0xAAAA, 0xBBBB];
        assert_eq!(write_targets(&p).unwrap(), Vec::<u32>::new());
        assert_eq!(read_targets(&p).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn a_wrong_arity_mis_frames_and_the_reader_errors() {
        // MOVE_TO claims 2 operands but only 1 follows.
        let p = vec![MOVE_TO, 0x11];
        let last = program(&p).last().unwrap();
        assert_eq!(last, Err(ProgramError::Truncated { action: MOVE_TO, want: 2, got: 1 }));
        assert!(write_targets(&p).is_err());
    }

    #[test]
    fn an_unknown_action_stops_the_stream() {
        // A bogus action id has no arity, so nothing after it can be framed.
        let p = vec![0xDEAD, 1, 2, 3];
        let mut it = program(&p);
        assert_eq!(it.next(), Some(Err(ProgramError::UnknownAction(0xDEAD))));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn empty_program_is_valid_and_empty() {
        assert_eq!(program(&[]).count(), 0);
        assert_eq!(write_targets(&[]).unwrap(), Vec::<u32>::new());
    }
}
