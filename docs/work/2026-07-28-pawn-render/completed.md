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

## 2026-07-28 · P1 — depth-correct composite (2/2)

The blit now decodes both zdepth B lanes and lets a cold THING whose base row sits serially
south (mod-128, wrap-aware, window ≪ 64 rows) of the mover's win the pixel outright (wcov →
0 before every downstream mix, so albedo/surface/emissive/light all follow the winner);
ground never occludes; equal rows keep warm-over-cold (F6). The GLSL-backtick guard hook
caught a comment backtick on the first edit (the memory foot-gun, live). **Verified in the
browser with CREATE-posed wolves**: one row behind the torch trees → crowns occlude the
wolves with only heads above (before-shot showed them painting over the same trees); a
commanded lap south showed the flip both ways — a bush south of the mid-walk wolf occluded
it, and the tall wolf simultaneously drew OVER the tree north of it while the tree south
occluded its feet (one frame, both directions). Zooms 1 / 0.5 / 0.25 + two transitions:
consistent, no drift artifacts. Row-key audit: both tiers' depth resolves are the SAME
function (`Viewport.channels(suffix)` instantiated per tier), so the row convention agrees
by construction; the observed flips landed at the visually-correct rows with no premature
pop.

## 2026-07-28 · P2 — movers into the HOT lighting pass (4/4)

Implemented via F2-AMENDED (see forks): warm prims flow into `ShadowGather.tick` alongside
cold, tagged `hot: true` (a new `Primitive.hot` field — one landmine: `addPrim` copies fields
explicitly and DROPPED the flag, live-diagnosed by monkey-patching `markPrimDirty` from the
console and catching cls-2 rects from `buildCasters`); `billboardDataFor` stamps the class
bit (25) on the root prim record and `resolveCarried` folds it to the leaf; the dirty
machinery is class-aware (hot prims queue cls-1 rects + their OLD box on move, and their
light-cascade overrides the cascaded rect to hot); the shaders enforce the matrix —
`casterOne` skips hot casters in the cold pass, both gather + light passes DEMOTE
hot-receiver texels to ground in the cold pass (the cold map bakes the terrain BENEATH the
wolf — correct the instant it leaves, never re-baked by its motion), and the hot pass takes
hot lights everywhere plus cold lights on hot-receiver texels; the blit blends the cold map
OUT by mover coverage (F3 — mover pixels read ambient + hot only). **Verified live**: with
the npc stopped, 10 s = 0 cold / 0 hot bakes; wolf walking = **0 cold** / ~25k hot over
10 s (the matrix's promise: mover motion re-renders hot only); wolves render toned into the
scene's light with Lambert-consistent shading (the P0 pale-flat look is gone; a wolf in a
dark corner is dark); no GLSL errors; **120 fps at zoom 1 AND 120.3 at zoom 0.25 while
walking** (the 63-light/120 baseline held).
