# Todo — def frame/anchor rework (execution order)

_Phases run in order; each verifiable on `/overlayRT shadow-cold` + the corridor↔brute identity
diff (`__corridor(false)` + `__gather.debugReadShadow()`, must stay **0 mismatches**). Items move to
[`completed.md`](completed.md) when done **and** verified. Layout + model in
[`README.md`](README.md); amendments in [`forks.md`](forks.md)._

_**P0–P4 done + verified 2026-07-23** → [`completed.md`](completed.md) (layout ratified with F1 u4 +
F4 signed nudge_x; encode/decode live; identity 0 mismatches at zoom 0.5 AND 1; lod 5→6 swap
exercised live). Only P5 remains._

## P5 · Prim size sourcing (USER-DEFERRED — "we will fix that later")

- [ ] Move prim size off the DSL pickup onto the def's bbox model (align `thingPlacement` /
      `thing_layout` with anchored-bbox semantics). Scope TBD with the user.
