# Plan — stat-model

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the schema, documented before parsed

- [x] Write the four-family pawn model into `VARIABLES.md`: packed rows `kind:12|variant:4|data:16`
      (trait level / condition remaining-at-write / need value), the u16 fixed-point mapping
      ([F4](forks.md#f4)), and the `[[stat]]` schema (global bounds + `min_wins`/`max_wins`).
      Acceptance: every field P2 authors has a spelling here first. → §Pawn gameplay state
      rewritten; row = `data:16 | kind:12 | variant:4` (low 16 ≡ the def ref's low 16); winner
      field spelled `winner = "min"|"max"`.
- [x] Document the reworked defs: `[[trait]]` per-level stat contributions + need modifiers,
      `[[affordance]]` as a structured predicate ([F10](forks.md#f10)) listed BY `[[interaction]]`,
      carrier `interactions = [{name, magnitude}]`, thing `traits` bindings with level
      ([F5](forks.md#f5)/[F9](forks.md#f9)/[F11](forks.md#f11)). Acceptance: walks +
      can_move_ground worked example in the doc. → schema block rewritten (walks
      `add = [24, 12, 6]` so the wolf's level-2 value equals its `speed` — I10); carriers moved
      to `interactions = [...]`.
- [x] Document the combiner law ([F6](forks.md#f6)) and the re-stamp law ([F7](forks.md#f7)) with
      a hand-computed quenched piecewise window ([I4](issues.md#i4)). Acceptance: the worked
      window's numbers appear in the doc and P1's eval test reuses them. → sums/intersections/
      winner/rate-product rules + the 39.9994 − 8.3333 − 6.4815 ≈ 25.1846 window in VARIABLES.
- [x] TABLES.md: the needs sub-table row `(entity, packed, set_tic)`; payload TRAIT(count 1) /
      CONDITION(count 2) entries; NEED leaves the payload ([I1](issues.md#i1)). ACTIONS.md:
      SET_NEED/GRANT_CONDITION operand meanings. Acceptance: `bin/rd docs-check` green. →
      opcodes RETIRE values (NEED 2 + CONDITION 3 dead; CONDITION=4, TRAIT=5), `needs` table
      documented (uid = entity:32|need_key:16, zone-slaved, no log twin); SET_NEED arity 3→2;
      docs-check green.

## P1 — codec + the shared evals

- [ ] Codec: pack/unpack for the 16+16 gameplay row, the ONE f32↔u16 quantize/dequantize pair
      (rounding defined at the fn, [I2](issues.md#i2)), `GAMEPLAY_CATEGORIES` appends `stat`
      (append-only law + test, [I5](issues.md#i5)). Acceptance: round-trip tests incl. both
      domain ends.
- [ ] Codec payload: TRAIT/CONDITION entries in the packed shape; the NEED opcode DELETED
      ([I11](issues.md#i11)); count-guard posture kept. Acceptance: old-shape-ignored test green;
      `grep` finds no payload NEED composer or reader.
- [ ] Loader: `[[stat]]`; trait level tables + contributions + need modifiers; structured
      predicates validated against the stat registry; interaction `affordances`; carrier
      `interactions`; thing trait bindings. Refusals for unknown/empty/dangling. Acceptance:
      per-category crate tests incl. each refusal.
- [ ] Shared eval: stat derivation `clamp(sum, combined bounds)` + the affordance predicate check
      over (trait, condition) rows ([F8](forks.md#f8)). Acceptance: the user's combiner cases
      pinned — 3..7 ∧ 4..8 → 4..7; 2..3 ∧ 4..5 → the authored winner.
- [ ] needs_eval reworked: packed fixed-point rows, modifier sets through the SAME combiner,
      piecewise integration across a DERIVED condition expiry, wrap guards at both new tic seams
      ([I4](issues.md#i4)/[I6](issues.md#i6)). Acceptance: the P0 worked window reproduced
      exactly; future-stamp tests green.
- [ ] Extend the golden dump (stats, leveled traits, predicates, carrier interactions,
      fixed-point probes) and re-bless; 2-pass gate green. Acceptance: every diff line accounted
      for in completed.md ([I7](issues.md#i7)).

## P2 — the corpus re-expressed

- [ ] Author `[[stat]]` metabolism + ground_speed; rework biological_lifeform (level 1 → +1
      metabolism) and author walks (1: 60, 2: 50, 3: 40 tics/tile → ground_speed); affordances
      can_drink / can_move_ground; drink gains `affordances = ["can_drink"]`; quenched gains the
      thirst rate ×0.5 modifier ([I9](issues.md#i9)). Acceptance: loads; `rd content-check` green.
- [ ] Rebind the carriers: water `interactions = [{ name = "drink", magnitude = 3 }]`; wolf
      `traits = ["biological_lifeform", { name = "walks", level = 1 }]`; add the I10 guard test
      (derived ground_speed == the `speed` field). Acceptance: golden shows exactly these rows;
      guard test green.

## P3 — spacetime + the worker

- [ ] Pawn shard: the needs sub-table via the shard-tables macro family; CREATE mints trait rows
      + need rows from the def ([F11](forks.md#f11)). Acceptance: a fresh wolf read via sql shows
      walks level 1 + thirst full with a stamped set_tic; the new subscription checked LIVE
      ([I3](issues.md#i3)).
- [ ] Reducers: SET_NEED targets the sub-table (quantized u16 write); GRANT_CONDITION writes
      remaining-at-write and RE-STAMPS affected needs ([F7](forks.md#f7)). Acceptance: sql
      read-back after a drill grant shows the condition row AND the re-stamped need row.
- [ ] Worker: the interaction arm on the new rows — affordance PREDICATES gate execution from
      derived stats, ONE quantization at write, effects still queued at `master+4`. Acceptance:
      drills — a predicate refusal logs and splices nothing; an on-water sip lands exactly +3.0
      on the fixed-point value.
- [ ] Fan the needs table: edge + client engine + npc grow the row lane ([I8](issues.md#i8)).
      Acceptance: a need row update observed over the wire in BOTH the npc log and the browser
      console.

## P4 — the npc + the client

- [ ] npc wolves: thirst from the sub-table, traits from pawn rows, availability via the shared
      predicate eval (the string-set gate deleted, [I11](issues.md#i11)). Acceptance: the
      unprompted arc green — Thirsty → walk to water → sip → Quenched.
- [ ] Drill the rate modifier live: quenched VISIBLY halves depletion and the slope changes at
      its derived expiry ([I4](issues.md#i4)). Acceptance: logged satisfaction across
      grant→expiry matches the shared eval's piecewise numbers.
- [ ] wasm + panel: `pawnConditions`/`pawnMood`/`pawnNextCrossing` move to the new-shape inputs;
      the cards render live. Acceptance: browser captures — a Thirsty card and a
      quenched-SLOWED next-crossing.

## P5 — the verdict

- [ ] Docs + memory truth pass; the I11 delete-greps run clean (no payload NEED, no f32-word
      composers, no `requires`/`variants`, no string-set availability). Acceptance:
      `bin/rd docs-check` green; greps recorded in completed.md.
- [ ] Cold-boot the stack; standing drills (trips, thirst crossing, panel) + the unprompted
      drink arc on the NEW model green together; **the user's eyes close the stream**.
      Acceptance: captures + logs in completed.md.
