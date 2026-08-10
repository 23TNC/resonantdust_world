# Issues — server-chords

_Problems hit, candidate solutions, which we chose and why. Chronological append._

## I1 — the design is underspecified in eight places
**2026-08-10. Open — each needs a decision before the phase that touches it.**

Found by surveying the source against the design. Listed so an implementer does not guess:

1. **Which queue is 10?** `INTENT_CAP` caps parked INTERACTION orders; chords are a different
   queue (today N chords = N queued `MOVE_STEP`s, unbounded). Separate `CHORD_CAP`, or the
   truncation semantics collapse. → [P2](todo.md).
2. **Truncation resume conflicts with cancel-first.** "A new order issues CANCEL first" plus "the
   client re-requests when it drains" gives every long walk a resolve-and-place stutter at each
   chord-queue boundary. A re-request should be a CONTINUE — the server keeps `Running::Move`'s
   goal and extends — not a fresh order. → [P3](todo.md).
3. **Is the stated tic BINDING on the server?** If a hop writes late, the client is ahead of truth
   — the exact failure this design forbids. Either the server writes the literal stated tic, or the
   client re-lerps at every anchor. → [P3](todo.md).
4. **Mid-trip pace change.** `ground_speed` can narrow from a need band. Does that interrupt and
   re-issue, or does the stamped schedule stand? → [P3](todo.md).
5. **Sub-chord anchoring.** `REANCHOR_TICS = 32` exists to bound observer drift. Endpoint-only
   writes on a 40-tile chord are 240 tics with no anchor — and the client's tic estimate feeds on
   those writes. → [P3](todo.md), and the sharpest of the eight.
6. **Chord source is a tile CENTRE**; a mid-walk pawn is at a subtile point. → [P1](todo.md).
7. **Durability.** `intent_queues` is worker-ephemeral. If `Running::Move` holds the chord list, a
   worker bounce loses cancel authority while queued hops keep walking. → [P3](todo.md).
8. **Frame ordering on CANCEL** — the position write and the empty `QUEUE_STATE` are two frames;
   which one tells the client to drop its chords? → [P4](todo.md).

## I2 — the client's tic estimate is fed by the writes this design removes
**2026-08-10. Open — the invariant most likely to bite, and nobody had named it.**

`TicEstimate` anchors off state rows and promoted events. A moving pawn writing every
`REANCHOR_TICS` is what keeps a client's clock fresh, and this design makes that clock the
**single input to every interpolation** — a chord is only meaningful relative to "what tic is it".

Endpoint-only writes cut the anchor rate by the chord length. Trading a starved clock for a fixed
walk would be a poor bargain, and the symptom would look exactly like the drift we are trying to
remove.

**Likely answer:** keep stride-cadence anchor writes UNDERNEATH the chords — they cost what they
cost today and they are what the estimate eats. [P3](todo.md) carries it with an explicit
acceptance: anchor age p90 unchanged against the P2 baseline.


## I3 — `SET_NEED`'s arity: docs say 2, code says 3
**2026-08-10. Open — found while testing the new framing; not fixed in passing.**

`docs/ACTIONS.md` states "**Arity 3→2 with the stat-model reshape** (the def-ref + f32-bits form is
gone)". `shared/codec::action`'s signature table gives it three operands, and the framer agrees:
a two-operand `SET_NEED` yields `Truncated { action: 10, want: 3, got: 2 }`.

Docs outrank code, so the code is nominally the bug — but a verb's arity is a WIRE contract and
every existing packer writes whatever it writes today. Changing it is a codec change plus an
event-shard redeploy plus an audit of every producer, which is not a passing fix.

**Chosen path:** record it, leave both as they are, and settle it when P0's redeploy is happening
anyway. Whoever takes it must check which form the live producers actually emit before believing
either document.

It cost me several minutes of reading a framing failure as a bug in my own change, which is the
argument for closing it rather than leaving the two authorities disagreeing.
