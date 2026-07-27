# Plan — plane intersection instead of projected quads

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

## How this stream works (user, 2026-07-27)

> _"We are not going to tack a ton of extra stuff to try and enable/disable it. We are going to replace
> and delete the current shadow implementation as we go."_

**REPLACE AND DELETE, in the same change.** No `uRayTest`/`uPredDiff`-style switches, no keeping the old
path alive behind a uniform, no A/B dials. Each phase writes the new thing and deletes what it replaced,
in one commit. If a phase is right, the old code is gone by the end of it. See [F6](forks.md#f6).

**Acceptance is NOT bit-identity.** That was the previous gate and it is WITHDRAWN — we are deliberately
changing behaviour, so P0's measured ~26 % divergence is the expected outcome, not a blocker
([completed.md](completed.md)). What must hold instead, every phase:

- **corridor↔brute identity = 0 differing texels.** The corridor is what ships and the brute walk is the
  only oracle we have; if they diverge the shadow map is wrong in a way no screenshot will reveal.
- **Shadows look right** at zoom 0.5 / 1 / 2 — attached at the base, silhouette intact, no notches.
- **The frame is faster**, on the cold gather draw vs `checkpoint/pre-plane-intersection`.

**Scene budget for all verification: torch reach 8.** Reach 20 put the client past its own measured fps
table, which reads as a hang and trips the GPU watchdog — that is what cost 2026-07-27
([I6](issues.md#i6), and the reach comment in `content/visual/things.rd`). Do not raise it to inspect
long shadows; move the camera instead.

## P0 — Measure the two predicates before replacing (DONE, and it changed the plan)

- [x] Capture a REFERENCE `debugReadShadow(0)` from the current build at 16 forced taps, so the comparison has something to diff against.
- [x] Add a predicate-disagreement mode: run BOTH the quad and the ray solve per caster and paint where they differ.
- [x] Measure the disagreement. RESULT: ~26.5 % of shadowed slots, bidirectional, reproduced on two independent loads. The premise "these are the same predicate computed twice" is refuted.
- [x] Isolate a cause: the `reachU` clamp is confirmed — disagreement falls 21.6 % → 11.9 % as reach goes 8 → 48 tiles. A ~12 % reach-independent remainder is still unexplained.
- [x] Revert the P0 instruments. They were scaffolding, and this stream does not keep scaffolding.

## P1 — Replace the FORWARD projection with the BACKWARD solve, and delete the forward path

_Framing correction (user, 2026-07-27): there is no quad object. `shadowCover` computes four `vec2`
locals and four cross products — arithmetic, not geometry. Both approaches are "math to figure out where
we are"; today's runs FORWARD (project the caster's extremes, test if `P` is between them) and the
replacement runs BACKWARD (invert `P` onto the card, range-check). The backward solve is cheaper and
returns `(s,t)` — the texture coordinate — which the forward one discards._

- [ ] Write the backward solve as a function in `GATHER_COMMON` and call it from `casterCover` as THE predicate — not alongside `shadowCover`, in place of it. One implementation, no switch.
- [ ] Hoist the solve to a SINGLE gate ahead of the tap loop and feed its `s`/`t` into the centre tap's `uv` rather than re-solving. Without the hoist a miss costs N solves where it used to cost one forward test.
- [ ] Expect and record the penumbra tightening this causes: the forward projection covered the caster's full extremes, so a CENTRE-ray gate rejects texels where the centre misses but an offset ray hits. That is the penumbra, and P3 removes the gate rather than widening it.
- [ ] Build the ray gate with NO reach bound first, then test whether one is needed: run corridor↔brute identity, and move a light looking for a stale shadow trail outside its reach circle. Presence may already bound it ([F2](forks.md#f2)).
- [ ] Decide `projectTop`'s `k <= 0` case: it ran the corner OUT to reach when the light sat below the card top, where the ray test reports unshadowed. Reproduce it or drop it deliberately, and record which in `forks.md`.
- [ ] Carry `SHADOW_BASE_PUSH` across — it closed a caster/shadow seam from the quad's base corners. Fold it into the `t` range or the anchor, and confirm the seam has not returned.
- [ ] Hoist `worldTiltRad` out of `casterCover` to a per-pass value: it is a texel fetch returning a pass-constant, currently re-fetched per caster per texel.
- [ ] DELETE `shadowCover`, `projectTop`, and `cross2` if it has no other caller. The phase is not done while the quad is still compiled.

## P2 — Verify and measure the replacement

- [ ] Run corridor↔brute identity (`__corridor` toggle, `debugReadShadow(0)` for a COLD light) and require 0 differing texels. The argument defaults to 1 (hot), which returns all zeros for a cold light and reads exactly like perfect identity.
- [ ] Eyeball zoom 0.5 / 1 / 2 against the checkpoint build: shadows attached at the trunk, silhouette intact, no bright notch behind casters, no new hard clip at tile boundaries.
- [ ] Profile the cold gather draw against `checkpoint/pre-plane-intersection` (one moving light, 6-tile orbit, zoom 0.5, `EXT_disjoint_timer_query_webgl2`, cold draws only) and record the delta.
- [ ] Profile a MISS-heavy case — dense casters, small reach — since the predicted win is on the miss path and the standard fixture may under-report it.

## P3 — Two rays at light ± radius, and delete the tap ladder

_This phase DISSOLVES P1's gate rather than optimising it. Once the predicate is two solves at ± radius,
"does any ray hit" IS the answer — there is nothing left to gate on, and the penumbra P1 clipped to the
centre ray comes back wider than the original forward projection ever allowed._

- [ ] Replace the N-tap emitter loop with two rays at light ± emitter radius on the caster's cross-axis, classifying each texel umbra / penumbra / lit from the two solves.
- [ ] Implement the straddle rule: when the two samples land on opposite sides of the card, take the CENTRE ray's presence. Without it, points close behind a caster read lit — neither extreme ray hits — leaving a bright notch at every trunk.
- [ ] DELETE the adaptive tap ladder and everything serving it: `emitterOffset`, the tier selection, `uTapForce`, `uLadder`, and the `__taps`/`__ladder` dials.
- [ ] Verify the notch is absent at the trunk of every torch-lit conifer at zoom 2, and that penumbra now appears OUTSIDE the old hard-quad boundary rather than clipped to it.

## P4 — Fill the wedge

- [ ] Derive coverage analytically from the two `s` values where the caster's own card edge is the boundary, instead of returning a flat 1/2 for the penumbra class.
- [ ] Decide and record in [`forks.md`](forks.md#f5) how interior silhouette detail gets its gradient — more samples or a distance field — since two near-binary opacity samples cannot produce a ramp there.
- [ ] Compare the filled wedge against the checkpoint's 16-tap output for mean and max absolute error, and against 2 rays for cost.

## P5 — Close the other half of the tile clip

- [ ] Pad the caster BUCKETING box in `buildCasters` by the penumbra reach so a caster is registered in every tile its feather can touch, not only the tiles its body covers.
- [ ] Size the pad from the penumbra width per caster rather than a constant — the ground feather is the emitter scaled by the projection factor, so a tall caster under a low light needs a much wider pad.
- [ ] Verify the hard tile-aligned clip is gone at zoom 2 on the case in [moving-lights I17](../2026-07-26-moving-lights/issues.md), and re-run corridor↔brute identity.
