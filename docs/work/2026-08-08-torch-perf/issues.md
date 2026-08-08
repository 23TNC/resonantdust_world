# Issues — torch perf (anticipated; logged before they bite)

## I1 — other movers contaminate the measurement {#i1}

Wolves and bunnies are movers (warm re-bakes) and the wolves' prey drive
generates chase traffic. Both containers STOP for the whole measurement and
restart in the truth pass. The four placed world torches stay — cold lights,
constant across rows, named in the table so nobody subtracts them twice.

## I2 — hot lights re-bake whether or not the pawn is mid-trip {#i2}

`hot` is per-frame re-bake; an idle torch pawn still costs. The wander loop
must keep pawns ACTUALLY moving (a new trip on every arrival, short think
pause) so the rows measure the moving case the user named — and the soak
reads are taken while trips are live, not during a lull.

## I3 — WSL2 timing variance {#i3}

The clock runs loose here (movement-hardening found CLOCK_MONOTONIC at
0.90×). Each row records SEVERAL `__lightcost`/`__framecost` reads spread over
the soak and reports the spread, not one sample. A row whose min/max straddle
a cliff gets re-run once before being believed.

## I4 — this measures COUNT, not the spill {#i4}

One light on a one-part pawn: the spill machinery (trait-lights F8) is not in
play, deliberately — the variable is the number of moving emitters. A
spill-density test (few pawns, many lights each) is a different experiment; if
the count curve looks super-linear, that successor gets designed against
these numbers.

## I5 — pawns that wander out of the window stop being measured {#i5}

An unstreamed/unrendered mover attaches no light. The wander RADIUS must keep
the whole population inside the zoom-1 viewport around the home (roughly ±20
tiles), and the row capture doubles as the check — count the glows.

## I6 — server backpressure can masquerade as client cost {#i6}

24 pawns re-tripping continuously = 24 intent/chain streams through the
worker. If compose lags, movement stutters, and a stuttering mover re-bakes
LESS (fewer position changes) — flattening the client curve for the wrong
reason. Each row records the worker's composed-events line and any pacing
warnings beside the client numbers.

## I7 — the reset must not resurrect the trait-lights cast {#i7}

The pawn-module redeploy wipes the cast (humans, wolves, the drill history).
The npc containers hold adopt-first windows and re-mint their OWN kinds when
restarted — that is exactly the contamination I1 stops. Nothing else
resurrects pawns (lumberjack's seed-guard law: restarts do not re-seed
occupied zones).

## I8 — a glError 1282 lives in the debug read path {#i8}

The trait-lights P4 drill saw `glError: 1282` in the `__lightcost` payload
while rendering correctly. Before trusting the rows, check whether the error
is the debug hook's own read (benign, note it) or a real pipeline error
(finding). It predates this stream either way — recorded so the perf table
doesn't silently inherit an unexplained flag.
