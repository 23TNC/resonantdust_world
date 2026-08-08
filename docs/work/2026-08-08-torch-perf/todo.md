# Plan — torch perf

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the corpus

- [x] content: the `debug_torch` pawn kind (F1 — type pawn, flat tint part,
      `walks` level 2, constant `emit_light` level 1, NO needs); the human's
      drill bind REVERTED (F4). Acceptance: load + content-check green; golden
      re-blessed; no `emit_light` remains on a human kind.

## P1 — the torches brain

- [x] npc: `brains/torches.rs` (F2 — the bunnies group shape: SPAWN_REQUEST
      mints to `NPC_TORCHES` with pathable politeness + retry, adopt, wander
      forever with a fresh move_to per arrival inside the home radius) +
      the main.rs registry arm. Acceptance: npc builds; unit-free (the shape
      is proven) — the live mint is P2 row 1's first check.
- [x] Wire the container run: `bin/sim run npc` env passthrough already
      carries `NPC_BRAIN`/`NPC_TORCHES`/`NPC_HOME` — document the exact
      command in completed.md. Acceptance: a torches container starts and
      logs its config.

## P2 — the measurement

- [x] Scene reset (F3/I1/I7): stop wolves + bunnies + npc, redeploy the pawn
      module (cast wiped), restart master/orch/worker, verify zero pawns and
      the seed guard quiet. Acceptance: SQL shows 0 entity_state rows; arcs
      green.
- [ ] Row 1 — `NPC_TORCHES=8`: mint, verify 8 wanderers in-window at zoom 1
      (I5), soak ≥2 min, record `__lightcost` + `__framecost` spreads (I3),
      the worker compose line (I6), a capture. Acceptance: the row in
      completed.md.
- [ ] Row 2 — `NPC_TORCHES=16`: top up (adopted pawns count toward the cap),
      same soak + reads + capture. Acceptance: the row in completed.md.
- [ ] Row 3 — `NPC_TORCHES=24`: same. Acceptance: the row in completed.md;
      the I8 glError verdict (benign hook read or real) recorded by now.

## P3 — the verdict, and the truth

- [ ] The findings: the three-row table + the curve reading (linear? a
      cliff?) in completed.md; anything actionable lands in issues.md as a
      named successor (mitigations are the user's earlier call — "later").
      Acceptance: the table stands alone without this chat.
- [ ] Restore the world: wolves + bunnies containers back, their casts
      re-mint; docs + memory truth pass; **the user's eyes close the
      stream**. Acceptance: docs-check green; arcs green; captures + logs in
      completed.md.
