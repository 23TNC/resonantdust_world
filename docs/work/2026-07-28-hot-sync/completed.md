# Completed — hot-sync

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P0 — the before-numbers (1/1)

Console-only instrumentation (patched `billboardDataFor` + `markLightMove`, per-frame rAF
sampler, 1200 frames / 10 s of commanded walking): **sprite re-bakes 228 frames vs record
rewrites 47 vs hot-rect queues 47 — a ~4.9:1 cadence gap** (worse than the theoretical 2:1;
the effective record quantum measured ~0.11 tiles/step, so the lighting JUMPS in tenth-tile
chunks every ~¼ s while the sprite glides at 1/32-tile grain). Rect queues track record
rewrites 1:1 (same origin — confirming `buildCasters` is the sole per-move source). Warm
baked on 114 frames. Raw positional lag read mean 1.009 / max 1.067 tiles, but the 1.0
baseline is the METRIC's anchor-convention offset (record = box base-centre, sampler = tile
coord ≈ constant (0.5, 1.0)); the real positional skew is the residual ≈ 0.067 tiles ≈ 1
unit, plus the untraced one-frame order lag. Judgment metric for P2: cadence must go 1:1 and
the residual stay ≤ 1 unit.

## 2026-07-28 · P1 — one hot dirty (4/4)

Tick order flipped (`moverLayer.tick()` before `panel.tick()` — the frame renders THIS
frame's mover state). The unified entry: `Viewport.moverDirty(id)` →
`ShadowGather.moverDirty(prim, resolver)`, called from `applyVisual` at the SAME eps
crossing as the sprite bake — rewrites the record, queues the hot cls-1 rects (old ∪ new)
+ receiver rects, from one snapshot; `litSeen`'s per-frame clear moved to END-of-tick so the
light-cascade dedup spans both entry points; `buildCasters` demoted to backstop with a
counter. Sub-unit glide returns without dirt (the record's tile|unit encode can't express
< 1/16 tile — lighting steps at its own resolution IN LOCKSTEP with the sprite bake that
crossed the unit). Warm bakes UNBUDGETED (F4 — deferral impossible by construction; the
budget still governs cold). **Verified**: 10 s walking trace — **82/82 record rewrites
landed on sprite-bake frames** (perfect same-frame coupling; P0 had them on independent
schedules ~4.9:1), **backstop originated 0** per-move dirt, record cadence ~1.7× denser
than P0 (stepping at every unit crossing immediately).

## 2026-07-28 · P2 — lockstep verified (2/2) · STREAM DONE 7/7

The drill: anchor-corrected residual between the rendered position and the record —
**max 0.067 tiles (= exactly the 1-unit bound), mean 0.029**, at **120.1 fps** (zoom 1) and
**120.0 fps** (zoom 0.25, backstop still 0, record stepping); the 57.6k cold bakes during
the zoom check were the TRANSITION's owner-change re-bake (always the deal), confirmed by a
settled re-measure: steady-state walking = **cold 0, hot ~22k/8 s**. Docs:
`intent/tiered-lighting.md` gained the ONE-hot-dirty status line; the pawn-render folder
links here. Remaining oracle, honestly: the user's own eyes on the live tab for the 1:1
FEEL — they caught the artifact; the numbers say it's gone.
