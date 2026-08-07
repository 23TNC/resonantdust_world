# Forks — intent-queue-ui (plan-time decisions; each is mine unless the user vetoes)

## F1 — fan the queue as promoted events; authority stays ephemeral {#f1}

**Chosen**: a worker-only `QUEUE_STATE` event, promoted so it fans to the pawn's zone
subscribers, emitted at EVERY queue mutation with the full entry list (small: cap 5).
Lumberjack F1 is untouched — the worker's memory is still the authority and a bounce
still drops the tail; the fan is display truth only, self-correcting on the next
mutation.
**Rejected**: a pawn-shard `intents` row (the authority the user explicitly declined in
lumberjack F1); client-side reconstruction from existing order/completion events (mirrors
worker logic — the drift class the one-composer rule exists to prevent).

## F2 — one `queue = { … }` sub-table on the interaction {#f2}

**Chosen**: `queue = { hover, size, background, progress, progress_color, progress_fill,
cancelable }` — one optional table, one `QueueVisual` struct on `InteractionParams` with
defaults (neutral grey circle, size 1.0, progress "none", not cancelable). `hover`
defaults to the interaction's `label`. Authored: drink + cut_down `progress = "cw"`,
`progress_fill = true`, green, `cancelable = true`; move_to `progress = "none"`,
`size = 0.6`.
**Rejected**: flat `queue_*` fields on the interaction (seven new top-level keys crowd
the schema); a separate visuals.toml (the interaction IS the unit — one block, one
place).

## F3 — cancel semantics by phase; a new `CANCEL_INTENT` verb {#f3}

**Chosen**: `CANCEL_INTENT pawn order_ref`, CLIENT-open (like the other order verbs —
no ownership model). Worker resolution: PENDING entry → removed, queue refanned;
EXECUTING timed entry → only if `cancelable`: `running` cleared and the order_ref
remembered in an ephemeral cancelled-set, so the already-queued completion fires into a
logged `cancelled` NO-OP (no new event machinery — the completion still arrives, the
receipt check eats it); EXECUTING walk (a composed move_to) → the queue clears and the
chain finishes its current glide (no stop-seed machinery; the parked act dies with the
queue — the lumberjack replace law's shape). A bounce forgets the cancelled-set; a
still-valid completion then executes — the F1 "whatever happens to that queue happens"
posture, recorded loudly.
**Rejected**: cancel-by-supersession (issuing a self-move to re-stamp the serial — a
hidden movement side-effect for a UI click); making the completion event itself
retractable (events are append-only; retraction is exactly what re-validation replaces).

## F4 — cancel names the ORDER's event_reference, not a queue index {#f4}

The fan stamps each entry with the order's event_reference (unique per order — the same
word the trip-serial law leans on). A click sends that reference; an index would race
advancement (click slot 2 while slot 1 completes → cancel the wrong intent). A cancel
whose reference no longer sits in the queue is a logged no-op.

## F5 — the ring renders as an SVG stroke, bottom-up stack, active at the bottom {#f5}

One vertical flex column pinned to the panel's left edge, existing panel content shifted
right by the strip's width; entries render newest-at-top so the BOTTOM circle is the
active event (the user's reading order). The progress outline = an SVG circle stroke
(dashoffset percentage; `cw` natural, `ccw` mirrored, `progress_fill` chooses whether
the arc grows or shrinks); hover = a positioned tooltip div fed by the TOML `hover`
text. No canvas involvement — the panel is DOM already.
