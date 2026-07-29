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

## 2026-07-28 · P4 cut 3 — conservative fine receiver classification (STREAM 10/10)

User: a few SQUARES of the wolf's shadow still land on top of the billboard, and named the
tree mechanism (ground shadow drawn, billboard drawn over it). Root: the fine receiver bake
sampled `receiverAt` at the texel's min corner only, so edge texels the sprite partially
covers classified as GROUND — the fine-presence cut (which IS the "billboard drawn over"
mechanism, per the deleted-refine comment) never fired there, and P4's delta carved the
wolf's own shadow under its sprite pixels, one 1-unit square at a time. Fix: in
RECEIVER_FINE, a texel classifies as on-a-hot-billboard if ANY corner or the centre touches
a hot billboard's silhouette (adopt id, max coverage); cold receivers keep the tight
single-sample test (shipped edge look). Receiver rects already carry ±1 tile margin — the
adopted ring re-bakes on motion. Verified: typecheck + shader compile clean, 120.2 fps with
the wolf walking, scene shadows intact; the squares-in-motion check is the user's.

## 2026-07-28 · P4 — ground shadow before the billboard (1/1) · STREAM DONE 9/9

User (after P3): motion still off; then the directive — the light calculations must take the
sub-unit offset into account, and the ground shadow must draw BEFORE the billboard like trees.
Landed in two cuts: (1) all four gather/receiver record-position decodes now add the
`billboard_data.B` sub-unit lanes (the P3 lanes were stamped but only the sprite consumed
them — lighting still snapped to the 8 px unit grid; user's guess, correct). (2) The hot
lightmap is now a pure CORRECTION over cold: receiver texels deposit body − ground (the
ground term mirrors the cold pass's exact expression — receiver demoted, ndl 1, no glint,
uncut cold shadow), mover cast shadows stay negative deltas, and the blit sums cold+hot
unconditionally — the per-pixel `× (1−wcov)` cold zeroing is DELETED (it mixed pixel-granular
coverage with texel-granular deposits; the seam showed as the wolf's own shadow drawn over
its sprite fringe). Verified: typecheck + shader compile clean, scene lighting intact (torch
pools/tree shadows unchanged), resting wolf lit with its shadow attached beneath; the
in-motion feel is the user's oracle (the artifact never shows in stills — their observation).
En route: `docker restart edge-edge-1` KILLS the edge (the container is a dev shell running
`sleep infinity`; the binary is exec'd) — `bin/rd deploy edge` is the restore, edge relisted
+ login healthy.

## 2026-07-28 · P3 — one position authority (1/1) · STREAM DONE 8/8

The user's live check caught a residual: a dark wolf-shaped ghost trailing the moving wolf
(~0.5 tiles) — see I1 (first diagnosis wrong; corrected). Root cause: sprite baked from the
prim's CPU float x/y while lighting read the record's unit-quantised position — two
authorities. Landed: `billboard_data.B` sub-unit anchor lanes (bits 11–13/8–10, eighths of a
unit = 1 world px; VARIABLES.md updated) stamped by `billboardDataFor`, which now also SNAPS
the hot prim's x/y to the record-decoded anchor — the record is the authority; sprite bake,
zdepth, receiver rects, and lighting all derive from the one stamped datum. Fast-path
compare extended with the sub lanes so sub-only steps rewrite (and dirty) correctly.
**Verified live** (tracked multi-trip soak, camera-follow, n + e/w facings over lit ground):
no ghost in any frame, shadows attached; anchor whole-px mid-walk; 8 s counter drill —
moverDirty 100 calls / **100 changes (1:1 with the sprite step now, by construction)**,
backstop 0, cold bakes 110 (camera window slide, not mover churn), hot 30.4k/8 s (~40% up
from the finer cadence), **120.1 fps**.

## 2026-07-28 · P2 — lockstep verified (2/2) · STREAM DONE 7/7

The drill: anchor-corrected residual between the rendered position and the record —
**max 0.067 tiles (= exactly the 1-unit bound), mean 0.029**, at **120.1 fps** (zoom 1) and
**120.0 fps** (zoom 0.25, backstop still 0, record stepping); the 57.6k cold bakes during
the zoom check were the TRANSITION's owner-change re-bake (always the deal), confirmed by a
settled re-measure: steady-state walking = **cold 0, hot ~22k/8 s**. Docs:
`intent/tiered-lighting.md` gained the ONE-hot-dirty status line; the pawn-render folder
links here. Remaining oracle, honestly: the user's own eyes on the live tab for the 1:1
FEEL — they caught the artifact; the numbers say it's gone.
