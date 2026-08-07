# Plan — intent-queue-ui

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] VARIABLES.md: the interaction `queue = { … }` schema (F2) and the details panel's
      shift-right + left vertical strip layout. Acceptance: docs-check green. → the
      layout paragraph (strip left, content right, bottom = active, client-derived
      ring) beside the panel-ordering law; the `queue` block documented on drink with
      all seven fields, move_to (0.6/none) and cut_down authored in the schema
      examples; green.
- [ ] ACTIONS.md: `QUEUE_STATE` (worker-only, promoted — F1) and `CANCEL_INTENT pawn
      order_ref` with the phase-by-phase cancel law + the ephemeral cancelled-set
      caveat (F3/F4, I4). Acceptance: docs-check green.

## P1 — the corpus

- [ ] Loader: `QueueVisual` on `InteractionParams` (F2 defaults), the `queue` sub-table
      parsed + validated (progress ∈ cw|ccw|none). Acceptance: round-trip + refusal
      tests green.
- [ ] Author drink + cut_down (fill, green, cw, cancelable) and move_to (none, 0.6);
      golden re-blessed; ALL SIX consumers rebuilt, wasm cache-busted (I2).
      Acceptance: golden diff = the authored rows; edge hot-reload clean.

## P2 — the fan

- [ ] `QUEUE_STATE` behind ONE `fan_queue(pawn)` helper at every mutation site (I1);
      entries = order ref + interaction ref + phase + (started, fire) when executing
      (I3). Acceptance: a fan per mutation in a compose→schedule→complete drill.
- [ ] The client mirror: wire decode → `WasmClient` event → a per-pawn QueueModel.
      Acceptance: a browser probe tracks the worker's drill exactly.

## P3 — the strip

- [ ] The panel reshapes: content right, LEFT strip of TOML-styled circles, bottom =
      active (F5, I6). Acceptance: captures — a walk+chop shows the small move circle
      at bottom, the chop circle above.
- [ ] Tooltips (`hover`, default label) + the active PROGRESS ring (SVG stroke, cw/ccw,
      fill/empty; percentage COMPUTED from the tic estimate — I5). Acceptance: mid-chop
      ~50% capture; a probe checks the percentage against (started, fire).

## P4 — cancel

- [ ] `CANCEL_INTENT` end to end: pending removed; executing-timed only if `cancelable`
      → the completion logs a `cancelled` NO-OP; walking clears the queue (F3); every
      path refans. Acceptance: all three paths in one drill's log.
- [ ] The UI click sends the entry's order_ref (F4); drills: cancel a pending chop,
      cancel an executing chop mid-ring, the strip updates. Acceptance: captures +
      logs.

## P5 — the verdict

- [ ] Docs + memory truth pass: lumberjack I7's "no fan" claim amended; index row
      records delivery. Acceptance: docs-check green.
- [ ] Cold boot: the strip self-corrects from the next fan after a bounce (state what
      actually happens); standing arcs beside; **the user's eyes close the stream**.
      Acceptance: captures + logs in completed.md.
