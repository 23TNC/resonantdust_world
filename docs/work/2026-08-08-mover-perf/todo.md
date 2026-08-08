# Plan — mover perf

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the kind and the harness

- [x] content: the `debug_mover` pawn kind (F1 — walks 2, no needs, no light,
      plain gray part). Acceptance: load + content-check green; golden
      re-blessed.
- [x] npc: `brains/torches.rs` → `brains/debug.rs` (F2 — `NPC_KIND` default
      debug_mover, `NPC_COUNT`; the `torches` arm and `NPC_TORCHES` DIE);
      main.rs registers `debug`. Acceptance: npc builds clean; grep shows no
      `torches` arm; the harness line recorded in completed.md (I8).

## P1 — the rows

- [ ] Scene reset (I1): wolves + bunnies + npc stopped, pawn module wiped,
      sims restarted, seed guard quiet, 0 pawns. Acceptance: SQL count 0;
      arcs green.
- [ ] Row 1 — `NPC_COUNT=8`: mint (record actual N — I4), settle ≥1 min,
      then a ≥3-min soak sampling the worker's `tic=X master=Y` pair every
      ~10 s + `events=` distribution + pacing + client `__framecost` +
      capture. Acceptance: the row + lag series in completed.md.
- [ ] Row 2 — `NPC_COUNT=16`: same, ≥3-min soak. Acceptance: the row in
      completed.md.
- [ ] Row 3 — `NPC_COUNT=24`: same, **≥10-min soak** (I2) + the zone spread
      of the population (I3). Acceptance: the row + the full lag series in
      completed.md.

## P2 — the verdict, and the truth

- [ ] The verdict: per row IN-STEP / STABLE-BEHIND / GROWING from the lag
      SHAPE (I2), the dark-vs-lit comparison against torch-perf's table (F4
      — re-run the lit twin under this protocol only if the numbers argue),
      successors named (I7). Acceptance: the table + series stand alone.
- [ ] Restore the world (cast wiped — debug movers are deathless; wolves +
      bunnies back and re-minted); docs + memory truth pass; **the user's
      eyes close the stream**. Acceptance: docs-check green; arcs green.
