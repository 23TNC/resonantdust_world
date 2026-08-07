# Completed — pathfinding

- **2026-08-07 — P0: the paper.** ACTIONS.md §Movement: the greedy-straight-line seam
  paragraph became the pathing law (ONE `path_eval` over a probe closure; pathability
  DERIVED from the composed tile ⊕ thing tiers, never stored; per-hop stateless
  recompute; 8-way no-corner-cutting; unreachable = logged no-op; leaving always
  legal). VARIABLES.md: `pathable` (absence = true) documented on the tile schema
  (water authors false) and the thing schema (the tree authors false; kind-0
  suppression reopens). Verified: `bin/rd docs-check` green.
