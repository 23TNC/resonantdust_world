# Plan — trait-rows-u32

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md: the ONE u64 row (48-bit law, dead ZERO, def-declared u16 encodings,
      trait data reserved — F1/F6/F7/I9/I10); tier = VARIANT, level DELETED; the six
      categories (F2); the ACTIVE law (I5); sprint (F4). Acceptance: docs-check green.

## P1 — the row shape

- [x] codec: the ONE u64 row helper set (48-bit law) + the encoding decoder seam (F7,
      unknown refuses — I10); ALL payload entries grow a word; the six categories (F2).
      Acceptance: unit round-trips + 2×i8 lanes; level helpers DELETED.
- [ ] Modules: the `needs` column u64 in pawn + player_pawn; republished (WIPING, I2);
      st-bindings + edge bindings regenerated. Acceptance: publish green; a hand SET_NEED
      lands a u64 row.
- [ ] shared/content: the six tables parse into category-tagged lanes (per-level arrays →
      per-VARIANT, binds rename level→variant, params-by-REF — F6); the old two REFUSE
      (F2). Acceptance: unit — each category loads; the old tables refuse.

## P2 — the consumer sweep

- [ ] The ONE eval + every consumer on u64 rows (I1): worker, npc, wasm, webgl, panels;
      goldens + pins re-blessed deliberately (I4). Acceptance: builds green; the stack up;
      a pawn's panel shows traits/conditions/needs correctly post-sweep.
- [ ] content: every trait re-authored under its category in ONE commit (I3 — passive/
      constant homes per F2's list). Acceptance: registry seeds with ZERO divergence.

## P3 — active traits

- [ ] codec + worker: `ACTIVATE_TRAIT` — validates bind + active category + ≤3 slots
      (both doors, I5) + the availability predicate; executes the authored grants (F3).
      Acceptance: a WS drill activation grants the conditions.
- [ ] The full new-verb ritual (event-shard module + orchestrator/edge/worker/master/
      clients). Acceptance: the verb round-trips live.

## P4 — sprint

- [ ] content: `sprint` (active; grants `sprinting` + `sprint_cooldown` per F4) bound to
      a drill carrier. Acceptance: load green; the predicate refuses while cooling.
- [ ] The drill, on camera: faster hops while sprinting (I6), refusal mid-cooldown (I7),
      exhaustion on the cards, a player-pawn ACTIVATE via WS (I8). Acceptance: captures +
      logs in completed.md.

## P5 — the truth

- [ ] Docs + memory truth pass + stack bounce with arcs green (incl. the host's control arc
      re-proven on the new rows); **the user's eyes close the stream**. Acceptance:
      docs-check green; captures + logs in completed.md.
