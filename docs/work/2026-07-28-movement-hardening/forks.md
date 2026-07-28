# Forks — movement-hardening

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · Chain identity = a 6-bit trip-serial in the pawn's `data`, checked by a new hop verb

The supersession mechanism from pawn-movement I7's sketch, ratified. The seed stamps
`data = facing<<6 | (seed event_tic & 0x3F)`; continuations carry the serial as a third
operand and die on mismatch. Why this shape:

- **The pawn row is the only shared state a hop can read** — chains are otherwise just
  future-queued events with no linkage. The `data` u8 has exactly 6 free bits (facing owns
  the top 2; the client's `facing()` is `data >> 6` and never sees them — verified in
  `client/core/src/world.rs`).
- **Serial collisions don't matter**: a stale chain dies at its FIRST post-supersession hop,
  so a colliding serial would need the old chain to hop exactly when the ring re-aligns
  (64-tic wrap) with no intervening hop — impossible at `tics_per_tile` spacing.
- Rejected: a `cancel_moves(obj)` reducer scanning the queue (the event shard would need an
  obj→events index it doesn't have; racy against in-flight claims; heavy for the common
  case). Rejected: npc-only mitigation (longer slack — shrinks the window, doesn't close the
  class; any future driver re-inherits it). Rejected: dest-equality convergence (two
  same-dest chains still double-step — 2 tiles/slot wall speed — and different-dest fights
  stay possible from any two drivers).

## F2 · `MOVE_STEP` is WORKER-ONLY; the edge grows a client-verb allowlist

Continuations become their own verb (value 8, arity 3) rather than overloading `MOVE_TO`
with an optional operand (the RPN stream has fixed arity per action — a variable-arity verb
breaks framing). A client must never issue one (it could stomp serials and steer pawns one
tile at a time past validation), so the edge's queue door gains an explicit allowlist — the
first authorization seam, deliberately tiny: a verb-set check, not an ownership model
(ownership stays an open design question per first-pawns). Rejected: trusting clients not to
(the edge exists to validate); rejected: building ownership now (the user explicitly hasn't
decided pawn/player ownership yet — preserve that fork, don't half-build it).

## F3 · StateGone suppression lives at the EDGE

The edge uniquely holds all of a client's zone subs on one connection, so entity→zone
tracking there is one small map and every client (webgl, npc, future drivers) is fixed at
once. The one-beat hold covers delete-before-insert ordering; a real removal still relays
after the hold. Clients KEEP their defenses (wolves adoption-keep, MoverLayer re-add) —
defense-in-depth, now labeled defense rather than workaround. Rejected: client-side-only
(every client re-implements it; the npc's version already deadlocked once); rejected:
server-side zone-handoff protocol in the shards (heavy, and the subscription model is the
edge's seam, not the shards').

## F4 · The master holds the PERIOD; no burst catch-up — and diagnosis precedes the fix

A tic is a wall-time promise to speculating clients (the whole estimator exists because the
promise was broken). Bursting `bump_tic` to catch up would briefly run the world fast —
worse than running uniformly slow. So the fix targets per-iteration overhead (measured
first: interval overshoot vs SDK call latency vs the dedup-skip), holding cadence accuracy.
The learned-rate estimator STAYS regardless — it is the defense against any future drift
(load spikes, other hosts); F6 of pawn-movement is not un-done by fixing the clock.

## F5 · The persisted rate is a HINT, not state

localStorage seeds the estimator's starting rate (clamped to the same band the estimator
enforces); the live stream immediately refines it. A wrong or stale hint costs at most the
warmup it was meant to save; it can never poison the estimate (the band + windowed
re-anchor bound it). Rejected: persisting the full anchor (a (tic, wall) pair goes stale the
moment the server restarts or the realm's tic resets — the RATE is the only durable part).
