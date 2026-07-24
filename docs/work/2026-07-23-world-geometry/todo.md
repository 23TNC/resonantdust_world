# Todo — world geometry (execution order)

_Model in [`README.md`](README.md); decisions in [`forks.md`](forks.md). Verifiable in-browser against
the 3-light rig; the shadow's own corridor↔brute identity is the regression net._

## P0 · Write the model down (authoritative)
- [ ] Put the ratified model in a **durable** home ([forks F1](forks.md#f1)) — not just this stream:
      view plane, 65° ground tilt, billboards parallel to view, `z = sin65·(px.y − base)`, ground/base
      selection, `s = z_light/(z_light − z_point)`, x-separability, and the 4-point billboard quad
      (one y + two top x + two bottom corners).
- [ ] Name the tilt **once** as a shared constant ([forks F4](forks.md#f4)) instead of `65.0` literals
      scattered through the shaders.

## P1 · Audit — find every geometric assumption in the code
- [ ] Enumerate each place that encodes world geometry and record what model it assumes:
      `shadowCover`/`casterCover` (the leaning card + the `0.5·H·cos65` offset), `SHADOW_LIFT`, the
      `#3` zdepth elevation, `placeThing`/`thingPlacement` draw height, light `z`, the lighting pass.
- [ ] Output: a short table (assumption → file → matches-model? ) in [`issues.md`](issues.md). This is
      the "firm understanding" deliverable — no code change yet.

## P2 · Conform the shadow projection to the model
- [ ] Replace the leaning-card placement with the confirmed geometry ([forks F2](forks.md#f2)) and
      re-derive the shadow quad from the **4-point construction**: ONE y calculation, project the
      **top-left/top-right x** (`G.x = light.x + s·(prim.x − light.x)`), and take the **bottom two
      points as the billboard's bottom corners**.
- [ ] VERIFY: shadows still read correctly (shape, direction, penumbra); corridor↔brute **0 mismatches**
      both classes; display cap holds.

## P3 · Retire the compensating fudges
- [ ] `SHADOW_LIFT` should trend to ~0 once the model is right ([forks F3](forks.md#f3)) — it exists to
      seat a shadow whose base was misplaced by the old card. Re-measure the base seating; keep only a
      genuine art dial, if any.
- [ ] VERIFY: shadow bases seat on sprite bases **without** the lift (the `#2` check, re-run).

## P4 · Close + hand off
- [ ] Update the model doc with anything the build taught us; note residuals.
- [ ] Unblock [`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md) — its receiver
      elevation now shares one vertical frame with the caster projection.
