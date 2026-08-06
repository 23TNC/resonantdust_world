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
      winner/rate-product rules + the 40.0 − 8.3333 − 6.4815 ≈ 25.1852 window in VARIABLES
      (numbers corrected in P1 — the 40.0 stamp dequantizes exactly).
- [x] TABLES.md: the needs sub-table row `(entity, packed, set_tic)`; payload TRAIT(count 1) /
      CONDITION(count 2) entries; NEED leaves the payload ([I1](issues.md#i1)). ACTIONS.md:
      SET_NEED/GRANT_CONDITION operand meanings. Acceptance: `bin/rd docs-check` green. →
      opcodes RETIRE values (NEED 2 + CONDITION 3 dead; CONDITION=4, TRAIT=5), `needs` table
      documented (uid = entity:32|need_key:16, zone-slaved, no log twin); SET_NEED arity 3→2;
      docs-check green.

## P1 — codec + the shared evals

- [x] Codec: pack/unpack for the 16+16 gameplay row, the ONE f32↔u16 quantize/dequantize pair
      (rounding defined at the fn, [I2](issues.md#i2)), `GAMEPLAY_CATEGORIES` appends `stat`
      (append-only law + test, [I5](issues.md#i5)). Acceptance: round-trip tests incl. both
      domain ends. → `object::pack_gameplay_row`+key/data/reference, new `value.rs`
      (quantize half-away-from-zero, re-stamp stability test), palette test freezes all six
      ids. 71/71 codec tests.
- [x] Codec payload: TRAIT/CONDITION entries in the packed shape; the NEED opcode DELETED
      ([I11](issues.md#i11)); count-guard posture kept. Acceptance: old-shape-ignored test green;
      `grep` finds no payload NEED composer or reader. → values 2+3 RETIRED (never reuse),
      CONDITION=4/TRAIT=5; upsert matches the row's LOW 16 so a re-grant with different
      remaining refreshes in place; retired-value test feeds old entries and reads nothing.
- [x] Loader: `[[stat]]`; trait level tables + contributions + need modifiers; structured
      predicates validated against the stat registry; interaction `affordances`; carrier
      `interactions`; thing trait bindings. Refusals for unknown/empty/dangling. Acceptance:
      per-category crate tests incl. each refusal. → all six categories; leveled arrays must
      agree on length; `the_stat_model_refusals_are_loud` covers predicate/level/winner/F13
      refusals; F13 recorded (a DERIVED condition may not modify needs — the circularity cut).
- [x] Shared eval: stat derivation `clamp(sum, combined bounds)` + the affordance predicate check
      over (trait, condition) rows ([F8](forks.md#f8)). Acceptance: the user's combiner cases
      pinned — 3..7 ∧ 4..8 → 4..7; 2..3 ∧ 4..5 → the authored winner. → new `stat_eval.rs`:
      `combine_bounds`/`stat_value`/`affordance_passes`/`interaction_available` +
      `condition_remaining` (F3 + future-stamp guard); the F6 cases are `the_users_combiner_cases_hold`.
- [x] needs_eval reworked: packed fixed-point rows, modifier sets through the SAME combiner,
      piecewise integration across a DERIVED condition expiry, wrap guards at both new tic seams
      ([I4](issues.md#i4)/[I6](issues.md#i6)). Acceptance: the P0 worked window reproduced
      exactly; future-stamp tests green. → `rate_windows` (traits unbounded, stored rows cut at
      remaining-at-set), piecewise `depletion` + `crossing_elapsed`; the 25.1852 window test +
      a piecewise-crossing test (expiry wake at 100, band at 101); wrap/future-stamp kept.
- [x] Extend the golden dump (stats, leveled traits, predicates, carrier interactions,
      fixed-point probes) and re-bless; 2-pass gate green. Acceptance: every diff line accounted
      for in completed.md ([I7](issues.md#i7)). → 44+/19−, all reviewed (completed.md); probes
      BIT-STABLE (quantization exact at the probe points); native codec 71 + content 37+3,
      wasm32 pkg rebuilt with the stride-2 needs-row surface.

## P2 — the corpus re-expressed

- [x] Author `[[stat]]` metabolism + ground_speed; rework biological_lifeform (level 1 → +1
      metabolism) and author walks (1: 60, 2: 50, 3: 40 tics/tile → ground_speed); affordances
      can_drink / can_move_ground; drink gains `affordances = ["can_drink"]`; quenched gains the
      thirst rate ×0.5 modifier ([I9](issues.md#i9)). Acceptance: loads; `rd content-check` green.
      → landed WITH P1's golden re-bless (the schema change makes the old corpus refuse — the
      same one-build-unit coupling as last stream); walks re-based to `[24, 12, 6]` so a level
      EQUALS `speed = 12` (I10; the 60/50/40 illustration could not satisfy the guard).
      content-check clean (7 files).
- [x] Rebind the carriers: water `interactions = [{ name = "drink", magnitude = 3 }]`; wolf
      `traits = ["biological_lifeform", { name = "walks", level = 1 }]`; add the I10 guard test
      (derived ground_speed == the `speed` field). Acceptance: golden shows exactly these rows;
      guard test green. → wolf binds walks at LEVEL 2 (the 12-tics/tile slot; the plan's
      level-1 guess predated the re-based table); golden rows verbatim (`water [("drink",
      3.0)]`, `wolf traits=[("biological_lifeform", 1), ("walks", 2)]`); the guard is
      `the_wolfs_derived_ground_speed_equals_its_speed_field` (derives 12.0 == 12).

## P3 — spacetime + the worker

- [x] Pawn shard: the needs sub-table via the shard-tables macro family; CREATE mints trait rows
      + need rows from the def ([F11](forks.md#f11)). Acceptance: a fresh wolf read via sql shows
      walks level 1 + thirst full with a stamped set_tic; the new subscription checked LIVE
      ([I3](issues.md#i3)). → `needs` table (uid entity:32|key:16, zone-slaved by the state
      hook) + spawn gains `needs`; the WORKER composes both sidecars (`mint_sidecars` — the
      module holds no corpus). sql: thirst `0xFFFF_0010` @918 + payload traits bio@1/walks@2
      (level 2 per the re-based table). Live subs: worker sips off it, npc + browser fan green.
- [x] Reducers: SET_NEED targets the sub-table (quantized u16 write); GRANT_CONDITION writes
      remaining-at-write and RE-STAMPS affected needs ([F7](forks.md#f7)). Acceptance: sql
      read-back after a drill grant shows the condition row AND the re-stamped need row. →
      set_need arity 5→4 (packed row); grant_condition packs remaining|key; the RE-STAMP moved
      to COMPOSERS ([I12](issues.md#i12) — no corpus in the module); drill sql: quenched
      `3600@3213` + thirst re-stamped at the SAME tic 3213.
- [x] Worker: the interaction arm on the new rows — affordance PREDICATES gate execution from
      derived stats, ONE quantization at write, effects still queued at `master+4`. Acceptance:
      drills — a predicate refusal logs and splices nothing; an on-water sip lands exactly +3.0
      on the fixed-point value. → off-water NPC_INTERACT → `the tile does not offer this
      interaction (F8: on-tile only)`, zero splices; sips exact (`from=17.241013 to=20.241013`).
      The predicate NEGATIVE case is undrillable (every pawn kind carries biological_lifeform)
      — the gate runs positively on every sip via the SAME `interaction_available` the npc asks.
- [x] Fan the needs table: edge + client engine + npc grow the row lane ([I8](issues.md#i8)).
      Acceptance: a need row update observed over the wire in BOTH the npc log and the browser
      console. → `Need` frame (edge zone sub + snapshot replay) → `Event::PawnNeed` →
      npc buffer + MoverLayer.pawnNeeds; proven by the npc's band-change evals tracking sip
      set_tics AND the browser panel's cards (below) — both read ONLY the fanned rows.

## P4 — the npc + the client

- [x] npc wolves: thirst from the sub-table, traits from pawn rows, availability via the shared
      predicate eval (the string-set gate deleted, [I11](issues.md#i11)). Acceptance: the
      unprompted arc green — Thirsty → walk to water → sip → Quenched. → NPC_THIRST=12 →
      Thirsty@1436 → `heading to water (102,68)` → sips +3.0 → band clears at 35.17 →
      `["quenched"] mood 0.7`; trait rows read from the PAYLOAD (runtime truth, F11);
      `usable_interaction` = tile_interactions × `interaction_available`.
- [x] Drill the rate modifier live: quenched VISIBLY halves depletion and the slope changes at
      its derived expiry ([I4](issues.md#i4)). Acceptance: logged satisfaction across
      grant→expiry matches the shared eval's piecewise numbers. → between sips 6 tics apart
      thirst dropped 0.0138 = 6·100/43200 EXACTLY (the halved rate); the npc's crossing
      prediction 1713 (73 tics for 0.17 at half slope) came true at the next eval (1718 flip)
      — the piecewise crossing arithmetic live. Found+fixed en route: the combined
      NPC_THIRST+NPC_GRANT drill raced (the re-stamp composed the PRE-seed value the same
      tick) — the grant lane now defers 3 s past init; re-run holds 49.97%.
- [x] wasm + panel: `pawnConditions`/`pawnMood`/`pawnNextCrossing` move to the new-shape inputs;
      the cards render live. Acceptance: browser captures — a Thirsty card and a
      quenched-SLOWED next-crossing. → stride-2 needs rows beside the payload; captures at
      `:5174/?focus=102,68`: wolf `0x30800001` mood 70% **Quenched +0.20 3170t**, then the
      thermostat window mood 55% **Thirsty −0.15 + Quenched +0.20 2433t** (priority order);
      the slowed crossing is the npc's `next_crossing=1713` (unhalved would be ~37 tics out).

## P5 — the verdict

- [x] Docs + memory truth pass; the I11 delete-greps run clean (no payload NEED, no f32-word
      composers, no `requires`/`variants`, no string-set availability). Acceptance:
      `bin/rd docs-check` green; greps recorded in completed.md. → greps EMPTY (two doc
      comments only); component docs already clean (P0 moved the authoritative text);
      memories: needs-moodlets rewritten (fixed-point/sub-table/piecewise),
      stat-model-delivered written, interactions-delivered superseded-in-part, index rows
      updated. docs-check green.
- [x] Cold-boot the stack; standing drills (trips, thirst crossing, panel) + the unprompted
      drink arc on the NEW model green together; **the user's eyes close the stream**.
      Acceptance: captures + logs in completed.md. → edge re-exec + master/orch/worker/npc
      bounced together: registry re-seeded idempotently (193, no collisions), corpus loaded,
      the EXISTING wolf ADOPTED (no re-mint) and woke Thirsty off its PERSISTED row (49.97%
      lazily drained to 33.46 across the downtime), walked to (102,68), sipped to 36.46
      UNPROMPTED, Quenched mood 0.7; the cold-loaded panel shows **Quenched +0.20 3388t**.
      **The user's look is the remaining close.**
