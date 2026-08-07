# Plan — lumberjack

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [ ] ACTIONS.md: the intent-queue law — one queue per pawn, cap 5, fresh order REPLACES
      (F3), head stamps `started_tic`, pops at `elapsed ≥ duration` (I2 stated).
      Acceptance: docs-check green; the law readable without this folder.
- [ ] TABLES.md + VARIABLES.md: the pawn-shard `intents` row (F1), the `"adjacent"`
      location rule (F2), `duration`, `destroy = "carrier"` + reserved `yields` (F5).
      Acceptance: docs-check green.

## P1 — the corpus

- [ ] interactions.toml: `logging` stat, `lumberjack` trait, `can_fell_trees`, `cut_down`
      (adjacent, duration 30, destroy carrier — F7); `drink` → adjacent; the consumed I9
      RESERVED note deleted. Acceptance: content-check clean.
- [ ] things.toml: tree/shrub/cactus carry `cut_down` (F6); humans author lumberjack 1
      (F4); golden re-blessed (I5). Acceptance: golden diff = the authored rows only;
      loader round-trips `duration`/`destroy`.

## P2 — adjacency

- [ ] `"adjacent"` in BOTH gates — worker location match + wasm menu offer (Chebyshev ≤ 1
      inclusive); the worker resolves THING carriers from thing ⊕ overlay (I1).
      Acceptance: shared-crate rule tests; native+wasm gates green.
- [ ] Drill: a human drinks from the SHORE tile; the wolf's stand-on-water drink still
      lands (F2, I6). Acceptance: both `satisfied=Some("thirst")` in one worker log.

## P3 — the intent queue

- [ ] The pawn-shard `intents` row (F1): table, state-claim slaving, zone rekey, worker
      the only writer. Acceptance: sql shows a queued row; subscription SQL live-checked.
- [ ] Worker composition: out of place → `[move_to(adjacent), act]`; in place → `[act]`;
      fresh order replaces; cap-5 rejects whole (F3, I7). Acceptance: composed queue in
      the log; a mid-chop re-order leaves ONE fresh queue.
- [ ] The executor: move_to head pops on arrival; `duration > 0` head stamps
      `started_tic`, pops at elapse, effects queue at pop (I2). Acceptance: start/elapsed/
      pop tics logged; drink-via-queue still +3 exact.
- [ ] The intents wire: edge frame + `Event::PawnIntents` + client mirror (I4).
      Acceptance: browser probe watches the queue arrive and drain.

## P4 — timber

- [ ] A FRESH lumberjack (I8: worker restarted, re-minted) menu-clicks a FAR tree: Cut
      Down offered, queue walks then chops 30 tics, the cell SETs 0, tree AND shadow
      leave the render (I3). Acceptance: walk+chop+destroy log; before/after captures.
- [ ] The refusals + neighbours: a wolf's tree menu offers no Cut Down; shrub and cactus
      fell; drink + wolf trips beside (F6). Acceptance: refusal + two fells + both
      species' arcs in one session.

## P5 — the verdict

- [ ] Docs + memory truth pass: memories note timed interactions + the queue; consumed
      RESERVED notes gone; index row records delivery. Acceptance: docs-check green.
- [ ] Cold boot: the queue's bounce behavior STATED (resumes or restates), fell + drink +
      wolf arcs green together; **the user's eyes close the stream**. Acceptance:
      captures + logs in completed.md.
