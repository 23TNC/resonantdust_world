# server-chords — the server states where the pawn will be; the client interpolates

_User (2026-08-10): "The client MUST ALWAYS display what the server thinks is true... If the client
fails to do its job, that's fine — the pawn just becomes idle 80 tiles short, which is the correct
state for that pawn. What we have now is the client trying to guess what the server is going to
say, which causes us to fail horribly."_

Successor to [shared-simulation](../2026-08-09-shared-simulation/README.md), which put the walk in
`shared/` and both hosts on one model — and then proved, expensively, that *one rule on both sides*
is not enough.

## Why the predecessor's answer was not the answer

That stream's thesis was: the walk is written twice, so write it once and have everyone call it. It
delivered that. It also produced the evidence against it:

- `move_eval` was built to make the two walks agree, and an adversarial audit then found **five
  more divergences in code written after it existed** — `walkGreedy` stepping Chebyshev where the
  worker steps Euclidean, a frozen `ticsPerTile`, a skipped clamp, a same-tic tiebreak.
- On a healthy world with the pace **provably correct on both sides**, the client's belief still
  sat a median 1.25–2.13 tiles from truth at every anchor, and 12–15% of corrections exceeded the
  render-chase's give-up distance.

A shared rule leaves two *extrapolators*. Agreement is a property that must be re-earned by every
author on every edit, and it silently decays. **Two stated endpoints and a lerp cannot drift** —
there is nothing to agree about.

The second half matters as much, and is the user's: it **bounds the failure**. A client that falls
behind skips forward along a line the server also believes in, instead of teleporting from a route
only it computed. Being wrong stops being unbounded.

## The design

The server answers a move REQUEST with **chords** — straight segments crossing no impathable tile,
each carrying a source position+tic and a destination position+tic. The client picks the chord
bracketing `now` and lerps. It never pathfinds, never derives a pace, never predicts.

- **`MOVE_CHORDS` (id 20)** — the worker-only fan carrying the stamped route. Variable-arity like
  `QUEUE_STATE`, so `count` must be operand index 2 (the reader reads it there unconditionally).
- **`CANCEL` (id 21)** — the formal interrupt this system has never had, and the thing that makes
  the rest safe. It bumps the trip serial (killing queued hops via the predicate that already
  exists), resolves the pawn's position **from the tic**, PLACEs it, and fans an empty queue. A new
  player order issues it first. It cannot reuse `CANCEL_INTENT` (14), whose signature is
  `[Imm, Imm]` — the pawn is not a write target there, so it neither groups nor serialises.
- **Queue depth** doubles to 10; the client displays N/2+1 and re-requests below that.
- **A tile change issues interrupts** — the worker knows which live paths cross the tile.
- **Chase is deferred.** Intercept chords are their own problem and want a stable base.

## The stance

**Prove the server can predict its own arrival tic before deleting anything.** The whole design
rests on a stated destination tic being true. [P1](todo.md) fans chords *alongside* the existing
per-hop chain, changes no motion, and measures `|stated dest tic − observed arrival tic|` over 50
trips. If the server cannot hit ±2 tics, the design fails there, cheaply, with the client
untouched. Deletions come last — they are the irreversible half and they are worthless until that
number holds.

**Measure the three unknowns before committing to them** ([P2](todo.md)), because each has a
plausible value that makes this worse than what we have:

| unknown | why it could bite | the measurement |
|---|---|---|
| chord count on real terrain | string-pulling breaks at every corner; if trips routinely need >10 the re-request stutter is the normal case | `find_chords` over 1000 random pairs × 3 distance bands; report the **fraction needing >10** |
| player-order latency | cancel-first adds a barrier round trip — CANCEL ≥ master+3, resolve, compute, fan ≈ 6–8 tics (~1.1 s) vs ~4 today | wall ms from command to first rendered displacement, p50/p90, n=50, before/after |
| worker load | one route per order is cheaper than per-hop, but the supercover index and per-`SET` interrupt scan are new — and `BUILD_WALL` expands a whole perimeter, so one wall could re-plan every mover in a pass | mover-perf harness at 24 movers with a wall-build burst; compose-lag series vs baseline |

## Watch — invariants that depend on the thing being removed

The per-hop `PROMOTE` cadence is load-bearing in ways the design does not mention:

- **The client's tic estimate is fed by those writes.** `TicEstimate` anchors off state rows and
  promoted events; a moving pawn writing every 32 tics is what keeps it fresh. Endpoint-only writes
  cut the anchor rate by the chord length — and F9 makes that estimate the **single input to every
  interpolation**. Starving the clock to fix the walk would be a poor trade. Likely answer: keep
  stride-cadence writes as anchors *underneath* the chords.
- **`active_dest`/`position_at` are the WORKER's answer too** — every adjacency and range check on
  a moving pawn resolves through them. Deleting the extrapolator without re-pointing the worker at
  the chord makes moving pawns read as their stale row.
- **Arrival is polled tile-vs-dest.** A truncated queue never reaches `dest`, so a parked
  interaction never advances — a hang, not a slowdown.
- **The trip serial is 6 bits.** Cancel-per-order makes churn the normal case, and 64 values alias.
- **Zone framing** happens at queue time; a chord endpoint 200 tics out can cross zones.

## Exit

A pawn's motion is entirely described by chords the server stated. No client pathfinds, derives a
pace, or extrapolates. A player order interrupts within a tic or two via `CANCEL`. A wall dropped
across a route stops the pawn where it stands rather than walking it through. And the measured
reseed error — the number this whole line of work has been chasing — is a rounding artifact,
because there is nothing left to reseed.
