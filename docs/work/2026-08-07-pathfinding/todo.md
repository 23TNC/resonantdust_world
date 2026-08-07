# Plan — pathfinding

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] ACTIONS.md §Movement: the pathing law — ONE `path_eval`, per-hop recompute, no
      corner cutting, unreachable = logged no-op, leaving always legal (F2–F6);
      VARIABLES.md: `pathable` on tile + thing defs. Acceptance: docs-check green. →
      the greedy-straight-line seam paragraph replaced by the six-point law; both
      schema examples annotated; green.

## P1 — the knob + the flags

- [x] URL `ambient=<0..1>` on the command pipe (F7): parsed like `focus`, overrides
      the viewport's 0.12 `uAmbient` floor. Acceptance: `?ambient=0.8` screenshot
      shows terrain past the torch radius; no param = unchanged look. → `/ambient`
      chat command + the URL replay; probe read 0.8; capture is daylight-flat.
- [x] Loader: `pathable` flag (default true) on tile AND thing defs; Bundle accessors
      `tile_pathable`/`thing_pathable`; water tile authors `pathable = false`.
      Acceptance: round-trip + default tests green; golden diff = the authored row. →
      45 lib green (unknown ids degrade OPEN); golden's new impathable section reads
      exactly `tile water`.

## P2 — the pathfinder

- [x] `path_eval` in shared/content: A* 8-way over a probe closure — F4/F6 rules,
      expansion cap (I6), `next_step`/`path_len`. Acceptance: unit tests — detour,
      wall, corner-cut refusal, walk-out-of-tree, unreachable → None. → 7/7 green
      incl. determinism (seq-tie-broken heap) and cap-exhaustion-reads-unreachable.

## P3 — the worker walks around water

- [x] Worker: MOVE_TO seed + MOVE_STEP/CONTINUE hops step via `path_eval` over the
      mirrored composed tiers; unreachable/impathable dest = logged no-op (F5).
      Acceptance: hop logs show a detour; the no-op line fires on a water click. →
      probes hoisted to pass level; F5 fired live ("move_to no-op: destination
      impathable or unreachable"); blocked hops idle-and-resume (bare requeue).
- [x] Drill on a water zone at `ambient=0.8`: order a trip across a bay — the pawn
      walks the shoreline (captures + worker hop logs); a click ON water no-ops
      (I5 posture). Acceptance: captures + logs in completed.md. → a minted pawn
      crossed lake (95,75)→(113,76) ARRIVING via the north tip (seen mid-detour);
      water click F5-refused; BONUS: drink-on-water walked to beach (110,72) and
      drank (I4 proven live).

## P4 — trees block, fells reopen

- [x] Corpus: tree thing def authors `pathable = false`; worker probe consults thing
      occupancy (composed, kind-0 suppresses). Acceptance: golden re-blessed; drill —
      a treeline blocks the straight line and the path goes around (captures). →
      golden diff exactly `thing tree`; cut_down's walk-dest EXCLUDED the tree cell
      (picked (113,71) beside (114,72)) — the occupancy gate live.
- [x] Drill: fell a blocking tree (lumberjack arc) → the SAME trip now paths through
      the opened cell; a pawn standing on a tree cell walks OUT (I3/F6). Acceptance:
      before/after captures + hop logs. → the felled cell (114,72) held LOGS and a
      Move To ONTO it arrived; a female spawned INSIDE the tree at (114,71) walked
      out to (116,71) — F6 live; before/after captures taken.

## P5 — the observers agree

- [x] wasm: `pawnPath`/`pathNextStep` accessors over the client's composed view; the
      speculation glide follows the SHARED path, not the straight line (I1).
      Acceptance: drill — the glide traces the detour, no rubber-band on promote. →
      wasm `findPath` (windowed grid) + `tilePathable`/`thingPathable`; the spec
      carried a 21-waypoint lake detour, the render traced it, landed e=0.01 tiles.
- [x] npc: trip-time estimate = shared path length, not cheb (I7); wolves keep
      roaming green across water zones. Acceptance: npc soak — trips arrive, no
      early arrival-poll churn in logs. → tiles-only probe (F8), impathable dest
      re-roll, and the SHORELINE drink (dest = the water, wolf on land); lake soak:
      trips arrive, the thirsty wolf drank from shore (29.5→35.5, quenched, happy).

## P6 — the verdict

- [ ] Docs + memory truth pass (index row records delivery) + a stack bounce with
      arcs green; **the user's eyes close the stream**. Acceptance: docs-check green;
      captures + logs in completed.md. → truth pass + bounce DONE (arcs green);
      B1 holds the one open half: the user's eyes.
