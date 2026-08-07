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

- [ ] Worker: MOVE_TO seed + MOVE_STEP/CONTINUE hops step via `path_eval` over the
      mirrored composed tiers; unreachable/impathable dest = logged no-op (F5).
      Acceptance: hop logs show a detour; the no-op line fires on a water click.
- [ ] Drill on a water zone at `ambient=0.8`: order a trip across a bay — the pawn
      walks the shoreline (captures + worker hop logs); a click ON water no-ops
      (I5 posture). Acceptance: captures + logs in completed.md.

## P4 — trees block, fells reopen

- [ ] Corpus: tree thing def authors `pathable = false`; worker probe consults thing
      occupancy (composed, kind-0 suppresses). Acceptance: golden re-blessed; drill —
      a treeline blocks the straight line and the path goes around (captures).
- [ ] Drill: fell a blocking tree (lumberjack arc) → the SAME trip now paths through
      the opened cell; a pawn standing on a tree cell walks OUT (I3/F6). Acceptance:
      before/after captures + hop logs.

## P5 — the observers agree

- [ ] wasm: `pawnPath`/`pathNextStep` accessors over the client's composed view; the
      speculation glide follows the SHARED path, not the straight line (I1).
      Acceptance: drill — the glide traces the detour, no rubber-band on promote.
- [ ] npc: trip-time estimate = shared path length, not cheb (I7); wolves keep
      roaming green across water zones. Acceptance: npc soak — trips arrive, no
      early arrival-poll churn in logs.

## P6 — the verdict

- [ ] Docs + memory truth pass (index row records delivery) + a stack bounce with
      arcs green; **the user's eyes close the stream**. Acceptance: docs-check green;
      captures + logs in completed.md.
