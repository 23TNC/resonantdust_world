# Forks — chord movement (plan-time decisions; each is mine unless the user vetoes)

## F8 — hops fire AT the re-anchor cadence, every hop PROMOTEs (resolved in execution) {#f8}

F2's "one event per chord" and F4's "re-anchor every 32 tics" meet an economy fact:
you cannot fan without an event, so the event cadence must be ≥ the re-anchor cadence
anyway. Merged: a hop's STRIDE = `clamp(REANCHOR_TICS / tics_per_tile, 1, 8)` tiles —
hops land every ≤32 tics, each writes its (possibly fractional) progress point, and
EVERY hop promotes. That write IS the re-anchor; separate bare-hop + anchor events
would cost strictly more. Fan ≈ one frame per 32 tics per MOVING pawn (0.19 Hz) — far
under the per-tile fan-out the cadence law forbids. Fast pawns still get fewer, longer
hops; the chord cap (I7) bounds the window.
**Rejected**: per-chord hops with separate re-anchor events (more events, same fan);
bare hops with rare promotes (late joiners drift up to a whole chord).

## F1 — the pawn's dead layer byte becomes SUBTILE nibbles {#f1}

A `position_reference` low byte is `layer_reference` (`type_id:4 | layer_id:4`) —
COLD rows address layers with it; PAWNS always write 0 and every decoder
(`position_to_tile`) ignores it. For pawn positions it becomes `sx:4 | sy:4`
(sixteenths of a tile, offset from the tile's origin): 1/16-tile authoritative
resolution, zero schema change, graceful degrade (an old reader floors). Cold rows
keep layer semantics untouched — the byte's meaning is per-shard, and the codec gets
explicit `pack_pawn_position/subtile` helpers so the two never mix.
**Rejected**: widening positions to f32 pairs (every table, every event operand, and
the codec reshuffle for resolution beyond what movement needs); keeping tile-only
authoritative positions (re-orders backtrack up to a tile — the exact complaint).

## F2 — one hop event per CHORD {#f2}

The chain keeps its shape — seed + self-queued continuations + serial supersession —
but a hop walks ONE chord: it writes the chord's END (subtile) and queues the next
hop at `tic + ceil(chord_len × tics_per_tile)`. Event count = the chord count (the
user's "minimum number of chords"), DOWN from per-tile. Stateless recompute survives:
each hop re-paths from its stored position, so world changes and supersession stay
correct with no stored route.
**Rejected**: fixed-cadence fractional stepping (more events, and the cadence is a
new magic number); storing the polyline in the chain (stale the moment the world
changes — the same rejection as pathfinding F3).

## F3 — mid-chord RESOLVE-ON-TOUCH is a law, not an accident {#f3}

Between chord writes the pawn's stored row is its chord START + the write tic. Any
event that touches the pawn — a superseding move seed, an interaction effect, a
re-stamp — must first resolve `position = start + unit(chord) × (now − write_tic) ×
tiles_per_tic` (quantized to subtile), using the DETERMINISTIC path recompute to know
the chord. No new state: the row + the corpus + the tic are the whole computation
(the lazy-eval pattern needs/stats already follow). This is what makes "no
backtracking" true SERVER-side: an interrupting order starts from where the pawn
actually is.
**Rejected**: writing position every tic (per-tic fan-out is the thing this whole
architecture refuses); accepting chord-start staleness (backtrack up to a whole
chord — worse than today).

## F4 — the seed fans NO position; the every-N re-anchor turns on {#f4}

The user's call: omit fanning the starting tile. The intent fan (dest + tic) stays —
speculation arms from the client's CURRENT belief, so an interrupted trip re-aims
from the rendered position with no snap-back. The cost is honest: a fresh subscriber
or a drifted estimate gets no correction until landing — so the held re-anchor knob
(reserved in ACTIONS.md since first-pawns) turns ON: a bare mid-trip `PROMOTE` every
`REANCHOR_TICS` (start 32, tuned by the recorded spec error), subtile-accurate so a
correction NUDGES rather than backtracks. Landing still promotes exactly as today.
**Rejected**: keeping the start fan (it re-quantizes an interrupted pawn to a tile —
the backtrack the user is deleting); no re-anchor at all (unbounded drift for
late-join observers).

## F5 — integer polyline: octile A* + string-pulling on DOUBLED corners {#f5}

Route determinism stays integer-exact: A* on tile centers with octile costs
(integers ×5/×7 for straight/diagonal — no floats in the heap), then a funnel/
string-pull pass whose corner points live on the DOUBLED grid (tile corners and
centers are both integers ×2). Line-of-sight = supercover traversal with the
pathfinding F4 corner rule; the swept line must clear the footprint
([F7](#f7)). Chord LENGTHS (the only irrational) are computed per-observer in f64 —
IEEE sqrt is correctly rounded, so even they agree bit-for-bit.
**Rejected**: any-angle A* (Theta*) — costlier per hop and needless once
string-pulling runs; float heap keys (tie-break determinism gets fragile).

## F6 — content keeps tics/tile; consumers derive tiles/tic {#f6}

`walks` levels stay authored in tics/tile (the derived `ground_speed` stat is
untouched — input-rework F8's law that a TIC_HZ change changes wall speed still
holds). The INVERSION happens at eval: `tiles_per_tic = 1 / ground_speed`. Chord
duration = `ceil(len × ground_speed)` keeps all arithmetic in tic space, so the
event-shard queue math, the ring estimate, and the npc deadline stay integer tics.
**Rejected**: re-authoring content in tiles/tic (a corpus sweep for zero gain).

## F7 — clearance is FOOTPRINT-RADIUS-AWARE from day one; 1×1 ships {#f7}

The user names the footprint as part of the model. The LOS/clearance signature takes
a footprint radius (in half-tiles) and inflates the blocked set by it; this stream
ships every pawn at radius 0 (1×1, today's truth) and the multi-tile future changes
an argument, not an algorithm.
**Rejected**: hardcoding 1×1 (the documented design names footprints; simplifying it
away is the preserve-future-intent failure).
