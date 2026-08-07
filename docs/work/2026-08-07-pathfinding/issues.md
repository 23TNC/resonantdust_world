# Issues — pathfinding (anticipated; logged before they bite)

## I1 — client speculation glides the straight line today {#i1}

The client tweens a mover from fanned MOVE_TO intent toward dest at the learned rate;
per-hop state never fans. Once the worker detours around water, a straight-line glide
diverges and the pawn rubber-bands on the next promote. The speculation must walk the
SAME shared path (wasm accessor) — this is why F2 exists. Watch the pending-intent
buffer (snap-tween-snap) when the path is longer than the cheb distance.

## I2 — cross-zone trips and the mirror's scope {#i2}

The worker mirrors EVERY zone row at dev scale (its own comment says the per-zone
worker-state plans will scope it later). A* may cross zone borders; the probe closure
must answer cells in any mirrored zone and treat an UNKNOWN zone's cells as
impathable-but-non-fatal (bounded search, not a panic). Do not build zone-scoped
mirroring in this stream — note it and move on.

## I3 — pawns already stand on (or spawn into) newly-impathable cells {#i3}

Worldgen scatters trees; wolves and drilled humans stand where they stand. The moment
trees turn impathable, some pawn is standing IN a tree. F6 (leaving is always legal)
covers escape; drills must include "pawn starts on a tree cell and walks out".

## I4 — drink pathing must stop ADJACENT to water, never enter it {#i4}

The drink interaction's `location = "adjacent"` (cheb ≤ 1) plus walk-then-act compose
currently walks TO a cell near the carrier. With water impathable, the walk
destination must be a pathable cell within range — the intent queue's walk leg and the
menu's distance relax must agree, or drink orders start no-opping at the shoreline.

## I5 — a click on water is normal play {#i5}

F5 no-ops the trip. Successor (not this stream): nearest-reachable-cell targeting so a
water click walks to the shore; menu graying for impathable move_to destinations.

## I6 — A* needs a hard expansion cap {#i6}

An enclosed destination otherwise floods the whole mirrored world per hop, per pawn,
at 6 Hz. Cap expansions (order 4–8k cells) and treat cap-exhaustion as unreachable
(F5). The per-hop recompute (F3) multiplies whatever this costs — measure in the
drill (worker tic-pass timing already logs).

## I7 — the wolf's trip-time estimate is chebyshev {#i7}

`hops = cheb(from, dest)` feeds the npc's arrival poll and the intent strip's ring
estimate. A detour makes real hops exceed cheb — the estimate must become the shared
path's length or arrival polls fire early and rings finish before the pawn arrives.

## I8 — worldgen may seed pawns/things ONTO water {#i8}

Scatter rules don't know pathability. If a tree scatters onto water or a wolf spawns
in a lake, F6 lets it walk out; but check the biome rules' tile/thing pairing in the
drill zone before blaming the pathfinder for a stranded pawn.
