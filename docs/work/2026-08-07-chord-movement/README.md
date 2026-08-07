# Chord movement — shortest paths, tiles/tic, and the anchor that stops backtracking

**What** (user, 2026-08-07): "Instead of only moving in cardinal directions we will
move using shortest path. I believe we can do this by understanding the pawns
footprint and path. I suspect we should be able to determine the minimum number of
chords a pawn must walk to reach its destination. This likely flips our math from
tics/tile to tiles/tic and makes working out which tile a pawn is on speculatively
more difficult. We will also omit fanning out the starting tile. This will prevent
our pawns from back tracking to the nearest tile when movement is interrupted. I'd
like your honest opinion if this is feasible given our current architecture as that
speculation will likely make or break this plan."

## The feasibility verdict (asked for, given honestly)

**Feasible — and the architecture is unusually well-positioned. The speculation half
is the LOW-risk half.** Four load-bearing facts, verified in code before this plan
was written:

1. **The wire already has room, backwards-compatibly.** A `position_reference`'s low
   byte is `layer_reference` — pawns always write 0, and `position_to_tile` (every
   existing decoder) IGNORES it. Repurposing it as `sx:4 | sy:4` SUBTILE nibbles
   (1/16-tile authoritative resolution) needs **no schema change and breaks no
   decoder** — an old reader simply floors to the tile. This is what kills
   backtracking: a mid-trip re-order resumes from the stored sixteenth-of-a-tile,
   not the last whole tile ([F1](forks.md#f1)).
2. **Per-hop state never fans**, so redefining a hop (one CHORD instead of one tile)
   is invisible to the protocol. The chain's supersession/serial law carries over
   unchanged; hop event count actually DROPS to the chord count — exactly the
   user's "minimum number of chords" ([F2](forks.md#f2)).
3. **The client is already continuous.** The render track is floats with a chase;
   the spec becoming "distance along a polyline = elapsed × tiles/tic" is SIMPLER
   than today's waypoint hop-walk. The learned-rate estimator, the chase, and the
   promote corrections all survive verbatim.
4. **Observer agreement stays provable.** The polyline's corner points are INTEGER
   grid coordinates (doubled corners — [F5](forks.md#f5)), so the ROUTE is
   integer-exact across worker/wasm/npc like today's A*; only PROGRESS along it is
   fractional, and progress divergence is bounded by the tic estimate and corrected
   at promotes — the same error budget movement has now.

**The two genuinely hard parts are worker-side, not speculation** — named so they
get built deliberately:

- **Mid-chord resolve-on-touch** ([F3](forks.md#f3)): with per-chord writes, any
  event touching an in-flight pawn (a superseding order, an interaction, a needs
  re-stamp) must first RESOLVE its interpolated position — derivable from the stored
  row (position + write tic) plus the deterministic path recompute, no new state.
  The "resolve-on-touch is free" law already exists in spirit; this makes it real.
- **Tile-flooring consistency** ([I2](issues.md#i2)): everything that asks "what
  tile is the pawn on" floors a fractional position. The rule that keeps it safe:
  VALIDATION floors only worker-stored positions (as today — the client's menu
  distance is advisory and the worker re-validates), so a boundary rounding
  disagreement can annoy but never desync.

**The no-start-fan is feasible with one condition**: the held every-N re-anchor knob
(ACTIONS.md has reserved it since first-pawns) must finally turn ON
([F4](forks.md#f4)). Without the start anchor, a fresh subscriber or a drifted
estimate has NO correction until landing; a subtile-accurate mid-trip promote every
~32 tics is the safety net, and — because it carries subtile position — it corrects
without ever causing the backtrack visual the start fan caused.

Sizing honestly: this is 1–2 streams of work, with the worker's resolve law the
long pole. Recommend shipping 1×1 footprints with a footprint-radius-aware clearance
signature ([F7](forks.md#f7)) so the multi-tile future slots in without a rewrite.

## The stance

- Route = octile A* (Euclidean costs) + string-pulling to the minimal chord
  polyline, in the SAME `path_eval` every observer already shares ([F5](forks.md#f5)).
- Speed inverts at EVAL: content keeps authoring `walks` in tics/tile; consumers
  derive tiles/tic ([F6](forks.md#f6)). A chord's duration = ceil(len × tics_per_tile).
- Authoritative subtile positions at chord ends; mid-chord = resolve-on-touch.
- The seed fans the INTENT (dest + tic), not the start position; the every-N
  re-anchor corrects; landing still promotes ([F4](forks.md#f4)).

Authoritative docs touched: `docs/ACTIONS.md` §Movement (the chord law),
`docs/VARIABLES.md` (the pawn subtile nibbles).
