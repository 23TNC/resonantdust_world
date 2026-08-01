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

## 2026-07-31 · P2 — normal maps, properly

**The sampler moved to the baked composites**: the slot pass now computes every texel's N
from `normal-cold`/`normal-warm` (warm-over-cold by warm surface coverage — the blit's
rule), addressed by the display torus (slot = tile mod cols/rows, +1 apron slot, the
CACHE'S own `slotPx` passed as a uniform, sizes from `textureSize()` — never recomputed).
The composite is screen-space, so sampling at the texel's OWN position P returns the
normal of whatever is drawn there — card or ground — and every consumer (authored tile
maps, per-material RNM detail, mover facings) is inherited from the bake with no frame
mapping. Only the light DIRECTION still branches receiver/ground. `receiverNormalAt`
(the def-quadrant sampler) deleted from `records.ts`; grep-clean; typecheck clean.
Verified live: composites confirmed bound (32×16 slots @ 128 px, 512/512 baked), scene
lit at zoom 1 and zoom 2 with no tile-boundary artifacts at either partition.

**Consumers verified on screen** (captures in the session record): (a) the user's wall
enclosure under its torch shades with graded faces (walls are tile-kind with AUTHORED
normal maps — `brick/wall`, `blueprint/wall` normals exist on disk); (b) per-material
needle detail visibly mottles the torch conifer's lighting — per-texel normal variation,
impossible under flat-up; (c) the placed human side-lit BOTH ways: at (99,52) west of
nothing/east of torch its torch side (east) is bright and its face side dark, moved to
(103,52) the gradient FLIPS (west bright, shadow falls east) — mover facings shade
directionally on the warm tier. Terrain grass has no authored normal map, so open ground
decodes flat-up — the correct fallback, identical to the old ground model, not a defect.
The wolf was not live (npc soak not running); the human exercises the same warm path.

**The tune held** (wrap floor 0.25, ambient×AO 0.75 mix, ×4 decode): against the captures
the pools are bright, shadowed crevices dark but readable (the bush beside the pool), and
the human's dark side clearly legible — no constant changed. Differential exactness
re-run with the new shader: `__lightexact` → `bitIdentical: true`, 0 differing floats
after remove. (Its `glError 1282` is a PRE-EXISTING drill artifact — the optless
baseline `run()` binds the uint prim texture to `sampler2D` placeholders, [I2](issues.md#i2).)
