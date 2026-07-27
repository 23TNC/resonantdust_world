# Plan — plane intersection instead of projected quads

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance rule for the whole stream.** P0–P2 must produce **bit-identical** shadow output — this is a
refactor, and any pixel that changes is a bug, not a tuning result. Only from P3 does output legitimately
differ. Every phase re-runs corridor↔brute identity before being ticked.

## P0 — Prove the two predicates agree, before deleting anything

- [ ] Add `__preddiff` to `GATHER_FRAG`: run BOTH `shadowCover` and the `(s,t)` inversion for the centre sub-light per caster and write 1.0 where they disagree, so disagreement is visible on screen rather than assumed.
- [ ] Sweep `__preddiff` over the 3-torch scene at zoom 0.25/0.5/1/2 and record the disagreeing-texel count per zoom in `completed.md`. Expect near-zero; a non-zero count means the two differ and P1 must reconcile, not delete.
- [ ] Characterise every disagreement found: for each, name which of `reachU` clamping, `SHADOW_BASE_PUSH`, or the `t`/`s` range bounds causes it. No deletion until each has a named cause.

## P1 — Make the ray inversion the sole test

- [ ] Hoist `worldTiltRad` out of `shadowCover`/`casterCover` to a per-pass value passed down the walk — it is a texel fetch returning a pass-constant, currently re-fetched per caster per texel.
- [ ] Replace `shadowCover`'s quad build with an early `(s,t)` inversion for the centre sub-light, returning 0 on `t` or `s` out of range, so a MISS costs a subtract, a divide, a multiply-add and two compares.
- [ ] Re-apply the reach bound explicitly: `projectTop` clamped the projection to `reachU`, and that bound is what makes a shadow unable to escape the corridor's reach box. Add the equivalent distance test or the identity proof breaks.
- [ ] Re-apply `SHADOW_BASE_PUSH`: it nudged the base south to close a caster/shadow seam and lived in the quad corners. Fold it into the `t` range or the anchor, and confirm the seam does not return.
- [ ] Delete `shadowCover` and `projectTop` once nothing calls them, and delete the `cross2` helper if it has no other caller.

## P2 — Verify and measure the refactor

- [ ] Diff shadow-cold against the pre-change build at 16 forced taps, on the 3-torch scene at zoom 0.5. Requires 0 differing texels; anything else means P1 changed behaviour and must be fixed before proceeding.
- [ ] Re-run corridor↔brute identity (`__corridor` toggle, `debugReadShadow(0)` for the COLD class) and require 0 differing texels.
- [ ] Profile the cold gather draw against the pre-change baseline (1 moving reach-16 light, 6-tile orbit, zoom 0.5, `EXT_disjoint_timer_query_webgl2`, cold draws only) and record the delta in `completed.md`.
- [ ] Profile a MISS-heavy configuration specifically — dense casters, small reach — since the predicted win is on misses and the standard fixture may under-report it.

## P3 — Two-ray classification (light ± radius)

- [ ] Replace the N-tap loop with two rays at light ± emitter radius along the caster's cross-axis, classifying each texel umbra / penumbra / lit from the two `(s,t)` results.
- [ ] Implement the straddle rule: when the two samples land on opposite sides of the card, take the CENTRE ray's presence. Without it, points close behind a caster read lit — neither extreme ray hits — leaving a bright notch at every trunk.
- [ ] Verify the notch is absent at the trunk of every torch-lit conifer at zoom 2, with and without the straddle rule, and record both screenshots.
- [ ] Confirm the gate now admits penumbra outside the old hard quad: compare the lit/shadowed boundary against the 16-tap build and show the outward feather is present rather than clipped.

## P4 — Fill the wedge

- [ ] Derive coverage analytically from the two `s` values where the caster's own card edge is the boundary, instead of returning a flat 1/2 for the penumbra class.
- [ ] Decide and record in [`forks.md`](forks.md#f5) how interior silhouette detail gets its gradient — more samples, or a distance field in the atlas — since two near-binary opacity samples cannot produce a ramp there.
- [ ] A/B the filled wedge against 16 taps for mean and max absolute error over the shadow map, and against 2 taps for cost, recording both in `completed.md`.

## P5 — Close the other half of the tile clip

- [ ] Pad the caster BUCKETING box in `buildCasters` by the penumbra reach so a caster is registered in every tile its feather can touch, not just the tiles its body covers.
- [ ] Size the pad from `pw` per caster rather than a constant — the ground feather is the emitter scaled by the projection factor, so a tall caster under a low light needs a much wider pad than a short one.
- [ ] Verify the hard tile-aligned clip is gone at zoom 2 on the case in [moving-lights I17](../2026-07-26-moving-lights/issues.md), and re-run corridor↔brute identity afterwards.
