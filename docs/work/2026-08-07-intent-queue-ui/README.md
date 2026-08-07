# intent-queue-ui — the details panel shows the queue; clicking cancels

**What** (user, 2026-08-07): reshape the details panel — "shift all of our information
right, and display the pawn's intent queue vertically along the left. The bottom most
element of the intent queue will be the current active event… Every event will be
displayed using a circle. When we hover over the event we will get a tool tip… The
currently active event will also display progress as an outline as a percentage… The
interaction toml will define the visual elements of these displays… the text we see on
hover, the size, background color, the progress outline cw or ccw or none, the progress
color, and if it fills or empties. For the log cutting and drink interactions it will
fill, green, cw. For move to… progress set to none… smaller than standard interactions so
that it is visually distinct. New functionality: when we click on an event in the intent
queue we will attempt to cancel the interaction. The interaction toml will define if the
event is cancelable, in the case it is currently executing."

## Design stance

- **The queue FAN is presentation, not authority** ([F1](forks.md#f1)): lumberjack F1's
  ephemeral queue STANDS — the worker's memory remains the only authority, and the
  no-op re-validation law remains the safety. What this stream adds is the fan lumberjack
  I7 named: the worker emits a promoted `QUEUE_STATE` event at EVERY queue mutation
  (compose, schedule, advance, replace, complete, no-op, cancel) carrying the pawn and
  its entries. A lost fan means a stale display, never a wrong action.
- **Entries carry identity**: each fanned entry = the ORDER's event_reference + the
  interaction's definition_reference + phase (pending | walking | executing) + the
  timing pair (`started_tic`, `fire_tic`) when executing. The event_reference is what a
  cancel CLICK names ([F4](forks.md#f4)) — index-based cancel would race the queue.
- **Progress is client-derived**: percentage = elapsed/(fire − start) through the learned
  tic estimate — the same clock speculation trusts. cw/ccw/fill/empty are pure
  presentation from the TOML; `none` renders no ring (move_to).
- **The TOML owns the look** ([F2](forks.md#f2)): a `queue = { … }` sub-table on the
  interaction — `hover`, `size`, `background`, `progress` (`"cw"|"ccw"|"none"`),
  `progress_color`, `progress_fill` (fills vs empties), `cancelable`. Defaults make an
  unauthored interaction render as a standard neutral circle. Authored here: drink +
  cut_down = fill, green, cw, cancelable; move_to = progress none, size smaller.
- **Cancel is an ORDER, not a mutation** ([F3](forks.md#f3)): a new client verb
  `CANCEL_INTENT pawn order_ref`. The worker resolves it against its queue: a PENDING
  entry is removed; the EXECUTING entry cancels only if its TOML says `cancelable` — the
  scheduled completion then fires into a logged `cancelled` no-op. A composed WALK
  cancels by clearing the queue (the chain finishes its glide harmlessly — no new
  stop-machinery). Ephemeral like the queue: a worker bounce forgets a pending cancel
  and the completion executes if still valid — the F1 posture, stated not hidden.
- **The panel layout is VARIABLES.md's** ([[variables-authoritative]]): the shift-right +
  left vertical strip is recorded there before the code moves.

## What exists (audited 2026-08-07, post-lumberjack/logs-drop)

- The worker's queue: `PawnQueue { pending: VecDeque<PendingIntent>, running:
  Option<Move|Timed> }`, mutated at six sites (fresh-replace, compose, schedule,
  arrival-advance, completion-advance, drop-on-error) — each becomes a fan point.
- Events with a `macro_position` fan to subscribed clients via the edge's event channel
  (the MoveIntent path) — the QUEUE_STATE fan rides it; no new edge plumbing expected.
- The details panel is a DOM panel (`client/webgl` panels); interaction TOML loads
  through `InteractionParams` (loader) → wasm accessors; five content consumers
  (worker/master/orch/npc/EDGE + the wasm) must rebuild on schema change — the
  logs-drop I2 law.
- Progress inputs exist: the completion's `fire_tic` is chosen at schedule time and the
  client's tic estimate is proven (speculation runs on it).
