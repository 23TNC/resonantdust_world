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

## 2026-08-01 · P3 — the world displays correctly; the successor opens

**The joint drill** (captures in the session record): cold page loads at the fixture at
zoom 1 (focus 103,55) and zoom 2 (focus 106,56) — all three torch pools bright and wide
(100,51 / 108,53-in-enclosure / 104,59); every conifer shadow springs from its trunk
base; the user's wall enclosure shades under its interior torch with the inner tree's
silhouette cast onto floor and wall; the placed human stands by the fire, side-lit with
its figure-shadow at its feet; and the WOLF walking live (npc soak restarted —
`bin/sim build npc` then `run npc`; log-verified trips (104,56)→(98,54) etc.), captured
mid-trip west-facing at the (100,51) pool's edge beside the human, dim-lit by falloff
exactly as its distance says. No floating shadows, no tile-boundary artifacts, no
console errors at either zoom. **The visuals now await the user's eyes — the stream's
declared exit criterion; reopen on their verdict if anything reads wrong.**

## 2026-08-01 · P4 — the user's verdict, executed

**Reach-scaled falloff**: `lightFalloff(d, reach, I)` — d₀ = reach/2, linear feather to
exactly 0 AT the stored reach — replaced the pinned one-tile d₀ (which put a reach-16
light at 1/257 of its intensity at its own boundary: the "very dim, doesn't project"
verdict, quantified). One definition in `LIGHT_LANES_GLSL`, both slot writers consume it;
`__lightexact` bit-identical after.

**The chain rides the slot torus** (user: "use the same machinery you're using for the
existing toroidal maps"): slot map, summed map, receiver map and shadow buffer all
address a tile's texel block at `mod(tile, cols/rows) × texelsPerTile`, texels-per-tile
= `TEXTILE_LIGHT >> lod` / `TEXTILE_UNIT >> lod`, fixed texture sizes; writers unwrap
residues (`fillDisplay`'s rule), the gather's tier-2 neighbour taps and the blit's
bilinear WRAP by `pmod` (a wrapped tap lands on the world-adjacent tile — the torus
makes the seam free). Pinned first by probe: at lod 1 the cache window is 64×32 tiles
while the RTs covered 32×16 — two of the three torches addressed OUTSIDE the map
(px 2336 > 2048), exactly the missing pools. After: all three pools at zoom 0.5, the
summed-map probe answers in-bounds at every lod, `__lightexact` bit-identical.

**Tile normals** ([I3](issues.md#i3)): probed — authored tile normals DO reach
`normal-cold` (wall tile (106,51): real normal (236,100,189), healthy surface) and DO
shade on screen; the flat ground is GEO-TIER terrain (solid colour fills, no textureName)
until texture-generalization lands. Not a lighting defect.

**The shadow-base gap, closed**: two mechanisms — the bbox threshold (coverage > 127)
cut softly-drawn feet out of the box while the blit renders ANY nonzero coverage
(threshold now > 8, the visual edge); and `silhouetteHit` at `frac = 1.0` (the BASE —
fracY is top-down) sampled one texel PAST the last art row, reading transparent
(sampled offsets now clamp inside the subframe). Verified at zoom 2: conifer shadows
spring from the trunks, the wolf walked into the pool on-screen with its shadow attached.

**The zoom sweep** (captures in the session record): cold loads at zoom 2 / 1 / 0.5 /
0.25 — pools project their full 16 tiles at every lod, silhouette shadows radiate and
attach, the enclosure lights inside and spills out, dirt/sand/water regions read, no
window edges, no seams, no console errors. Lod-1 wall surface healthy
([I4](issues.md#i4) — the pre-fix black reading does not reproduce).

**The successor stream opened**:
[`2026-07-31-lighting-performance`](../2026-07-31-lighting-performance/README.md) —
steady-state gating (skip-when-unchanged, per-light scissored deltas, dirty-rect
uploads, pan increments), with this stream's final numbers recorded as its baseline
(N=16 reach-16: 9.76 ms moving / 9.55 ms static; unlit floor 0.104 ms; static ≈ moving
= nothing gated). Indexed in `docs/work/README.md`; this stream marked done there.
