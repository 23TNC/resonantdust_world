# Completed — intent-queue-ui

## 2026-08-07 — P0–P5, delivered in one arc

- **P0/P1**: the `queue = { … }` schema + strip layout in VARIABLES; `QUEUE_STATE` (13)
  and `CANCEL_INTENT` (14) palette rows in ACTIONS; `QueueVisual` in the loader
  (defaults, closed ring-direction set, positive size, colors via the shared helper —
  round-trip + refusal unit tests); drink/cut_down authored fill/green/cw/cancelable,
  move_to none/0.6; golden re-blessed; six consumers rebuilt clean.
- **P2, the fan**: `PawnQueue::fan_program` — the ONE helper — at every mutation site;
  entries keyed by WORKER-MINTED `entry_id`s (a composed [walk, act] pair = two ids
  from one order; `advancing` carries identity across re-queues); the COMPLETE pass
  zones the fan from the PAWN. Client: core decode → `Event::QueueState` → wasm
  marshal → `WasmClient` → the `IntentQueues` mirror (serial dedup, `__queues`).
  Drilled: the executing entry carried `(started, fire = +30)` exact and drained on
  completion. FOUND: the event-shard MODULE's compiled codec rejects unknown verbs
  ASYNC-INVISIBLY — redeployed (transient data only) + sim crates rebuilt (the
  stale-binary guard caught the master itself).
- **P3, the strip**: `IntentStrip` in a flex-row details panel (content shifted right;
  the CONDITION CARDS' origin cleared of the strip column — the user's mid-drill
  call); circles styled by the wasm `queueVisual` accessor; custom tooltip; the ring =
  SVG dashoffset RECOMPUTED per frame from the tic estimate. Captured: the small
  neutral walk circle at the BOTTOM, the green chop above. The ring probe swept
  0.33→0.60→0.83→1.0 across one 30-tic chop (the ~50% acceptance in probe form; a
  pixel capture of the 5-s ring slipped tool latency — honestly absent). FOUND +
  FIXED: a snapshot-stale pawn row could POISON the tic estimate and its own
  zone-churn replays (in-band BEHIND) kept resetting the heal streak — health now
  requires in-band-AT-OR-AHEAD (ticclock + a regression test).
- **P4, cancel**: all three paths through the REAL strip circles in one log —
  `pending entry removed`; `executing entry; its completion will NO-OP` followed by
  `intent completion NO-OP why="cancelled by CANCEL_INTENT"` AT the fire tic (the
  tree stands); `walk; queue cleared, glide finishes`. The executing cancel landed at
  ring 0.27; the mirror emptied after each.
- **POST-DELIVERY (user report, same day)**: a BARE `Move To` executed through the
  seed path without touching the queue — only COMPOSED walks registered, so a plainly
  walking pawn showed an empty strip. Fixed: any executed move-effect order now
  registers as `Running::Move` (fresh entry id, walking phase, fanned) and the
  standing arrival poll completes it. Drilled: the walk circle appeared within one
  fan, persisted the whole 18 s glide, cleared on arrival.
- **P5**: lumberjack I7 bannered superseded; the memory + index rows record delivery.
  The bounce drill OBSERVED: the strip held its stale snapshot through the worker
  bounce, a fresh order's fan replaced it whole (entry ids restarting — the mint is
  worker memory), and the chop completed with logs yielded. F1's stated loss (the
  pre-bounce parked chop) dropped exactly as documented.
