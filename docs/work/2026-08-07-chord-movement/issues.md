# Issues — chord movement (anticipated; logged before they bite)

## I1 — float determinism across worker/wasm/npc {#i1}

The ROUTE is integer (F5) so observers cannot disagree on it. The residual float
surface: chord lengths (IEEE sqrt — correctly rounded, identical everywhere) and
per-observer progress (never part of agreement — corrected at promotes). Keep it
that way: any future cost/heuristic must stay integer or provably-rounded, and the
determinism unit test (byte-identical polylines) guards the door.

## I2 — tile-flooring at boundaries {#i2}

"Which tile is the pawn on" = floor of a fractional position, and a pawn WALKING A
CHORD spends real time exactly on tile edges. The safety rule: VALIDATION (adjacency,
completion re-validation, carrier resolution) floors only WORKER-STORED/RESOLVED
positions; client-side flooring is display/menu-advisory only. Define edge ownership
once in the codec (half-open: a subtile of 0 belongs to the tile) and test it.

## I3 — late joiners without a start fan {#i3}

A client that subscribes mid-trip has only the LAST promote (up to a re-anchor
interval stale) plus the intent. It speculates from stale data until the next
re-anchor lands (F4). Bound it: REANCHOR_TICS starts at 32 (~5 s) and the recorded
spec-error telemetry (first-pawns F8) decides if it tightens. Do not chase perfect
late-join fidelity — landings and resolve-on-touch promotes already correct.

## I4 — the intent strip's ring and the queue's walk estimates {#i4}

The ring computes from (started, fire) tics the WORKER stamps — chord math changes
how fire tics are computed (ceil(len × tics_per_tile) summed over chords), not the
display fan. Verify the walk leg's registered dest/fire agree with the chord
schedule or rings will finish early/late.

## I5 — the interrupted-trip drill is the acceptance {#i5}

The whole point: order a trip, interrupt it mid-CHORD with a new order, and the pawn
must continue from its mid-chord point — no backtrack server-side (resolve-on-touch)
and no snap client-side (no start fan). Drill BOTH halves explicitly, plus the
diagonal shortcut (a chord trip measurably shorter than the old hop path).

## I6 — every position consumer meets subtile {#i6}

Writers: worker chord ends + resolves. Readers that must LEARN subtile: the client's
authX/authY (fractional now), the npc's `self.at`, adjacency floors. Readers that may
IGNORE it (floor for free, verified): edge fan routing (macro/tile bytes), zone
subscription keys, the renderer's zone math. Sweep with a checklist, not vibes.

## I7 — zone borders mid-chord {#i7}

A chord may cross a zone boundary between writes; the pawn's row zone updates only
at chord ends/resolves. Subscriptions and fan routing key on the STORED row's zone —
identical to today's behavior between hops, just longer windows. If a chord spans
beyond the subscribed ring, the resolve at its end re-zones; cap chord length (split
long chords at ~8 tiles) so the window stays bounded.

## I8 — the pie menu's distance relax and walk-then-act {#i8}

The walk-then-act composer picks a pathable cell in the carrier's range and the
completion re-validates cheb ≤ 1 — both keep working on floored tiles (I2), but the
walk DEST should become the chord-nearest point rather than tile-center where it
matters. First pass: tile centers (correct, marginally longer); note the successor.

## I9 — TicEstimate pacing vs variable hop intervals {#i9}

Hop events now land at irregular intervals (per-chord durations). The client's rate
learner anchors on fanned events; nothing in it assumes uniform spacing (verified in
the poisoned-anchor rework) — but watch the drill for estimate wobble on long chords
and lean on re-anchors (F4) if it appears.
