# Todo — moving prims + affordable moving lights (execution order)

_Model in [`README.md`](README.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). Ordering principle: **attribute the cost before redesigning it, and build the move
before the thing that needs moving** — every optimisation here is measured against a prim that provably moved._

_Acceptance rule for this stream, from [I40](../2026-07-25-primitive-graph/issues.md): **every check reads the
OUTPUT** (`__gather.debugReadShadow(cls)` bytes / a frame time), never a JS field or an input counter. Three
false verifications this session all came from trusting the near end of a pipeline. `debugReadShadow` defaults
to `cls = 1` (HOT); content torches are class **0**._

_**Execution order amended 2026-07-26**, after [I3](issues.md#i3) came back with a confirmed root cause: P0's
third item needs "ONE moving light" and nothing can move yet, so **P1's first two items run before P0**. The
phases stay as written — only the order changes, and the reason is that the measurement has a prerequisite,
not that the plan was wrong._

## P0 — Attribute the cost before changing anything
- [ ] Instrument the bake to report, per frame: dirty tiles, shadow texels baked, lightmap texels baked, and
      ms split cold/hot. Acceptance: numbers appear for a static frame and a moving frame, and the moving
      total accounts for the wall-clock delta within ~20%.
      **NOT DONE — and deliberately left open rather than ticked.** The phase's *goal* (attribute the cost) was
      met by the zoom and reach sweeps, which needed only `debugDirtyTiles` + wall-clock. The per-texel
      breakdown would still be worth having before options 1/3 are ever built; it was not needed to choose.
- [x] Measure the empty-corridor fast path: what fraction of walks terminate with no caster, and what they
      cost vs a walk that hits one ([F2 option 4](forks.md#f2)). Acceptance: a ratio, so option 4 is either
      promoted to the fix or struck. — corridor 57.49 ms vs brute 87.74 ms → skip already earns ~35 %, **struck**.
- [x] Confirm or kill [I2](issues.md#i2) — sweep zoom 1 / 0.5 / 0.25 with ONE moving light and record dirty
      tiles as a fraction of the map. Acceptance: measured fractions compared against the predicted
      100/50/12.5%; a mismatch means the model is wrong and P2 is re-planned.
- [x] Record all P0 numbers in [`completed.md`](completed.md) as the baseline every later phase is judged
      against. Acceptance: a table a future session can re-run and diff.

## P1 — A real method to move a prim
- [x] Rewrite a carrier prim's POSITION record every frame its prim moved, not only at allocation
      ([I3](issues.md#i3) — `carriedLightFor` guards the write behind `prim === undefined`).
      Acceptance: `carriedLights.get(id)` reports a changed `x/y` after a move.
- [ ] Add the move entry point that mutates the prim graph AND notifies lighting, per [F1](forks.md#f1).
      Acceptance: one call moves sprite, light and shadow together.
- [x] Fix the stale-trail bug: `buildCasters` calls `markLightDirty` without `from`, so a moving light dirties
      only its NEW reach box and leaves the old one baked ([I1](issues.md#i1)).
      Acceptance: the union old ∪ new is queued; no residue behind a moved light.
- [ ] Re-home the debug orbit onto the P1 entry point (its premise — that writing `lastStanding` is a discarded
      read model — was refuted by [I3](issues.md#i3), but a debug-only route is still worth retiring).
      Acceptance: orbit takes exactly the path a real mover takes.
- [x] Prove the move at the OUTPUT: orbit one torch and hash `debugReadShadow(0)` across frames.
      Acceptance: the hash CHANGES frame to frame (it was bit-identical in I40, which is the exact failure
      this item exists to catch).

## P2 — Make a moving light affordable
- [x] Apply the P0 winner from [F2](forks.md#f2) — measurement chose **bound reach**, not the coarser-lod lean.
      Acceptance: 3 moving lights hold ≥60 fps at zoom 1. **120 fps** (8.32 ms), was 23.
- [x] Bound reach against the visible world so a torch stops being ambient at zoom 1 ([F3](forks.md#f3)).
      Acceptance: a light's dirty box is a stated fraction of the map at EVERY lod, not 100% at lod 0.
      **80 % / 34 % / 12 %** at zoom 1 / 0.5 / 0.25 — no longer saturating.
- [x] Re-measure the P0 table with the fix in. Acceptance: a like-for-like row next to the baseline, same
      reach and same light count, so the delta is attributable. — table in [`completed.md`](completed.md).

## P3 — Verify + close
- [ ] Corridor↔brute identity with a MOVING light, at zoom 1 / 0.5 / 0.25. Acceptance: 0 mismatches on a
      **non-zero population on both sides** — an empty-vs-empty pass is vacuous (the D-2 trap that hid a total
      shadow outage behind a passing test all through P5 of torch-thing).
- [ ] Confirm no stale residue: orbit a light through a full circle, stop, and compare the settled shadow
      against a cold bake at the same position. Acceptance: bit-identical, proving the move path leaves the
      map in the same state placement would.
- [ ] Record the final numbers + the retired options in [`completed.md`](completed.md).
      Acceptance: a future session can tell which levers were used and which were measured and rejected.
