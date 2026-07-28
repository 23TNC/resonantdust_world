# Issues — pawn-movement

## I1 · MoverLayer drops live intents on two paths (the snap-tween-snap's likely core)

Inherited defect, found at plan time (`client/webgl/src/game/world/MoverLayer.ts::onMoveIntent`):
an intent for a mover not yet in the map is dropped (`if (!m) return`) — the code comment
claims the seed `State` "reseeds everything" but the seed only PLACES the pawn; the dropped
intent never arms a spec, so the trip renders as two snaps. Likewise an intent arriving before
the tic clock anchors (`d === null`) is dropped. A later stale replay (I2 below) can then arm
a WRONG spec — old dest, old tic — producing "snap somewhere, tween, snap somewhere else".
Fixed by P3's pending-intent buffer.

## I2 · `event` table retention (inherited: first-pawns I2)

Still no retention sweep — every (re-)subscribe replays all settled intents; client guards are
the only defense. P3 builds the server sweep on the master's gc cadence. Tracked here so THIS
stream closes it; first-pawns stays done.

## I3 · Intent delivery flakiness (inherited: first-pawns I3)

Measured absent/doubled per trip during first-pawns; the edge event-sub `on_applied` replay
added during sim-self-heal was a mitigation, never re-measured. P3 re-measures under a ≥10-trip
soak after retention lands; if still flaky, root-cause at the edge's per-zone sub callback
semantics. Findings land here.
