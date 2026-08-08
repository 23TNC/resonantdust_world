# Issues — mover perf (anticipated; logged before they bite)

## I1 — contamination discipline, unchanged {#i1}

Wolves + bunnies STOPPED for every row (their movers and hunts are load);
debug movers are the only pawns. The four placed world torches stay — cold
lights are client-side only and this stream's primary metric is the worker.

## I2 — stable-behind vs growing is THE question {#i2}

Torch-perf's single 24-tic sample cannot tell a fixed pipeline offset from a
compounding deficit. Each row's lag series must show its SHAPE over the whole
soak — flat line, flat-with-offset, or a slope. A slope on the 24 row is the
expected headline; a slope on the 8 row would be a much worse finding.
Ten minutes minimum on the 24 row before any verdict.

## I3 — zone concentration is deliberate worst-case {#i3}

The ±12 pen packs every mover into a handful of zones and therefore into the
same work-groups — the serial worker's worst case, and exactly the scenario
that showed the lag. Record the zone spread (macro refs of the population)
beside the rows so a future multi-worker test can reproduce the same
concentration.

## I4 — population lands N±1 (the mint gate's residual) {#i4}

Torch-perf I9's fix leaves a ±1 race when a pre-existing pawn adopts after
the window. Rows RECORD the actual population (SQL count), and the table
reports it — 8/16/24 are targets, not promises.

## I5 — WSL2 timing variance {#i5}

The host clock runs loose (movement-hardening: 0.90×). Lag is measured in
TICS from the worker's own lines (immune to wall-clock drift); wall-clock
enters only through the master's pacing report, which rides along per row.

## I6 — the client is the secondary instrument, not the subject {#i6}

`__framecost` at zoom 1, tab foreground, per row — expected flat (torch-perf
proved the client ceiling is the 8-slot cap, and these movers carry no
lights at all). A NON-flat client row here would mean mover prim sync itself
scales badly — a separate finding, named if seen.

## I7 — this stream buys the ruler, not the fix {#i7}

The user's framing: catch the workers falling behind → a stable platform to
debug and improve off of. Improvements (a second worker — the orchestrator
split exists and has never been exercised; cheaper hop round-trips; batched
promotes) are SUCCESSORS designed against this stream's numbers. Nothing
here touches the worker's code.

## I8 — the debug brain rename must not strand the torch rows {#i8}

`NPC_TORCHES`/the `torches` arm die in the conversion (F2). torch-perf's
completed.md documents the OLD command — correct as history, stale as
instruction. The new harness line (`NPC_BRAIN=debug NPC_KIND=… NPC_COUNT=…`)
lands in THIS stream's completed.md, and re-running the lit twin uses it.
