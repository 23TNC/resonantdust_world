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
- **2026-08-07 — P4: trees block, fells reopen.** The tree authors `pathable = false`
  (golden diff exactly `thing tree`; worker restarted for the startup-loaded corpus).
  Drilled at the lake's east treeline: cut_down on the pine at (114,72) — the
  walk-dest selector EXCLUDED the impathable tree cell and walked to (113,71) (the
  occupancy gate live); the fell yielded LOGS and a Move To ONTO (114,72) then
  ARRIVED (kind-0 suppression reopens, logs stay pathable — composed-view probe read
  kind 12 there); a female pawn spawned INSIDE the standing tree at (114,71) walked
  OUT to (116,71) (F6 — leaving impathable is always legal). Captures: pie offering
  Cut Down on the pine, the logs square where it stood, the pawn standing on the
  felled cell, the in-tree pawn + her walk-out.
- **2026-08-07 — P5: the observers agree.** wasm gained `findPath` (the shared A* over
  a WINDOWED grid the caller fills — outside the window reads closed, the margin
  tradeoff documented) + `tilePathable`/`thingPathable`; the client speculation's Spec
  carries the shared path (recomputed at every reseed, the worker's cadence) and
  `walkPath` glides waypoint-to-waypoint — drilled: the return lake trip armed a
  21-waypoint detour, the render track traced the west shore, and the landing logged
  `e=0.01 tiles` (no rubber-band; stray e=6 lines are hidden-tab rAF-freeze artifacts,
  not route error). npc: trip estimate = `path_len` over the Bot's TILES-ONLY mirror
  (F8; fallback cheb×2), wander dests re-roll impathable picks, and the drink flow
  drives to the SHORE and names the WATER cell as destination (fire_interaction gained
  a dest param) — the lake-homed thirsty wolf walked to (109,69) and drank (108,70)
  repeatedly (thirst 29.5→35.5, quenched, emotion=1); 4-min soak: 11 arrivals, 2
  deadline re-issues, no F5 spam.
- **2026-08-07 — P6: the verdict (eyes pending).** Truth pass: memory gained
  pathfinding-delivered (+ index line); client-sync notes the glide walks the shared
  path; debug-browser-url carries the webgl `:5174` form with `ambient=`/`cb=`.
  Stack bounce: master/orchestrator/worker/npc restarted on the delivered binaries
  (edge redeployed earlier in-stream) — master seeded, npc trips arriving, worker
  error-free. B1 records the last step: the user's eyes close the stream.
