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

- [ ] Sample the baked `normal-cold`/`normal-warm` composites in the slot pass
      (warm-over-cold by warm coverage, the blit's rule) for EVERY texel's N·L; retire
      `receiverNormalAt`. Acceptance: grep-clean of the def-quadrant sampler; the pass
      binds both composites; live render.
- [ ] Verify the consumers inherited: the user's walls shade by their analytic normal
      pitch under a torch; tile ground normals shade (not flat-up); material normal
      detail reads under raking light; movers shade per facing. Acceptance: captures of
      each case in `completed.md`.
- [ ] Re-tune the shading interplay against captures: the wrap floor, ambient×AO, and
      the ×4 decode together — pools bright, crevices dark, backsides readable.
      Acceptance: A/B captures beside the strip's before-images; differential exactness
      (`__lightexact`) still bit-identical.

## P3 — the world displays correctly

- [ ] The joint drill: cold load at the fixture, both zooms — torch pools, grounded
      silhouette shadows, shaded walls, the wolf walking, the human by the fire.
      Acceptance: captures in `completed.md`; **the user's eyes are the exit criterion**.
- [ ] Open the PERFORMANCE successor stream (steady-state gating: skip-when-unchanged,
      per-light scissored updates, dirty-rect uploads) with this stream's final numbers
      as its baseline. Acceptance: the successor folder exists with its baseline
      recorded; this stream marked done.
