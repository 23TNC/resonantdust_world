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
