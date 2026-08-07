# Issues — intent-queue-ui (anticipated inventory)

## I1 — the fan must come from every mutation site or the display lies {#i1}

The worker mutates the queue at six sites (fresh-replace, compose, schedule,
arrival-advance, completion-advance/no-op, error-drop) — and cancel adds two more. A
missed site shows a ghost circle until the next mutation. Sweep them behind ONE
`fan_queue(pawn)` helper so a new site can't forget.

## I2 — schema change = SIX content consumers rebuild {#i2}

The `queue` sub-table changes `InteractionParams`: worker, master, orchestrator, npc,
EDGE (its corpus hot-reload refuses unknown fields — seen live in logs-drop), and the
browser wasm. The stale-binary failure is SILENT for some of them (master skips the
registry seed with a warn; the browser serves a CACHED wasm — logs-drop's find: 304s
kept a stale binary running with no error). After the rebuild: cache-bust the wasm
fetch and verify the pkg mtime + a marker string.

## I3 — progress needs the START tic on the wire {#i3}

The completion knows its fire_tic; the RING needs `(started, fire)`. The schedule site
must stamp both into the fanned entry — deriving start client-side from arrival guesses
wrong under lag. move_to entries fan without timing (progress "none" needs none).

## I4 — the cancelled completion must log as CANCELLED, not stale {#i4}

The receipt check that advances the queue is where the cancelled-set is consumed; the
completion's reject must say `cancelled` (a distinct why) or drills can't tell a cancel
from a wandered-off no-op. The cancelled-set is ephemeral — a worker bounce forgets it
and a still-valid completion executes; state it in ACTIONS.md rather than pretending
durability.

## I5 — the hidden-tab freeze makes the ring lie in drills {#i5}

The ring animates on rAF; a hidden tab freezes it while tics advance (lumberjack I10's
family). The percentage must be computed from the CURRENT tic estimate at each frame —
never incremented — so a foregrounded tab snaps to truth. Drills read the computed
percentage via probe, not by watching the animation.

## I6 — the strip competes with the panel's existing left edge {#i6}

The details panel already hosts condition cards and text at fixed offsets; the shift
must move the CONTENT container, not each element, and the strip must not widen the
panel's hit area over the world (clicks on empty strip space should still reach the
canvas… or not — decide in the item and record it).

## I7 — carried successors {#i7}

Queue REORDERING (drag), multi-pawn queue overviews, and cancel feedback in the UI
(today a refused cancel is a worker log line only) are recorded, not built.
