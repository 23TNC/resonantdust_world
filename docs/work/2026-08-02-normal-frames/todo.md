# Plan — normal frames

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.**

- **Measure `N·L`, do not eyeball brightness.** The wrap floor `mix(0.25, 1.0, ndotl)` compresses the
  visible range, so a wrong normal still looks lit. Probe the term itself.
- **A light orbiting a billboard must swing its shading.** That is the number; a static screenshot
  cannot show it.
- **The ground must not move.** It is drawn top-down, so its normals are already correct
  ([F1](forks.md#f1)); a change there is a regression, not a fix.
- **`occlusionDiffering` stays 0.** This touches shading only — any change to the occlusion walk is a
  mistake, and `__gather()` is the guard.
- **Never quote a shadow-texel count** ([z-positioning I17](../2026-08-01-z-positioning/issues.md#i17)):
  the buffer is incremental and the counts are not reproducible.

**Fixture:** `?user=Claude&focus=95,56&zoom=2`, one `tick()` to force the sync (the tab is
backgrounded, so rAF never runs), then `__lights(n)`.

## P0 — Prove the frame error before changing anything

- [ ] Add an `N·L` probe: for one texel, report the sampled normal, the light direction, and the dot. Acceptance: one call returns all three, so the frame mismatch is visible as numbers rather than inferred.
- [ ] Measure the shading swing for a light due EAST vs due WEST of one billboard. Acceptance: the two `N·L` values are recorded; a camera-facing normal makes them near-equal, which is the defect stated as a measurement.
- [ ] Do the same for a ground texel. Acceptance: recorded separately, because ground and billboard take different rotations and the baseline must distinguish them.
- [ ] Confirm the flat background really is `(0,0,1)`. Acceptance: sample a texel outside any silhouette and show it decodes flat-out-of-image, so the camera-space claim rests on the data and not only on the design doc.

## P1 — One angle, two rotations, one place ([F3](forks.md#f3))

- [ ] Add `normalToWorld` for a BILLBOARD to `worldTilt.ts`, TS and GLSL ([F1](forks.md#f1)). Acceptance: a flat `(0,0,1)` maps to a horizontal, ground-parallel vector — the card's face normal.
- [ ] Prove the GROUND needs NO rotation. Acceptance: its flat `(0,0,1)` already reads as straight up, because the grid is drawn top-down — the existing flat-up path is correct and must be left alone.
- [ ] Derive the angle from `WORLD_TILT_DEG`, never a literal, and say at the constant that the tie is a CONVENTION ([I5](issues.md#i5)). Acceptance: one dial, with the art-vs-geometry distinction written where someone would change it.
- [ ] Round-trip the rotation. Acceptance: unit vectors stay unit and the inverse returns the input across a spread of inputs, CPU and GPU agreeing, the `LIGHT_LANES_GLSL` pattern.

## P2 — Rotate the normal, collapse the branch ([F2](forks.md#f2))

- [ ] Rotate the sampled normal into world space, choosing the rotation by receiver-ness. Acceptance: `sceneNormalAt`'s result is world-space at every consumer; the surface class picks the rotation.
- [ ] Build ONE world-space `ldir` and delete the second frame. Acceptance: the sprite/ground branch survives only where it picks a rotation, not where it redefines the light.
- [ ] Re-check the walk. Acceptance: `occlusionDiffering` 0 over 131,072 comparisons — shading changes, occlusion does not.
- [ ] Re-measure the east/west swing from P0. Acceptance: the swing is now large and correctly signed for the billboard, and for the ground separately.

## P3 — The mirrored facing ([F4](forks.md#f4), [I2](issues.md#i2))

- [ ] Determine whether west-facing normals arrive with an inverted X. Acceptance: a stated answer with the evidence — light an east- and a west-facing sprite from the same side and compare which side reads lit.
- [ ] If inverted, negate X off the same `flipX`/rotation the albedo already uses. Acceptance: both facings light consistently from one light; if not inverted, record why not so it is not re-investigated.

## P4 — Pin the angle and verify ([F1](forks.md#f1))

- [ ] Confirm the sprite obliquity against the art ([F1](forks.md#f1)). Acceptance: a billboard's flat normal comes out horizontal, and a lit sprite reads front-on rather than overhead-lit — the check is whether the art agrees, not whether the maths closes.
- [ ] Capture a light orbiting one billboard. Acceptance: the lit side tracks the light through a full circle, beside the before-capture.
- [ ] Re-measure the frame. Acceptance: no measurable cost — this is a rotation applied to a value already fetched, not a new pass or a new fetch.
- [ ] Record the convention in `design/de-lighting.md`. Acceptance: it states that maps are authored camera-space and rotated at consumption, and names the two rotations — so the next generator change does not silently re-break it.
