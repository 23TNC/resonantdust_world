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
/// Latch: project this event to `event` on settle. No operand. The movement INTENT channel —
/// its presence also marks a program as the intent event (the `MOVE_TO` seed that stamps the
/// trip-serial and steps nothing; `ACTIONS.md` §Movement).
pub const PROMOTE_EVENT: u32 = 2;
/// Mint a new entity at a position — THE creation verb for any object (human-pawns F2: no
/// separate spawn action; the packed def's `type_id` routes the mint to its type's shard arm —
/// `TYPE_PAWN` today, others rejected by name). **Variable arity** (like `INIT_ZONE`):
/// `CREATE def position count payload×count` — `def` (imm, packed `definition_reference`),
/// `position` (imm), `count` (imm — how many payload words follow), then the `count`-word
/// payload opcode stream written to the minted entity's sidecar (`TABLES.md § payload`).
/// Written target is the minted id, not an operand.
pub const CREATE: u32 = 3;
/// Set an object's position absolutely. Operands: `obj` (write), `position` (imm).
pub const PLACE: u32 = 4;
/// Step an object one tile toward a destination, then queue the next hop. Operands: `obj`
/// (read+write), `dest` (imm).
pub const MOVE_TO: u32 = 5;
/// **Generate a zone's whole baseline row** through the pipeline (the event-driven `seed`, F12).
/// **Variable arity** (with [`CREATE`]): `INIT_ZONE cold_row type_id count item×count` — `cold_row`
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
/// One movement-chain hop, **WORKER-ONLY** (the edge's client-verb allowlist rejects it —
/// movement-hardening F2). Operands: `obj` (read+write), `dest` (imm), `serial` (imm — the
/// trip-serial the chain's seed stamped into `obj`'s `data` low bits). Steps one tile and
/// re-queues ONLY while the serial still matches; a mismatch means a newer intent superseded
/// this chain, and the hop dies silently (`ACTIONS.md` §Movement chain identity).
pub const MOVE_STEP: u32 = 8;
/// The CLIENT-issued build order (build-walls D5). Operands: `start` (imm,
/// `position_reference`), `end` (imm, `position_reference`), `object` (imm — the kind's
/// definition reference, a full u32 slot). Writes NOTHING itself: the worker expands the
/// rect's PERIMETER and queues a `PROMOTE SET` per tile (the verb-that-queues-events
/// pattern), so each cell routes to ITS zone — a multi-zone rect is safe by construction.
/// Immediate building is the worker's CURRENT policy, not this verb's contract: the
/// documented future swaps the queued events for blueprint-entity creates (`ACTIONS.md`).
pub const BUILD_WALL: u32 = 9;
/// Set one NEED's value on a pawn (stat-model F1/F4). Operands: `obj` (write — the pawn;
/// grouping serialises this with its movement writes), `row` (imm, the packed gameplay row
/// `value:16 | kind:12 | variant:4` — value is u16 FIXED-POINT on the need's authored
/// domain, quantized ONCE by the composer via [`crate::value::quantize`]). The pawn module's
/// `set_need` reducer upserts the `needs` sub-table row (`set_tic` = the composing tic) —
/// the worker relays, so the verb adds nothing to the worker's read set. Arity 3→2 with the
/// stat-model reshape (the def-ref + f32-bits form is gone).
pub const SET_NEED: u32 = 10;
/// Grant one STORED (timed) condition to a pawn (stat-model F1/F3). Operands: `obj`
/// (write), `row` (imm, the packed gameplay row `remaining_at_write:16 | kind:12 |
/// variant:4` — the composer passes the condition's authored `duration` as
/// `remaining_at_write`). `written_tic` = the composing tic; remaining-now and expiry are
/// DERIVED at read, never stored. A re-grant refreshes the row; the reducer also RE-STAMPS
/// the condition's affected need rows in the same transaction (stat-model F7). DERIVED
/// (band) conditions have no verb — they are computed, not granted.
pub const GRANT_CONDITION: u32 = 11;
/// Execute a corpus-defined interaction (interactions F4 — the user's layout verbatim).
/// Operands: `interaction` (imm, gameplay `definition_reference`) · `version` (imm) ·
/// `count` (imm) · `inputs×count` (imm — untyped words; value inputs are f32 BIT PATTERNS,
/// meaning comes from the interaction's corpus input SIGNATURE). The third variable-arity
/// verb (`INIT_ZONE`, `CREATE`); `count` sits third in all three. **Writes NOTHING itself**
/// (the `BUILD_WALL` pattern): the worker resolves + validates and queues
/// `PROMOTE SET_NEED`/`PROMOTE GRANT_CONDITION`, whose typed operands carry the real write
/// set — so the untyped input words never need top-byte routing.
pub const EXECUTE_INTERACTION: u32 = 12;

/// The pawn's INTENT-QUEUE snapshot, **WORKER-ONLY** and always `PROMOTE_EVENT`-prefixed
/// (intent-queue-ui F1 — a pure DISPLAY fan; authority stays the worker's ephemeral
/// map). Operands: `pawn` (imm) · `_reserved` (imm, 0) · `count` (imm, total payload
/// words) · `entry×4 per intent` — `[order_event_reference, interaction_ref, phase,
/// started:16|fire:16]`, entry 0 = the ACTIVE event, phase 0 pending / 1 walking /
/// 2 executing. The fourth variable-arity verb; `count` sits third like the others.
pub const QUEUE_STATE: u32 = 13;

/// Cancel a queued intent by its ORDER's event_reference (intent-queue-ui F3/F4) —
/// CLIENT-open; resolves against worker MEMORY and writes nothing. Operands: `pawn`
/// (imm) · `order_event_reference` (imm). Unknown reference = a logged no-op.
pub const CANCEL_INTENT: u32 = 14;

/// Add an item to a pawn's inventory at the FIRST FREE slot (inventory F3) —
/// WORKER-only. Operands: `obj` (write — the holding pawn) · `item` (imm, the held
/// thing's `definition_reference`). The composing program MUST carry the free-count
/// `SET_NEED` beside it (one author, one atomic program — rows and count never
/// diverge).
pub const INV_ADD: u32 = 15;

/// Remove one inventory slot's row (inventory F3) — WORKER-only. Operands: `obj`
/// (write) · `slot` (imm, 0-based). Same law as [`INV_ADD`]: the free-count
/// `SET_NEED` rides the same program.
pub const INV_REMOVE: u32 = 16;

/// THE SPAWN AUTHORITY DOOR (spawn-authority F1/F6, the user's layout) — the only
/// CLIENT-OPEN spawn lane; writes nothing itself. Operands (all imm):
/// `xy` = `x:16 | y:16` RAW tile coordinates; `rot_def` = `rotation:4 | type:4 |
/// subtype:12 | kind:12` (the def minus its variant nibble, shifted down four —
/// `def = (rot_def & 0x0FFF_FFFF) << 4 | variant`; rotation = the pawn's initial
/// facing, or the SET data lane for cold types); `variants` = `v0:4 | … | v7:4`,
/// part i's variant at nibble i (the corpus part declarations are the contract —
/// the ≤4-parts law keeps this ONE word so the verb frames corpus-free). The
/// worker validates (registered def, in-world + pathable/empty cell, rotation ≤3,
/// no stray nibbles — REFUSE never nudge) and routes by the TYPE nibble: pawns →
/// the worker-only [`CREATE`] (the spawn ledger stays the one mint gate); things/
/// tiles → the validated [`SET`].
pub const SPAWN_REQUEST: u32 = 17;

/// `RESTAMP_NEED obj need_reference` — the CROSSING RE-STAMP (survival F4/D1,
/// WORKER-ONLY): re-evaluate `obj`'s named need's lazy value AT PROCESSING and write
/// it, feeding the need-write sweep (death at ≤ 0). Queued by the worker's crossing
/// scheduler at the predicted floor-crossing tic; a stale prediction (the pawn ate)
/// re-stamps the honest value and re-schedules — never the predicted one.
pub const RESTAMP_NEED: u32 = 18;

/// `ACTIVATE_TRAIT obj tref` (trait-rows-u32 F3 — CLIENT-OPEN): request activation of the
/// ACTIVE trait `tref` on `obj`. Writes NOTHING itself: the WORKER validates (the carrier
/// BINDS the trait; the category is `*_active`; the def's `blocked_by` conditions are all
/// inactive — sprint's cooldown gate) and queues the def's authored condition GRANTS.
pub const ACTIVATE_TRAIT: u32 = 19;

/// `MOVE_CHORDS pawn entry_id count chord×count` — the STAMPED ROUTE fan (server-chords F1),
/// **WORKER-ONLY** and always `PROMOTE_EVENT`-prefixed so it reaches the zone's subscribers.
///
/// The fifth variable-arity verb; `count` sits third like the other four, because the reader
/// reads it at a fixed offset. Writes NOTHING — every operand is imm and it carries no sets.
///
/// A chord is a straight segment crossing no impathable tile, spelled with BOTH endpoints so it
/// is self-contained: a client interpolates between two stated points and never derives a route,
/// a pace, or a position of its own. That is the whole design — the client's belief cannot drift
/// from the server's because the client holds no belief, only two numbers and a clock.
pub const MOVE_CHORDS: u32 = 20;

/// `CANCEL obj` — **THE FORMAL INTERRUPT** (server-chords F2). Bumps the pawn's trip serial so
/// every queued hop dies at the predicate it already checks, RESOLVES the pawn's position from
/// the current tic, writes it, and drops the intent queue.
///
/// Distinct from [`CANCEL_INTENT`] and it cannot be folded into it: that verb's operands are both
/// `Imm`, so the pawn is no write target — it neither joins the pawn's write group nor serialises
/// against its in-flight hops, and it writes nothing. A verb that PLACES a pawn must be a write,
/// and a signature cannot be conditional on an operand's value.
pub const CANCEL: u32 = 21;

/// Compose a [`SPAWN_REQUEST`] program (spawn-authority F1) — ONE packer shared by every
/// client so the words cannot drift. `def` is a full `definition_reference` whose variant
/// nibble is IGNORED (variants ride the nibble vec); `variants[i]` = part i's u4.
pub fn pack_spawn_request(x: u16, y: u16, rotation: u8, def: u32, variants: &[u8]) -> [u32; 4] {
    let mut v = 0u32;
    for (i, &n) in variants.iter().take(8).enumerate() {
        v |= u32::from(n & 0xF) << (i * 4);
    }
    [
        SPAWN_REQUEST,
        (u32::from(x) << 16) | u32::from(y),
        (u32::from(rotation & 0xF) << 28) | ((def >> 4) & 0x0FFF_FFFF),
        v,
    ]
}

/// Decode a [`SPAWN_REQUEST`]'s three operand words: `(x, y, rotation, def_with_variant0,
/// nibbles)` — the def reconstructs with variant 0 (the caller substitutes per part).
pub fn unpack_spawn_request(xy: u32, rot_def: u32, variants: u32) -> (u16, u16, u8, u32, [u8; 8]) {
    let mut n = [0u8; 8];
    for (i, slot) in n.iter_mut().enumerate() {
        *slot = ((variants >> (i * 4)) & 0xF) as u8;
    }
    (
        (xy >> 16) as u16,
        (xy & 0xFFFF) as u16,
        (rot_def >> 28) as u8,
        (rot_def & 0x0FFF_FFFF) << 4,
        n,
    )
}

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
        // CREATE is variable-arity (def, position, count, payload×count — all Imm; the write is
        // the minted id, not an operand) — framed in the reader like INIT_ZONE, no fixed signature.
        PLACE => &[Write, Imm],      // obj, position
        MOVE_TO => &[ReadWrite, Imm], // obj (reads its own position, writes the next), dest
        MOVE_STEP => &[ReadWrite, Imm, Imm], // obj, dest, trip-serial (worker-only chain hop)
        SET => &[Write, Imm, Imm, Imm, Imm], // cold_row, type_id, tile_reference, kind_reference, data
        BUILD_WALL => &[Imm, Imm, Imm], // start, end, object — writes nothing; the worker queues SETs
        // trait-rows-u32 F1: the u64 row rides the u32-word program SPLIT — obj, the FULL
        // reference, the def-interpreted data u16 (low half of its word).
        SET_NEED => &[Write, Imm, Imm], // obj, need reference, data (value u16)
        ACTIVATE_TRAIT => &[Write, Imm], // obj, the ACTIVE trait's full reference
        GRANT_CONDITION => &[Write, Imm, Imm], // obj, condition reference, data (remaining u16)
        // EXECUTE_INTERACTION is variable-arity (interaction, version, count, inputs×count —
        // all Imm; writes ride the verbs the worker queues) — framed in the reader like
        // CREATE/INIT_ZONE, no fixed signature.
        // QUEUE_STATE is variable-arity too (pawn, _reserved, count, entry-words×count —
        // all Imm; a display fan, no write set).
        CANCEL_INTENT => &[Imm, Imm], // pawn, order_event_reference — worker-memory resolution
        INV_ADD => &[Write, Imm],    // obj, item definition_reference (inventory F3)
        INV_REMOVE => &[Write, Imm], // obj, slot index (inventory F3)
        SPAWN_REQUEST => &[Imm, Imm, Imm], // xy, rot_def, variants — a pure request, no write set
        RESTAMP_NEED => &[Write, Imm], // obj, need_reference — the crossing re-stamp (survival D1)
        // CANCEL: the pawn is a WRITE so the cancel groups and serialises against its own
        // in-flight hops, and a Read so the worker sees the row it resolves from (server-chords F2).
        CANCEL => &[ReadWrite],
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
        // The **variable-arity** verbs: `INIT_ZONE cold_row type_id count item×count` (F12),
        // `CREATE def position count payload×count` (human-pawns F2) and `EXECUTE_INTERACTION
        // interaction version count inputs×count` (interactions F4). All three carry their
        // `count` as the third operand (`pos + 3`), so arity is `3 + count`. Others are fixed.
        let ar = if action == INIT_ZONE
            || action == CREATE
            || action == EXECUTE_INTERACTION
            || action == QUEUE_STATE
            || action == MOVE_CHORDS
        {
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
        // CREATE is variable-arity with EVERY operand imm (the write is the minted id, which is
        // not an operand) — it contributes nothing to either set. EXECUTE_INTERACTION likewise:
        // its inputs are UNTYPED words (an f32 bit pattern can alias any nibble), and its real
        // writes ride the typed verbs the worker queues (interactions F4).
        if inst.action == CREATE
            || inst.action == EXECUTE_INTERACTION
            || inst.action == QUEUE_STATE
            || inst.action == MOVE_CHORDS
        {
            // QUEUE_STATE: a display fan — every operand imm, no sets (intent-queue-ui F1).
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
            // CREATE def position count payload×count — no routed target (the minted id is the
            // write, handled by the worker's spawn arm, not the claim machinery).
            CREATE => {}
            // EXECUTE_INTERACTION — no routed target either: untyped inputs, queued-verb writes.
            EXECUTE_INTERACTION => {}
            // QUEUE_STATE — a display fan; CANCEL_INTENT — worker-memory resolution. Neither routes.
            // MOVE_CHORDS joins them: a stamped-route fan, every operand imm.
            QUEUE_STATE | CANCEL_INTENT | MOVE_CHORDS => {}
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
    fn move_step_frames_routes_hot_and_carries_its_serial() {
        // A continuation: bare MOVE_STEP obj dest serial (and a landing hop, PROMOTE-prefixed).
        let obj = 0x3080_0001;
        let words = vec![PROMOTE, MOVE_STEP, obj, 77, 13];
        let insts: Vec<_> = program(&words).collect::<Result<_, _>>().unwrap();
        assert_eq!(insts.len(), 2);
        assert_eq!(insts[1].action, MOVE_STEP);
        assert_eq!(insts[1].operands, &[obj, 77, 13]);
        // obj is in BOTH sets (reads its position, writes the next); serial/dest are imm.
        assert_eq!(write_targets(&words).unwrap(), vec![obj]);
        assert_eq!(read_targets(&words).unwrap(), vec![obj]);
        // Hot routing by the target's own nibble, exactly like MOVE_TO.
        assert_eq!(target_routes(&words).unwrap(), vec![(obj, Route::Hot)]);
        // Continuations never carry the intent latch.
        assert!(!asks_promote_event(&words));
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
        // CREATE def pos count=0 — all operands Imm; the minted id is not an operand.
        let p = vec![CREATE, 0xAAAA, 0xBBBB, 0];
        assert_eq!(write_targets(&p).unwrap(), Vec::<u32>::new());
        assert_eq!(read_targets(&p).unwrap(), Vec::<u32>::new());
        assert_eq!(target_routes(&p).unwrap(), Vec::<(u32, Route)>::new());
    }

    #[test]
    fn create_frames_variable_arity_with_a_payload() {
        // PROMOTE  CREATE def pos count=4 [PART slot def, PART slot def packed as 4 words]
        // then a trailing PROMOTE_EVENT to prove the payload frames the NEXT instruction.
        let p = vec![PROMOTE, CREATE, 0xAAAA, 0xBBBB, 4, 0x0001_0002, 0, 0x0001_0002, 1, PROMOTE_EVENT];
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 3);
        assert_eq!(insts[1], Instruction {
            action: CREATE,
            operands: &[0xAAAA, 0xBBBB, 4, 0x0001_0002, 0, 0x0001_0002, 1],
        });
        assert_eq!(insts[2].action, PROMOTE_EVENT);
        // Nothing enters the sets — every word is imm, the minted id is not an operand.
        assert_eq!(write_targets(&p).unwrap(), Vec::<u32>::new());
        assert_eq!(read_targets(&p).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn execute_interaction_frames_and_contributes_no_targets() {
        // EXECUTE_INTERACTION interaction version count=3 [pawn, need, amount(f32 bits)]
        // (interactions F4): frames like CREATE, and NOTHING enters the sets — the pawn
        // input is untyped (an f32 bit pattern can alias any nibble); the real writes ride
        // the verbs the worker queues.
        let amount = 3.0f32.to_bits();
        let p = vec![
            EXECUTE_INTERACTION, 0x8004_0010, 0, 3, 0x3080_0000, 0x8001_0010, amount,
            PROMOTE_EVENT,
        ];
        let insts: Vec<_> = program(&p).map(|r| r.unwrap()).collect();
        assert_eq!(insts.len(), 2, "the count frames the tail");
        assert_eq!(insts[0], Instruction {
            action: EXECUTE_INTERACTION,
            operands: &[0x8004_0010, 0, 3, 0x3080_0000, 0x8001_0010, amount],
        });
        assert_eq!(write_targets(&p).unwrap(), Vec::<u32>::new());
        assert_eq!(read_targets(&p).unwrap(), Vec::<u32>::new());
        assert!(target_routes(&p).unwrap().is_empty());
    }

    #[test]
    fn create_truncated_before_its_count_errors() {
        // CREATE needs at least def, position, count — this stops before count.
        let p = vec![CREATE, 0xAAAA, 0xBBBB];
        assert!(write_targets(&p).is_err());
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

    #[test]
    fn spawn_request_frames_and_round_trips() {
        // spawn-authority F1: fixed arity 3, no write set, and pack ↔ unpack agree —
        // the user's layout ([x:16|y:16] [rot:4|type:4|sub:12|kind:12] [nibbles]).
        let def = 0x3005_0057u32; // type 3, subtype 5, kind 5, variant 7 (variant ignored)
        let p = pack_spawn_request(110, 68, 2, def, &[5, 12]);
        assert_eq!(p[0], SPAWN_REQUEST);
        let mut it = program(&p);
        let inst = it.next().unwrap().expect("frames");
        assert_eq!(inst.action, SPAWN_REQUEST);
        assert_eq!(inst.operands.len(), 3);
        assert!(it.next().is_none());
        assert_eq!(write_targets(&p).unwrap(), Vec::<u32>::new(), "a pure request");
        let (x, y, rot, d, n) = unpack_spawn_request(p[1], p[2], p[3]);
        assert_eq!((x, y, rot), (110, 68, 2));
        assert_eq!(d, def & !0xF, "the def reconstructs with variant 0");
        assert_eq!(&n[..3], &[5, 12, 0], "part nibbles in order, tail zero");
    }
}

#[cfg(test)]
mod chord_tests {
    use super::*;

    /// `CANCEL` must make the pawn a WRITE, or it neither groups nor serialises against the hops
    /// it is cancelling — the reason it cannot reuse `CANCEL_INTENT` (server-chords F2).
    #[test]
    fn cancel_writes_the_pawn() {
        assert_eq!(arity(CANCEL), Some(1));
        let prog = [CANCEL, 0x3080_0001];
        assert!(write_targets(&prog).unwrap().contains(&0x3080_0001), "the pawn must be a write");
        assert!(read_targets(&prog).unwrap().contains(&0x3080_0001), "and a read — it resolves");
    }

    /// `MOVE_CHORDS` is variable-arity, so `count` sits at operand index 2 and the reader frames
    /// from there. If it is missing from the framing list, everything AFTER it in a program is
    /// mis-framed with no re-sync point — so the test puts a verb after it.
    #[test]
    fn move_chords_frames_and_a_verb_after_it_survives() {
        let prog = [
            MOVE_CHORDS, 0x3080_0001, 7, 4, 11, 22, 33, 44, // pawn, entry_id, count=4, payload
            CANCEL, 0x3080_0002, // a fixed-arity verb AFTER it — the mis-framing canary
        ];
        let insts: Vec<_> = program(&prog)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| panic!("framing failed: {e:?}"));
        assert_eq!(insts.len(), 2, "the trailing verb survived the variable-arity read");
        assert_eq!(insts[1].action, CANCEL);
        assert_eq!(insts[0].operands.len(), 7);
    }

    /// A display fan carries no sets. Omitting it from the skip list panics inside
    /// `collect_operands` at the `signature().expect("parsed, so known")`, so this covers it.
    #[test]
    fn move_chords_contributes_no_read_or_write_set() {
        let prog = [MOVE_CHORDS, 0x3080_0001, 7, 2, 11, 22];
        assert!(write_targets(&prog).unwrap().is_empty());
        assert!(read_targets(&prog).unwrap().is_empty());
        assert!(target_routes(&prog).unwrap().is_empty(), "a fan routes nowhere");
    }

    /// Append-only: the two new ids must not collide with anything already allocated.
    #[test]
    fn the_new_ids_are_free() {
        assert_eq!((MOVE_CHORDS, CANCEL), (20, 21));
        for id in 0..=19u32 {
            assert!(id != MOVE_CHORDS && id != CANCEL);
        }
    }
}
