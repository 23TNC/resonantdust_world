# Todo — hot-sync

_Items tick in place; the box is the move. Design: [`README`](README.md) · [`forks`](forks.md).
Visual items verify at ≥3 zooms + during a transition, tab VISIBLE (hidden tabs freeze the
chase and fake every measurement — pawn-render's recorded observation)._

---

## P0 · Measure the desync (numbers before surgery)

- [ ] Instrument the skew: per frame, sample (a) the mover's rendered anchor (`m.rx/ry`),
      (b) the shadow record's decoded position, (c) whether the warm square / hot light
      rect / receiver rect re-baked this frame (counters per channel). Log a ~10 s walking
      trace. Acceptance: the trace QUANTIFIES the artifact — lag magnitude (tiles) and
      cadence per pair (sprite↔record, sprite↔light-rebake, record↔receiver) recorded in
      `completed.md` as the before-numbers the fix is judged against.

## P1 · One hot dirty

- [ ] Frame order: `moverLayer.tick()` runs BEFORE `panel.tick()` in `WorldScene.update()`
      so the frame renders THIS frame's mover state and every hot consumer sees the same
      snapshot. Audit MoverLayer for any dependence on the old order (it reads
      `client.ticDelta` and mutates prims — nothing viewport-order-sensitive expected;
      verify, don't assume). Acceptance: a single-frame step test (pause rAF, advance once)
      shows sprite + record + rects all reflecting the same position.
- [ ] The unified entry: one Viewport call (e.g. `moverDirty(prim, oldBox, newBox)`) invoked
      from MoverLayer's `applyVisual` at the SAME eps crossing that re-bakes the sprite —
      it (a) dirties the warm squares, (b) queues the hot cls-1 light/shadow rects
      (old ∪ new + cascaded light reaches), (c) queues the receiver rects, (d) rewrites the
      shadow record from the SAME position snapshot. `buildCasters`' change-detection stays
      as a BACKSTOP (first sight, def swaps, zoom) but no longer originates per-move dirt.
      Acceptance: with the backstop instrumented, a full walking trip originates ZERO
      per-move dirt from `buildCasters` — everything flows through the one call.
- [ ] Cadence/quantum: both consumers step on the SAME crossing — the record rewrite happens
      on every eps bake (its unit-quantised encode may round, but it re-stamps in lockstep;
      the ≤1-unit rounding is display-invisible at 16 units/tile). Acceptance: the P0 trace
      re-run shows sprite↔record cadence 1:1 (every sprite re-bake = a record rewrite).
- [ ] Budget coupling ([F4](forks.md#f4)): mover squares bake UNBUDGETED (movers are few —
      the wolf's slots always land the same frame; the budget continues to govern cold +
      streaming). Acceptance: a deliberately-starved budget test (drop `BAKE_BUDGET` low with
      heavy streaming) never defers a mover square while cold streaming visibly staggers.

## P2 · Lockstep verified

- [ ] The lockstep drill: re-run the P0 instrumentation over a ≥10-trip soak — assert the
      rendered anchor and the hot-map response (lit body + carved shadow) stay within ONE
      UNIT (1/16 tile) every frame, at zooms 1 / 0.5 / 0.25 + a transition. fps holds the
      120 baseline at zoom 1 and 0.25. Acceptance: numbers + screenshots in `completed.md`;
      the user confirms the 1:1 feel on the live tab (their eyes are the final oracle —
      they caught this one).
- [ ] Wrap: docs touched where behavior changed (`intent/tiered-lighting.md` status line:
      hot = ONE dirty; a note in the pawn-render folder linking here), memory updated,
      work-index row → done.
