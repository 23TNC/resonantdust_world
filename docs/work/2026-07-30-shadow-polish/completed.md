# Completed — shadow-polish

_Dated entries as items land: what landed + how it was verified._

## 2026-07-30 · P0 complete (3/3); P1 fix landed, drill pending

P0's pins are in `issues.md` with probe values: bug 1 = the ns card anchored at
base-centre (half a body south of the center line, registration rows included); bug 2 =
three cooperating leaks (tight-box narrower than the drawn sprite → ground-classified
flanks; unit-res shadow NEAREST-upsampled across the silhouette; bug 1 magnifying n/s
self-shadow) — the receiver map itself proved HEALTHY (855 rect-exact texels), and the
user's overwrite model holds. **P1's fix is IMPLEMENTED + typechecked**: `casterCoverNS`
spans `[A.y − W, A.y]` and the CPU registration rows `[ay − ws, ay]`. NOT yet verified:
the corridor↔brute identity diff and the live n/s drill (head tracks head) — the drill
technique (npc stopped, wolf driven by `moveEntity`, located via record decode +
worldToScreen) works but is slow; the probe traps a resuming session needs are recorded
in issues.md. The npc is RESTARTED (the world back to normal operation).
