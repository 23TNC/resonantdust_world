# Plan — lumberjack

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] ACTIONS.md: the intent-queue law — EPHEMERAL per-pawn list (F1), cap 5, fresh
      order REPLACES (F3), duration rides queue_at, every completion RE-VALIDATES →
      no-op when stale (I2). Acceptance: docs-check green; law readable alone. → new
      §"The intent queue, and interactions that cost tics" after §Movement: one
      composer, replace-whole, per-kind advancement (in-pass / final-hop-by-serial /
      queue_at +N), the no-op law; docs-check green.
- [x] VARIABLES.md: the `"adjacent"` location rule (F2), `duration`,
      `destroy = "carrier"` + reserved `yields` (F5) in the TOML schema. Acceptance:
      docs-check green. → the three location rules enumerated on drink's block (drink
      shown `adjacent`), duration's completion/re-validate comment, a full `cut_down`
      example block with `destroy = "carrier"` + the RESERVED `yields` note; green.

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

- [ ] The ephemeral queue (F1) + composition: out of place → `[move_to(adjacent), act]`;
      in place → `[act]`; fresh order replaces; cap-5 rejects whole (F3). Acceptance:
      composed queue logged; a mid-walk re-order leaves ONE fresh queue.
- [ ] Advancement (I4): `duration = 0` completes in-pass; move_to advances on the
      chain's FINAL hop, keyed by trip serial (a superseded chain advances nothing).
      Acceptance: walk-lands → next intent queued in the log; queued drink still +3.
- [ ] Duration via queue_at: a `duration = N` head queues its COMPLETION at `+N`, which
      RE-VALIDATES affordance + location and no-ops when stale (I2). Acceptance: fire
      tic = start + N in the log; a walked-away pawn's completion logs the no-op.

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
- [ ] Cold boot: pending intents drop, in-flight completions survive + re-validate —
      state what actually happened (F1); fell + drink + wolf arcs green; **the user's
      eyes close the stream**. Acceptance: captures + logs in completed.md.
