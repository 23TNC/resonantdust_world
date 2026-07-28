# Completed — pawn-render

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P0 — baseline (1/1)

Added `__bakes()` (cumulative [cold, hot] dirty-slot bake counters on ShadowGather) and a
`tileToPosition` wasm/console export (posing affordance). **Counters**: after standup
(1534 cold / 1522 hot), BOTH classes bake ZERO over 20 s while the wolf wanders — the mover
is invisible to the entire light/shadow system. **Screenshots**: ss_8905qj9on — the flat,
pale, shadow-less wolf amid warm torch-lit terrain (symptoms b+c context; torch emissive
glow = the working control, e); a zoom capture of two CREATE-posed wolves at (100,50)/
(101,50) standing one row BEHIND the torch trees with their billboards painting OVER the
tree crowns (symptom a, unmistakable). Symptom (d) n/s-casts-e/w: movers cast nothing at
all; for cold prims the e/w regime is hard-coded (`coldShadowData.ts:475`) — user-reported,
code-confirmed, demonstrated post-fix instead. Standup also found and fixed TWO out-of-scope
landmines recorded in issues.md: I1 (edge index registration doesn't self-heal → gateway
503s; restored by restart, spun off as a task chip) and I2 (a pawn resting past half the
tic ring loses its base to serial wraparound and teleports to position 0 on next touch —
FIXED at the root: the shard gc now re-stamps each kept latest-clean base row to the horizon
once it lags a quarter window, so parked pawns' bases stay serially near forever; all
modules republished).
