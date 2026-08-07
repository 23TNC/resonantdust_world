# Forks — pathfinding (plan-time decisions; each is mine unless the user vetoes)

## F1 — pathability DERIVED from the mirrored shards; NO spacetime pathfinding table {#f1}

The user suspects "we would likely want to hold pathfinding tables in spacetime". The
worker already subscribes to the tile AND thing shards' composed baseline ⊕ overlay
for every zone (its interaction probes read them today), the client composes the same
tiers to render, and the corpus carries the flags — so every observer can DERIVE
pathability with zero new writes, and the felled tree reopens its cell through the
existing overlay kind-0 suppression automatically. A stored pathability table would be
a second copy with its own re-stamp law (every SET/INIT_ZONE/fell would have to write
it too) — the drift class this repo pays for. **Escalation path** if derivation gets
slow: a per-zone occupancy bitmap as WORKER-MEMORY cache (the "per-zone worker state"
scoping already noted in worker comments), still never a shard.
**Rejected**: a `pathability` spacetime table (second truth, extra write on every
mutation); edge-computed paths (the edge is fan-out, not simulation).

## F2 — ONE pathfinder in shared/content (`path_eval`), consumed three ways {#f2}

A* on the 8-way tile grid, written once beside the other evals, reading cells through
a caller-supplied probe closure (`Fn(x, y) -> bool`) so the worker feeds its mirror,
the wasm client its render state, and tests a fixture grid. Corpus flags resolve
through the Bundle (`tile_pathable(kind)`, `thing_pathable(kind)`). Expansion-capped
(I6). **Rejected**: a worker-only pathfinder (client speculation would diverge — I1);
pathing in the codec crate (it needs corpus flags, which live in content).

## F3 — per-hop STATELESS recompute; the MOVE_STEP contract is unchanged {#f3}

Each hop recomputes the path from the pawn's CURRENT cell and steps its first move —
no stored route anywhere. Supersession, re-issue, and mid-trip world changes (a tree
felled across the route) all stay correct for free because every hop re-reads the
world. Cost is bounded by I6's cap and the 6 Hz hop rate. **Rejected**: a route
computed once at MOVE_TO and carried in the chain (stale the moment the world
changes; needs new operands and an invalidation law).

## F4 — 8-way movement keeps, with NO corner cutting {#f4}

A diagonal step is legal only if BOTH orthogonal neighbors it clips are pathable —
a pawn cannot slip between two water cells touching at a corner. Matches how the
sprite visually crosses the corner of both cells. **Rejected**: free diagonals
(pawns visibly clip obstacle corners); 4-way only (trips lengthen everywhere).

## F5 — impathable or unreachable destination = LOGGED NO-OP {#f5}

First pass: the worker resolves the trip at MOVE_TO seed time; a destination that is
impathable, or that A* cannot reach under the cap, logs (`move_to no-op: unreachable`)
and drops the order — the same quiet posture as intent-completion re-validation.
Nearest-reachable-cell targeting is a recorded successor (I5), not this stream.
**Rejected**: silent drop (undebuggable); erroring the queue (a click on water is
normal play, not a fault).

## F6 — leaving an impathable cell is ALWAYS legal {#f6}

Pathability gates the cell being ENTERED, never the cell being left. Worldgen scatter
and drills can strand a pawn on a newly-impathable cell (I3); it must be able to walk
out, and A* must accept an impathable START cell.

## F8 — the npc estimates over its TILES-ONLY mirror (resolved in execution) {#f8}

The Bot mirrors tile baselines ⊕ overlays but holds NO thing occupancy — so the wolf's
trip estimate runs the shared `path_len` over a tiles-only probe (water detours, the
big misses, now count; tree detours still read optimistic), unknown tiles read OPEN,
and `None` falls back to cheb × 2. The wander pick also re-rolls impathable dests
(eight tries) so water picks stop burning deadlines on F5 rejects. The authoritative
arrival poll remains the real completion signal — the estimate is only a give-up
timer. **Recorded successor**: a thing-aware npc world mirror.
**Rejected**: building the thing mirror in this stream (its own increment); leaving
cheb (a shoreline detour fires the deadline mid-trip and churns supersessions).

## F7 — the ambient knob is a URL param riding the existing command pipe {#f7}

`?ambient=0.8` → the parsed url-command list (the `focus`/`zoom` pattern) → the
viewport overrides its hardcoded 0.12 `uAmbient` floor for the session. Debug-only
surface, no persistence, no UI. **Rejected**: a settings-panel control (this is a
drill knob, not a player feature); reusing the pixijs `ambient` param machinery (that
client is legacy-to-retire).
