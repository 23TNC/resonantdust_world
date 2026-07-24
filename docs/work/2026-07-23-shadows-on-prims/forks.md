# Forks — shadows on prims

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Self-exclusion — by same-tile vs by prim-id {#f1}
**2026-07-23 — DECIDED (user): exclude by same TILE.** Pass 2 skips casters on the receiver's own tile
(self + same-tile neighbours). That kills the self-shadow (a prim blocking its own crown). Excluding by
prim **id** would be exact (only the receiver, not neighbours) but needs the prim id in the depth/presence
map. Same-tile is the agreed first cut; **inner-tile ordering** (two prims on one tile) is a later
refinement — it needs sub-tile UNIT rows (16/tile) anyway, same as the ground self-cast note.

## F2 · Other-caster height residual {#f2}
**2026-07-23 — DEFERRED (the old F6 caster-height).** Excluding self fixes the dominant artifact. A
residual remains: an *other* caster **shorter than the crown**, sitting between the crown and `G`, can
still spuriously shadow — because `casterCover(G)` can't tell a blocker before the crown from one after
it. For a light **above/north** of the receiver those casters throw their shadow the other way, so it's
rare. The true fix folds the caster's HEIGHT into the test (emit a per-caster ceiling — the shadow-ceiling
idea from the lighting stream's F6). Polish, not a blocker.

## F3 · Two maps + composite → ONE map, disjoint passes {#f3}
**2026-07-23 — SUPERSEDED (user): one shadow map, no composite.** The earlier plan was two world-space
maps (ground + prim) picked per texel in the lighting bake. The realization: the two passes write
**disjoint** texels — ground shadow is invisible under a sprite, so pass 1 need not write prim texels at
all. So pass 1 writes ground shadow and **discards where surface/`zdepth` says a prim is drawn**; pass 2
writes prim shadow **only** on prim texels; both into the **same** `shadow-cold`. The consumer samples the
one map by world position and gets the right value automatically — **no composite, no depth-mode branch,
and half the shadow maps** (cold/hot × {ground,prim} = 4 → cold/hot = 2). Requires: run ground→prim,
disjoint writes with no clear between, and a **union dirty** (a texel dirty if *either* shadow changed) so
a prim moving in/out re-partitions cleanly. Transparent sprite gaps read as "no prim" (bake discards
transparent) ⟹ ground shadow shows through — correct by construction.

## F4 · Cold/hot for pass 2 {#f4}
**2026-07-23 — lean: mirror the split.** Pass 1 is cold/hot (the #4 light-map split); pass 2 should be too (a
static prim shadowed by a static light re-bakes once; by the dynamic light re-bakes per frame). That's 2
more gather draws — but pass 2 early-exits on tileless fragments, so most of the screen is cheap. Simpler
first cut: a single pass-2 map (no split) to prove the geometry, then split for the perf win.

## F5 · Elevation constant (`z` from `Δ`) {#f5}
**2026-07-23 — TUNE by eye.** `z = k·Δ` where `Δ = base_row − pixel_row`. The pure card model gives
`z = 2·tan(65°)·Δ`, but our sprites are drawn **full-height** while the 65° card is the shadow's OWN
vertical fiction — so the drawn-offset-to-elevation constant is empirical (`z = Δ·sin65` under the
full-height reading, vs `2·tanθ·Δ` under the card reading). Wire the geometry, dial `k` (and the `G`
magnitude) in the browser. `__climb` is the live knob on the scaffold; pass 2 gets its own.

## F6 · Pass 2 cost {#f6}
**2026-07-23 — watch it.** Pass 2 adds gather draws (×2 if cold/hot). Mitigations: early-exit on tileless
fragments (one depth sample + discard — most of the screen); dirty-driven like pass 1 (only changed
regions re-walk the corridor); the corridor bound is already the same as pass 1. If it bites, the
prim-pixel fraction is small, so a coarser pass-2 resolution or a prim-only bounding pass are levers.

## F7 · Pass 2 method — re-sample vs direct CONE {#f7}
**2026-07-23 — DECIDED (user, ratified): the direct CONE method.** Not "re-project each receiver pixel to
the ground and re-sample the combined shadow" (that self-shadowed — [`issues.md#i1`](issues.md#i1)).
Instead, per caster→receiver: `shadow.tip.y` = caster-top projected through the light to the ground; a
receiver is hit iff `shadow.tip.y < receiver.bottom.y < prim.bottom.y` + x-in-cone (near bound free — an
in-front receiver would only catch the shadow on its unseen back); the climb = the caster-top ray∩receiver;
the shape = ray∩silhouette (`v` from the cone, `u` from `recv.x + s·(light.x−recv.x)`). Direct computation
⟹ self-shadow (skip same-tile) and caster-height (real 3-D intersection) are handled by construction, and
it needs no ground-shadow map for billboards. The cull is conservative; the ray∩silhouette is the truth.

## F8 · Back-lit cull — skip a light that can't light the prim's front {#f8}
**2026-07-23 — DECIDED (user): per-(light, receiver) early-out.** A billboard's front faces the viewer;
a light on the FAR side of it lights only the back, which we never render — so the visible front is dark
from that light regardless of shadow. Early-out the whole `(light, prim)` pair (light "above"/behind the
prim ⟹ skip), dropping entire prims per light. Consistent with the display's Lambert term (which already
clamps a back-lit `N·L` to 0) — this just predicts that cheaply from the y-ordering and skips the shadow
WORK. Safe: the only fuzzy zone is a light level with the prim (grazing), where its contribution is ~0
anyway. IMPLEMENTATION DETAIL: anchor the cheap y-test's sign/threshold to how the diffuse computes `N·L`
(light's true 3-D position incl. height) so the cull never drops a light the diffuse would actually light.
