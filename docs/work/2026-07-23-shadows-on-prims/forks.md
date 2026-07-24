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

## F3 · Where to composite the two shadows {#f3}
**2026-07-23 — DECIDED: in the LIGHTING BAKE, per texel.** Both shadow maps are world-space, so the
lighting pass picks per texel — billboard shadow where `zdepth` says a prim is drawn there, ground shadow
elsewhere — and bakes the chosen one into the lightmap. The blit then just samples the lightmap (no
depth-mode branch). Alternative (pick in the blit) keeps the bake simpler but pushes the branch to every
display pixel + needs both shadow maps bound at display; bake-side is cleaner.

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
