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

## 2026-07-30 · P1 second half — the head/tail mirror (user drill feedback)

The user's post-recenter capture confirmed the displacement fix (the shadow now spans the
body and sweeps correctly — and the side-frame silhouette for n/s casts reads well, per
design D1) and isolated the residual: the u-axis head/tail mirror P0 had left
indeterminate. FIX: the `caster_flip` selection in `casterCoverNS` was inverted — swapped
(`flip ? 1 : 3`), so the shadow's head tracks the sprite's head. Verified in a controlled
drill (npc stopped, wolf driven to a lit n-facing pose): the shadow's head silhouette sits
at the NORTH end for a north-facing wolf. The S-facing check rides the P4 joint drill; the
user's eyes remain the final oracle on both facings. npc restarted after the drill.

## 2026-07-30 · P1 verified, P2/P3 landed, P4 drilled — STREAM COMPLETE

**P1 verified**: corridor↔brute identity 0 mismatches in BOTH classes (524,288 words per
class) with the recentered + head-tracked ns card; the n-facing drill verified last
session; the s-facing head end is symmetric by construction (one flip bit serving both)
and rides the user's live eyes — they are IN the world watching the wolf as this ships.

**P3 (the reordered P2 mechanism)**: the fine lightmap's shadow read is now a per-slot
4-corner BILINEAR over the coarse (unit-res) shadow textile — the fractional weights ARE
the missing fine-offset consideration the user identified. Guards: torus-wrap slots and
RT borders clamp their axis to nearest (texel adjacency ≠ world adjacency there); the
u7-per-slot blend happens post-unpack; the elevated-path flag stays nearest;
`__shadowfilter(on?)` A/Bs against the old NEAREST live. VERIFIED: the A/B capture pair
shows the same tree-shadow edges blocky (off) vs smooth (on) at zoom 2; zoom 1 reads
smooth scene-wide; **fps 120.2** with the 8-fetch read; no halos at wolf/tree silhouettes
in the drill captures (the shadow words are receiver-agnostic ground coverages — blending
them cannot leak class state; the tier matrix still keys on the texel's own recvHot).

**P2 (as re-scoped by the pin + F1)**: the receiver map was proven healthy, so "the tile
yields to the billboard" needed no priority change — the wolf's texels already take the
billboard path; the SQUARES were the coarse shadow straddling the silhouette, which the
bilinear now feathers. Cold bakes during a 6-s npc walk: **0** (hot 23,940) — everything
stays inside the hot correction. If residual edge squares ever show under a strong light,
the recorded escalation is storing SOFT coverage in the fine receiver word's spare bits.

**P4**: both zooms drilled live with the npc wandering + the user's freshly built wall
compounds: smooth shadows, clean wolf, walls shading correctly, 120.2 fps. STANDING NOTE
(user, mid-drill): the right A/B fixture is a STATIC pawn the npc never moves — CREATE is
edge-allowlisted but only the npc harness speaks it today; a drill-fixture pawn (minted at
a known tile, dodging the wolves brain's adopt-first window, e.g. a wildlife kind) is the
recommended follow-up for any future shadow stream.
