# Completed — pathfinding

- **2026-08-07 — P0: the paper.** ACTIONS.md §Movement: the greedy-straight-line seam
  paragraph became the pathing law (ONE `path_eval` over a probe closure; pathability
  DERIVED from the composed tile ⊕ thing tiers, never stored; per-hop stateless
  recompute; 8-way no-corner-cutting; unreachable = logged no-op; leaving always
  legal). VARIABLES.md: `pathable` (absence = true) documented on the tile schema
  (water authors false) and the thing schema (the tree authors false; kind-0
  suppression reopens). Verified: `bin/rd docs-check` green.
- **2026-08-07 — P1: the knob + the flags.** `/ambient <0..1>` chat command + the URL
  replay (`?ambient=0.8`) overriding the blit's 0.12 floor — probe read 0.8, the
  capture is daylight-flat; loader `pathable` (absence = true) on tile + thing defs,
  Bundle accessors degrade UNKNOWN ids open, water authors false. Verified: 45 lib
  tests green; golden's new impathable section reads exactly `tile water`.
- **2026-08-07 — P2: the pathfinder.** `path_eval` — A* 8-way over a probe closure,
  seq-tie-broken heap (the determinism contract), no corner cutting, EXPANSION_CAP
  6144, impathable start accepted, `find_path`/`next_step`/`path_len`. Verified: 7/7
  unit tests (strait detour, sealed wall, corner-slip refusal, walk-out-of-tree,
  immediate refusals, cap-as-unreachable, byte-identical determinism).
- **2026-08-07 — P3: the worker walks around water.** The composed-tier probes
  HOISTED to pass level (one view for carriers AND movement); `cell_pathable` = tile
  ∧ occupant, unknown reads open; MOVE_STEP steps the shared path's first hop
  (stateless per-hop recompute); CONTINUE keys the final-hop PROMOTE on path length
  and idles blocked chains (bare requeue — a cleared blockage auto-resumes); both
  seed sites F5-gated; the walk-then-act dest = nearest pathable cell in the
  carrier's range (I4). Drilled live at ambient 0.8: pawn 0x30800004 minted at
  (95,75), ordered to (113,76) ACROSS the lake — arrived via the north-tip shoreline
  (seen mid-detour on the beach; arrival authoritative in the pawn shard); a click on
  open water logged `move_to no-op: destination impathable or unreachable (pathfinding
  F5)`; drink ordered ON a water cell walked to beach (110,72) and drank (thirst
  86.2→89.2, quenched granted, version-2 advance) — the shoreline law, live.
