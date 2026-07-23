# Blockers — def frame/anchor rework

_Open blockers; remove the row when cleared._

---

- **P5 · prim size sourcing — USER-DEFERRED, scope TBD.** The user explicitly deferred moving prim
  size off the DSL onto the def's bbox model ("This changes how we are specifying our prims size,
  currently we pick it up from the dsl. **We will fix that later.**"). P5's scope (which consumers
  move — `thingPlacement`/`thing_layout`, server occupancy, [F3](forks.md#f3) reported-x/y
  positions) is the user's call and touches the DSL/content pipeline beyond this stream's shadow
  focus. P0–P4 are complete + verified; the stream idles here until the user scopes P5.

_(P0 ratification cleared 2026-07-23: F1 as u4 tiles−1 aligned to ZONE_DIM, F4 as u12 ±2048 +
nudge anchors, arithmetic fixes accepted.)_
