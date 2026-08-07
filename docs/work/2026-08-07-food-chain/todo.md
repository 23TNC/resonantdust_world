# Plan — food chain

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [ ] VARIABLES.md: hunger + corpus (0..2 domain; the LEVELED corpus trait caps it,
      mint at effective max — F2), the need-check affordance (F4), the `self`
      location + spawn effect (F5/F6/I9). Acceptance: docs-check green.

## P1 — the machinery

- [ ] Loader: affordance `check = { need, cmp }` variant (F4); interaction
      `spawn = { thing, at = "adjacent" }` effect (F6); location rule `self` (I9).
      Acceptance: round-trip + refusal tests green.
- [ ] Eval: need MAX modifiers combine HIGHEST-wins (F2) in the ONE needs eval; the
      need-check affordance evaluates lazily beside stat checks. Acceptance: unit
      tests — bunny cap 1 / wolf cap 2; corpus ≤ 0 gates can_die.
- [ ] Worker mint: needs quantize the EFFECTIVE max from the kind's traits (I2).
      Acceptance: mint unit test — bunny corpus 1, wolf 2, one 0..2 encoding.
- [ ] Worker: the NEED-WRITE TRIGGER (F5, the user's law) — every need write sweeps
      the target's interactions for need-check affordances on THAT need, queueing
      the passers. Acceptance: SET_NEED corpus→0 queues death by itself.
- [ ] Pawn shard: the `remove` reducer (state + payload + needs rows; silent on
      absence — I3); module redeploy + master restart sequenced (I1). Acceptance:
      a drill remove deletes the rows; StateGone reaches a client.

## P2 — the corpus

- [ ] Content: needs hunger (I8) + corpus; the LEVELED corpus trait (`max = [1, 2]`
      — F2) + the four diet traits with their stats (F3); the four can_* affordances.
      Acceptance: golden diff = the authored rows.
- [ ] Content: things bunny/meat/plant_matter (F9); interactions death, forage,
      eat_plant_matter, eat_meat (F7); trait assignments per the design — corpus 1
      on bunny/humans, corpus 2 on wolves (I6). Acceptance: golden re-blessed; six
      consumers rebuilt.
- [ ] Client: textureless tint-rect placeholders draw a BLACK OUTLINE (F9 — a render
      rule for ALL placeholders, old and new). Acceptance: capture — shrub/logs/meat
      read as outlined placeholders, not bugs.

## P3 — the worker executes

- [ ] Worker: the death effect — validate can_die, spawn meat at the holder's cell,
      `remove` the pawn, clear its queue (I7); idempotent (I3). Acceptance: drill —
      SET_NEED corpus→0 ALONE kills; meat appears; the pawn vanishes everywhere.
- [ ] Worker: forage spawns plant_matter at the first EMPTY pathable adjacent cell
      (F6/I5); eats destroy the carrier + satisfy hunger. Acceptance: drill — a
      human forages then eats; hunger rises; the thing vanishes.

## P4 — the bunnies

- [ ] npc: the Bot's THING mirror (composed, kind-0 suppressed) + `nearest_thing`
      (F8/I4). Acceptance: unit-ish probe — an eaten thing leaves the scan.
- [ ] npc: `brains/bunnies.rs` — a GROUP of NPC_BUNNIES (default 3) minted/adopted
      near home; wander + drink + eat plant_matter when hungry; wolves gain
      eat-meat-when-hungry. Acceptance: soak — bunnies roam and eat; a hungry wolf
      finds meat.

## P5 — the verdict

- [ ] The chain drill: human forages → bunny eats plant matter → force a bunny's
      corpus to 0 → death drops meat → a hungry wolf eats it. Acceptance: captures +
      logs of every link.
- [ ] Docs + memory truth pass + a stack bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
