# Completed — chord movement

- **2026-08-07 — P0: the paper.** ACTIONS.md §Movement: the per-tile hop paragraph
  became the CHORD law (string-pulled polyline route, one hop per chord writing the
  chord's subtile END at `+ceil(len × ground_speed)`, speed inverted at eval,
  mid-chord resolve-on-touch as law, tile = floor half-open); the promote cadence
  rewritten — the seed fans NO position (the 2026-07-28 start anchor retired, the
  backtrack visual named as the reason) and the held every-N re-anchor turns ON
  (REANCHOR_TICS=32, subtile-accurate so corrections nudge). VARIABLES.md: the
  position layout shows the low byte's TWO per-shard readings (cold `type:4|layer:4`
  vs pawn `sx:4|sy:4` sixteenths), the graceful-degrade argument, and the half-open
  edge-ownership rule. Verified: `bin/rd docs-check` green after each file.
- **2026-08-07 — P1: the codec.** `pawn_position`/`position_subtile`/
  `position_to_point`/`point_to_position` (rounding carries the sixteenth into the
  next tile; `position_to_tile` unchanged = the floor). Verified: 71/71 codec tests
  incl. the round-trip, the old-style (0,0) degrade, and the half-open carry.
- **2026-08-07 — P2: the route.** `find_path_octile` (integer ×5/×7 costs),
  `line_of_sight` (Dedu supercover; exact corner = BOTH flanks; the start never
  probed), `find_chords` (greedy string-pull; the footprint radius inflates ONE probe
  shared by search and smoothing), `chord_len` (IEEE hypot). Verified: 13/13
  path_eval tests — one-chord open ground, the bent polyline's self-consistency,
  corner-slip refusal, walk-out, radius-1 corridor refusal, byte-identical reruns.
- **2026-08-07 — P3: the worker walks chords.** Base/Payload carry the row tic; hops
  land one STRIDE (= REANCHOR_TICS at pace, chord-capped — F8) along the first chord
  writing FRACTIONAL subtile positions, every hop PROMOTEs, arrival normalizes onto
  the dest point; both seed sites fan intent only (no PROMOTE); mid-chord
  RESOLVE-ON-TOUCH at superseding seeds AND the arm's location validation, reading
  the OLD chain's pending hop for the walk proof (I10) with queued events mirrored
  (I11). Verified live at the lake: subtile rows ((3,1)/(14,11)/(9,6)) re-stamping at
  EXACTLY +32 tics; a 6.803-tile trip landed in 164 = ceil(6.803 × 24) tics; the
  interrupt resolved `(99.0, 69.9375) → (99.25, 69.6875)` — a quarter-tile nudge
  along the chord, no teleport, no backtrack.
