# Completed — lighting visual correctness

## 2026-07-31 · P0 — the fixes land; the base offset is pinned

**The two in-tree fixes, verified on a cold load**: the intensity decode now restores the
×4 overbright range (authored 1.0 displays as 1.0 — the pools went from dim smudges to
bright, wide light on screen); torches author **reach 16** and the occupancy probe shows
registration to EXACTLY d = 16 and not d = 17, at intensity lane 16. All three content
torches confirmed (100,51) / (108,53) / (104,59), reach 16 each. The reach-8 lore comment
(whose fps table measured the DELETED gather) replaced with the current measurement basis.

**The base offset pinned** ([I1](issues.md#i1)): one mechanism, counted twice — the
letterboxed master's bottom margin puts the record's plan line `C.y` ~0.5 tiles south of
the drawn feet (conifer/wolf 4.5 units) while the height window starts the same margin
above it. Vertical cancels; plan does not — hence shadows visibly detached from feet.
The P1 model zeroes both columns by construction.

## 2026-07-31 · P1 — shadows begin at the base

**The model change** (the user's spec, exactly): the height window in EVERY shader site
(caster ct1 + ct2, receiver coverage, the normal sampler) is now `[0, subH]` measured
from the anchor — the bbox is BOTTOM-ALIGNED to the base, and `subY` remains purely an
atlas address; and the reconciler anchors the record's `C` at the DRAWN art's opaque
bottom (`p.y + (bb.fy + bb.fh) × p.height`), flip-independent.

**The I1 table re-probed**: worst anchor error across every live stem fell from 4.5 units
to **≤ 0.5** — the integer-unit lane's quantization floor (wolf-e measured exactly 0.0;
conifer 0.5; human body 0.1). Height-window bottom is the literal 0 in all sites.

**On screen** (captures in the session record): the pool conifers' shadows now SPRING
FROM THEIR TRUNK BASES (previously ~half a tile south); the human stands lit beside the
torch with its figure-shadow attached at its feet; shrubs likewise; nothing floats or
starts mid-air at either zoom. The wolf was wandering outside any pool at capture time —
its attachment is the same code path and its numeric error is 0.0, noted rather than
photographed.
