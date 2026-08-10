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


## I4 — the map is one zone: P2's chord histogram cannot be measured yet
**2026-08-10. Open — blocks [P2](todo.md)'s first item at the long bands.**

The chord survey (`headless <name> chords <x> <y>`) reports:

```
generated terrain in reach  known=256  impathable="7 (2.73%)"  extent="112..127 x 64..79"
chords band="3-12 tiles"   n=1000 p50=1 p90=2 p99=3 max=4  over_cap="0 (0.00%)"
chords band="12-40 tiles"  n=1000 p50=1 p90=2 p99=3 max=4  over_cap="0 (0.00%)"
band produced no routes    lo=40 hi=96 tried=200000
```

**256 cells is exactly one 16×16 zone.** The world generates on demand and nothing has walked far
enough to make more. Ten `cold tiles` fans arrive at an anchor with `cold: 8`, so zones are being
SUBSCRIBED without being GENERATED — an empty zone streams all-zero cells, which `tile_kind_at`
reports as unseen.

**The first version of this survey did not notice, and confidently reported `p50 = 1, 0% over
cap` across all three bands from a square that was 99.3% unknown.** `pathable` treats an unknown
cell as OPEN, so every pair string-pulled to one straight chord: a precise number about nothing,
and exactly the false green the tick audit exists to catch. The harness now draws pairs only from
cells the view actually holds, prints the extent, and warns below 2000 known cells.

**What the partial result says.** Inside the one generated zone, at 2.73% impathable, trips need
1 chord at p50 and 4 at worst — nowhere near a cap of 10. That is a genuine measurement of THIS
terrain and it points the right way, but it cannot answer the band the design actually worries
about, and open grass is the easy case: a corridor or a wall maze is where chord counts explode.

**Chosen path:** carry it into P2 as work rather than a blocker. The survey needs terrain, so P2's
first item grows a step — sweep an anchor across a grid to force generation, verify `known` grows
past the single zone, THEN histogram. If anchoring turns out not to generate beyond the active
radius, the fallback is to build a wall maze in a known zone and measure the hard case directly,
which is the number that matters anyway.


## I5 — core's "what tic is it" ignores wall time, so every position holds between events
**2026-08-10. Open — found live by the user (movers advance-then-pause at 60fps). Blocks the
smoothness half of [P5](todo.md).**

```rust
// client/core/src/client_world.rs
pub fn now_tic_at(&self, _now_ms: f64) -> Option<u16> {
    self.tic
}
```

The wall clock is **taken and discarded** — the leading underscore is the tell that this was meant
to extrapolate and never did. `self.tic` is written only here:

```rust
// client/core/src/web.rs, in emit() — THE fold
w.set_tic(self.tics.estimate_at(now).map(|t| t.rem_euclid(65536.0) as u16));
```

Two defects, stacked:

1. **Core's "now" advances on EVENT ARRIVAL, not on wall time.** Between inbound frames
   `pawn_point` returns a bitwise-identical position however often it is asked. Measured live: six
   samples over 107 ms returned `120.30518423120411, 79.4302061055012` every time, same tic.
2. **The fraction is thrown away.** `TicEstimate::estimate_at` returns `f64` tics — a smooth
   function of wall time, which is exactly what an interpolator needs — and the fold truncates it
   to `u16`. Even a perfectly fresh integer tic only changes ~5.4 times a second.

**webgl is not at fault.** `MoverLayer.tick` asks core every frame
(`MoverLayer.ts:585 — const core = this.client.pawnPoint(key)`) and chases the answer. It is
chasing a target that only moves when a packet lands; the chase closes the gap, then holds. That is
the "move a bit, pause a bit" the user sees, and it is mid-tile because event arrival has nothing
to do with tile boundaries. It reads as a stutter rather than a freeze because a compose pass fans
many rows at once, so the tic jumps in clumps.

**This is NOT fixed by chords.** A lerp between two stated endpoints is only as smooth as the
answer to "what tic is it now". [P5](todo.md) inherits this clock and will stutter identically
unless it is fixed — which makes it a prerequisite of P5, not a follow-up.

**The fix** is a fractional now, evaluated on demand rather than stamped:
- `now_tic_at` uses its `now_ms` against the estimator's anchor + learned rate.
- The interpolators take `f64` tics. `MoverTrack::point_at(entity, now: u16)` and
  `move_eval::advance_along(.., now: u16, ..)` are integer-tic today, so the quantisation survives
  even a fresh answer.
- Keep `anchored_tic()` integer and event-stamped — a FOLD legitimately wants the receipt tic. The
  two answers are different questions and currently share one field.
