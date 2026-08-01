# Completed — lighting correctness

## 2026-07-31 · P0 — the pins

Both items done on the live fixture (`focus=104,54`, cold load, then `__lit(true)` +
`__lights(16)`; captures in the session record).

**The headline pin ([I1](issues.md#i1)) is bigger than any named defect: the rework's lit
path is a debug harness.** Off by default; `buildRecords()` one-shot and racing frame
streaming (first call: 0 defs, 0 scene prims — silently; a later manual rebuild: 2 defs,
455 prims); movers (the placed human, the wolf) have NO records — the human stands flat
gray inside a light pool; content torches emit nothing (the DSL light struct is unread);
the `__lights` grid landed off-window (tiles 3..31 vs a window at col 88), with 2 426
dropped registrations from self-overlap and stale emitter prims never freed. The plan is
amended (forks F4): a new P1b wires records to the scene lifecycle before any named fix
is verified.

**The bbox truth table ([I2](issues.md#i2))**: flora exact; the conifer's `frameSpan` is
stored 1 against a TRUE world span of 2 — the writer derives span from the streamed atlas
frame's px (`round(frame.w / SQUARE)`), so every record is lod-dependent and the
conifer's caster card is half its world size. `frameX/Y` also freeze at build time with
no rewrite on lod swap (the old def-swap cascade has no successor). All 4 rotations of a
block share one facing's frame — `base + rotation` is real in the shape, fake in the
data. Wolf/human/west/linked rows unbuildable until P1b (no records exist for them).

**Silhouettes ([I3](issues.md#i3))**: zero visible shadows on the cold-started fixture
(the rework's own after-images required its drill sequence); plus a flickering
concentric-rectangle banding artifact NE of focus, unexplained, carried into P1b.

**Z-ordering ([I4](issues.md#i4))**: the prim `layer` lane is written 0 for every scene
prim — the record-side half of the z contract does not exist yet; the visible cases
(pawn over tile, head over body) cannot arise while movers have no records.
