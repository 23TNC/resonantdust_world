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
/// Latch: project this target's slot to `state` once settled. Operand: `target` (write).
pub const PROMOTE_STATE: u32 = 1;
/// Latch: project this event to `event` on settle. No operand.
pub const PROMOTE_EVENT: u32 = 2;
/// Mint a new entity at a position. Operands: `def` (imm), `position` (imm). Written target is the
/// minted id, not an operand.
pub const CREATE: u32 = 3;
/// Set an object's position absolutely. Operands: `obj` (write), `position` (imm).
pub const PLACE: u32 = 4;
/// Step an object one tile toward a destination, then queue the next hop. Operands: `obj`
/// (read+write), `dest` (imm).
pub const MOVE_TO: u32 = 5;

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
        PROMOTE_STATE => &[Write],   // the promoted target needs a slot to carry the flag
        PROMOTE_EVENT => &[],
        CREATE => &[Imm, Imm],       // def, position — the write is the minted id, not an operand
        PLACE => &[Write, Imm],      // obj, position
        MOVE_TO => &[ReadWrite, Imm], // obj (reads its own position, writes the next), dest
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
        let ar = match arity(action) {
            Some(a) => a,
            None => {
                self.done = true;
                return Some(Err(ProgramError::UnknownAction(action)));
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

#[cfg(test)]
mod tests {
    use super::*;

    // A representative program: place obj at pos, then move it to dest, promoting both.
    // PLACE obj pos  MOVE_TO obj dest  PROMOTE_STATE obj  PROMOTE_EVENT
    fn sample(obj: u32, pos: u32, dest: u32) -> Vec<u32> {
        vec![PLACE, obj, pos, MOVE_TO, obj, dest, PROMOTE_STATE, obj, PROMOTE_EVENT]
    }

    #[test]
    fn arity_is_the_signature_length() {
        assert_eq!(arity(PLACE), Some(2));
        assert_eq!(arity(PROMOTE_EVENT), Some(0));
        assert_eq!(arity(MOVE_TO), Some(2));
        assert_eq!(arity(0xDEAD), None);
    }

    #[test]
    fn program_frames_each_instruction_by_arity() {
        let p = sample(0x11, 0x22, 0x33);
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 4);
        assert_eq!(insts[0], Instruction { action: PLACE, operands: &[0x11, 0x22] });
        assert_eq!(insts[1], Instruction { action: MOVE_TO, operands: &[0x11, 0x33] });
        assert_eq!(insts[2], Instruction { action: PROMOTE_STATE, operands: &[0x11] });
        assert_eq!(insts[3], Instruction { action: PROMOTE_EVENT, operands: &[] });
    }

    #[test]
    fn write_and_read_sets_fall_out_of_the_scan() {
        let p = sample(0x11, 0x22, 0x33);
        // writes: PLACE.obj, MOVE_TO.obj, PROMOTE_STATE.obj — all 0x11
        assert_eq!(write_targets(&p).unwrap(), vec![0x11, 0x11, 0x11]);
        // reads: MOVE_TO.obj (ReadWrite)
        assert_eq!(read_targets(&p).unwrap(), vec![0x11]);
        assert!(asks_promote_event(&p));
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
