# Todo — lighting visual correctness

_Visual only; performance is the declared successor. Every phase ends on screen at a cold
load, both zooms. Stances: [`README`](README.md)._

---

## P0 — land the in-tree fixes + pin the base offset

- [x] Commit + verify the two in-tree fixes: the intensity decode ×4 (authored 1.0 was
      displaying 0.25) and torch reach 16 (user spec). Acceptance: on a cold load the
      torch pools are ~4× brighter and register to 16 tiles (occupancy probe); captures.
- [x] Pin the shadow-base offset per kind: for the conifer, wolf (e + n), human body +
      head, and a flipped (west) draw, probe the record's card base (C, height window)
      vs the DRAWN art's opaque bottom. Acceptance: a per-kind offset table in
      `issues.md` naming each error in units.

## P1 — shadows begin at the base

- [x] Re-model the height window to `[0, subH]` measured from the anchor (the bbox is
      BOTTOM-ALIGNED to the base; `subY` remains the atlas address only) in the shared
      occlusion block — caster, receiver coverage, and fracY together. Acceptance: the
      height window no longer references `fu − subY`; typecheck + shaders compile live.
- [x] Anchor the record's `C` at the DRAWN art's opaque bottom (from the prim box + bbox
      fractions, flip-aware) in the reconciler. Acceptance: the P0 offset table re-probed
      → 0 units of error for every kind.
- [x] Verify on screen: every shadow ATTACHES at its caster's feet — conifer, wolf both
      facings, the human by a torch. Acceptance: captures at both zooms in
      `completed.md`; no floating or mid-air shadow starts anywhere at the fixture.

## P2 — normal maps, properly

- [x] Sample the baked `normal-cold`/`normal-warm` composites in the slot pass
      (warm-over-cold by warm coverage, the blit's rule) for EVERY texel's N·L; retire
      `receiverNormalAt`. Acceptance: grep-clean of the def-quadrant sampler; the pass
      binds both composites; live render.
- [x] Verify the consumers inherited: the user's walls shade by their analytic normal
      pitch under a torch; tile ground normals shade (not flat-up); material normal
      detail reads under raking light; movers shade per facing. Acceptance: captures of
      each case in `completed.md`.
- [x] Re-tune the shading interplay against captures: the wrap floor, ambient×AO, and
      the ×4 decode together — pools bright, crevices dark, backsides readable.
      Acceptance: A/B captures beside the strip's before-images; differential exactness
      (`__lightexact`) still bit-identical.

## P3 — the world displays correctly

- [x] The joint drill: cold load at the fixture, both zooms — torch pools, grounded
      silhouette shadows, shaded walls, the wolf walking, the human by the fire.
      Acceptance: captures in `completed.md`; **the user's eyes are the exit criterion**.
- [x] Open the PERFORMANCE successor stream (steady-state gating: skip-when-unchanged,
      per-light scissored updates, dirty-rect uploads) with this stream's final numbers
      as its baseline. Acceptance: the successor folder exists with its baseline
      recorded; this stream marked done.

## P4 — the user's verdict (2026-08-01): reopened

_Four defects from the user's eyes: lights are dim and do not project their 16-tile
reach; zoom-out breaks lighting (the chain is NOT on the slot torus — probed: at lod 1
the cache window is 64×32 tiles, the lighting RTs cover 32×16, two of three torches
address OUTSIDE the RT); tiles have no working normals (the ground writes nothing into
`normal-cold` — overlay-verified); a gap remains between a tree and its shadow. User
directives: reach-16 lights must read as such; **"use the same machinery you're using
for the existing toroidal maps"** for the lighting window._

- [x] Scale the falloff with STORED reach (today `d0` is pinned at ONE tile, so a
      reach-16 light dies in ~3 tiles: at d=16 tiles attenuation = 1/257). One shared
      GLSL falloff in LIGHT_LANES, both slot writers. Acceptance: a torch pool visibly
      spans its 16 tiles; `__lightexact` still bit-identical.
- [x] Put the lighting chain on the SLOT TORUS (the composites' machinery): texel block
      = `mod(tile, cols) × (TEXTILE_LIGHT >> lod)`, fixed RT sizes, window-unwrap as in
      `fillDisplay`; same for the shadow buffer (`TEXTILE_UNIT >> lod`) and the blit's
      read. Acceptance: all three pools light at zoom 0.5; probe px < RT bounds at lod 1.
- [x] Make ground/tile normals reach the composite: the ground bake writes NOTHING into
      `normal-cold` (overlay: ground transparent/black; grass HAS `normal.l.0.png`, the
      manifest lists it). Find the ground fill's bake path, wire `uNormalTex`.
      Acceptance: the `normal-cold` overlay shows ground normals; grass shades under a
      torch. → resolved as [I3](issues.md#i3): tile normals DO reach the composite
      (walls probed + shading); terrain is geo-tier until texture-generalization.
- [x] Close the shadow-base gap: the bbox threshold (>127 coverage) sits above the drawn
      feet and `fracY = 1` samples one row PAST the subframe's last art row. Lower the
      bbox threshold toward the drawn edge and clamp the silhouette row inside the art.
      Acceptance: on screen the conifer/human shadow touches the visible feet at zoom 1
      and 2.
- [x] The lod-1 wall surface bakes BLACK (`surface-cold` overlay: walls 0,0,0 at
      zoom 0.5 — grid-stem surface missing at the 64 px lod → ambient×ao kills them).
      Diagnose the grid-stem lod resolve; fix or record the fallback. Acceptance: walls
      visible under ambient at zoom 0.5. → does not reproduce post-torus,
      [I4](issues.md#i4).
- [x] The zoom sweep: cold loads at zoom 2 / 1 / 0.5 / 0.25 at the fixture — pools
      project 16 tiles, shadows grounded, walls lit, no window edges. Acceptance:
      captures in `completed.md`; the user's eyes close the stream.

## P5 — the user's second pass (2026-08-01)

_Directives: strip the material from the tree's albedo + normal; fine position in the
lighting records so shadows GLIDE instead of stepping by units; shadows must land ON a
mover that spans multiple tiles — "I'm guessing it will need to occupy presence in both
tiles."_

- [x] Remove the conifer's material bindings (`packed.0.material` pineneedle,
      `packed.1.material` bark) keeping both channel TINTS (the colour reconstruction is
      exact under identity tint). Acceptance: the tree renders clean albedo (no mottle)
      and smooth normal (no RNM detail) on screen.
- [x] Write + decode the `fine.x/fine.y` lanes (u4 each, 16ths of a unit): the
      reconciler packs `round(pos·16)` carry-safe; ONE shared `primPos()` decode feeds
      every consumer (slot Lpos, occlusion C, receiver covers/baseY, gather L).
      Acceptance: a moving prim's shadow glides smoothly on screen; `__lightexact` still
      bit-identical.
- [x] Register a prim's presence in EVERY tile its drawn box x-overlaps (the wolf's
      2-tile card), not just its anchor tile. Acceptance: `__zprobe` on the wolf's
      off-anchor tile resolves texels to the wolf's record; a torch shadow lands on the
      wolf while it straddles tiles; droppedReceivers stays 0 at the fixture.
- [x] A prim never casts onto ITSELF (user): the refine skips the stored caster when it
      equals the texel's receiver. Acceptance: no self-shadow bands on the wolf/human
      flank under a torch; `__lightexact` bit-identical.
- [x] The normal select honours the PAINTER'S KEY (user: the wolf's normal embossed a
      tree it stood behind): `sceneNormalAt` picks warm only where the warm pixel WINS —
      the blit's zdepth-B rule — not wherever warm coverage exists. Acceptance: no mover
      normal reads through a cold thing drawn in front; live capture.
- [x] Billboards take NO shadow from a light north of their plan line (user: those
      shadows land on the card's BACK, never drawn): the refine skips billboard
      receivers when `Lpos.y < baseY`; back-lit fronts rest at the N·L wrap floor.
      Acceptance: the human south of the torch shows no tree-shadow bands; capture.
