# Completed — lighting correctness

## 2026-07-31 · P1 — reach is in the data

**The split** (`records.ts`): `prim_data.B` bits 0–9 are now `u4 reach (6–9, biased +1 =
1..16 tiles) | u6 intensity (0–5)`; `VARIABLES.md` updated (layout + the reach paragraph —
the cost-dial reasoning for the 16 cap is written where the lane is). The BLUE-lane GLSL
decodes live in `records.ts` as `LIGHT_LANES_GLSL` (`intensityFromB` / `reachUnitsFromB`),
injected into both passes — CPU registration and GPU walk bound read the SAME stored lane,
which is the agreement-by-construction that F6's derive-and-share existed to fake.

**Verified live** (lane self-test + probes on the fixture): intensity 64 THROWS, reach 17
THROWS, a valid write round-trips exactly (intensity 40 / reach 12 back as written); an
emitter authored `reach 8, intensity 63 (max)` registers at d = 0 and d = 8 and NOT at
d = 9 — registration follows the STORED reach, independent of intensity. `lightReach.ts`
deleted; grep-clean (`reachFromIntensity`/`__reachcheck` gone); typecheck green.

**Honest limits:** (a) the DSL side of authored reach (`&thing.light.reach` → the emit
prim) has nowhere to land until P1b mints content lights — the lane + the registration
honor authored reach today, proven with a hand-minted emitter; the torput plumb rides
P1b's content-light item. (b) "renders identically" was verified as: the pool at (100,50)
renders with the same position/extent under `reach 16` authored as under the old derived
16 — but the debug harness is not run-to-run deterministic (I1: readiness races, stale
prims, a state-corrupting `__lightpass` readback), so a pixel-identical A/B is deferred
to P1b's deterministic cold start. Bonus observation recorded: with the window warm, the
16-light fixture now lands ON-window and real wedge shadows appeared — the pre-split
"no shadows" pin was partly the off-window placement bug.

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
