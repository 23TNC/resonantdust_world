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
- **2026-08-07 — P4: the client glides chords from belief.** StateObject carries
  subX/subY (marshal + TS + headless); authX/authY fractional; wasm `findChords`;
  `walkPath` walks DISTANCE along the polyline; speculation ARMS FROM THE RENDERED
  BELIEF (F4 — no anchor). Drilled: a 22-tile trip speculated as FOUR chords from
  (95.81, 73.94); the scripted mid-trip interrupt's 22-sample render trail TURNED IN
  PLACE at (104.65, 67.7) — max inter-sample step 0.4 tiles = walking pace, zero
  snap-back, matching the worker's own resolve log (I5 both halves).
- **2026-08-07 — P5: the observers.** npc trip estimate = `ceil(chord_len × pace)`
  over its tiles-only mirror (est_tics=27 for a (2,1) trip — Euclidean to the tic);
  lake soak: 6 arrivals, ZERO deadline churn. The intent strip needs no change —
  rings render only on TIMED acts; walk circles are progress-none by TOML.
- **2026-08-07 — P6: the verdict (eyes pending).** ACTIONS.md's re-anchor paragraph
  reconciled with F8 (hops fire AT the cadence, each promotes); memory gained
  chord-movement-delivered (+ index line, client-sync updated). Stack bounce: the
  stale-binary guard caught master/orchestrator against the codec change — rebuilt,
  all four crates restarted; arcs green (npc arriving, master seeded 1×, worker
  error-free) and the mid-drill INTERRUPTED trip completed THROUGH the bounce,
  landing exactly on (95,75). B1 records the last step: the user's eyes.
