# Todo — world-space lighting (execution order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md). Verify in-browser on the
debug light rig; A/B world-space vs the current screen-space path at every step (a toggle, e.g.
`__worldlight`). Freeze the subject light + a zero-reach keep-alive so the change-gated loop keeps ticking._

## P0 · Derive + write the world-space lighting model (NO shader code yet)
- [ ] From the ratified `z = sin65·Δ` geometry ([world-geometry](../2026-07-23-world-geometry/README.md)),
      derive the **true world-3D light→point vector** for: (a) a **ground** point, (b) a **billboard** point
      at fictional elevation `z`. Pin the **N–S foreshorten factor** (is it `cos65`? `sin65`? tied to the
      height model?) and how the light **height** `Z` enters. Ground it: 1:1 on E–W, foreshortened on N–S,
      `Z` from the light record.
- [ ] Write it down as a short **model** (a `model.md` here, or fold into world-geometry) — the single source
      of truth, like `map-model.md` was for the maps.
- [ ] **Confirm against the user's simulation** (the same check that settled world-geometry) BEFORE coding —
      pick a light + a few points, compare the derived vector/oval to the simulation. Do not proceed on an
      un-verified factor (world-geometry I-3/I-5).

## P1 · Elliptical falloff
- [ ] Replace the screen-radius `dist = length(Lxy − P)` in `LIGHT_FRAG` with the **true 3D distance** from
      P0 (N–S un-foreshortened + light `Z`). Behind the `__worldlight` toggle.
- [ ] VERIFY: the light's reach reads as an **oval** (flattened N–S), not a circle; the ground gradient looks
      natural; A/B toggles cleanly back to the old circle. Confirm at ≥2 zooms (foreshorten is zoom-invariant
      in world units, so it must NOT change with zoom).

## P2 · World-space light direction for the normal N·L
- [ ] Feed the P0 **3D light direction** into the relief instead of the screen-space `toL/dist`, dotting it
      against the world-frame baked normal ([normal-tilt](../2026-07-24-normal-tilt/README.md)). Aggregate
      dir map (`oDir`) becomes a world-space direction.
- [ ] VERIFY: relief shades correctly across the tilt; a standing billboard turned **away** from a light
      darkens (the macro front/back term falls out); no seam as a prim crosses the light's latitude.

## P3 · Unify + close
- [ ] One world-3D light vector computed once per light per texel, feeding BOTH falloff (length) and N·L
      (direction). Confirm the bespoke `prim.y > light.y` darkening idea is **subsumed** (not needed).
- [ ] Decide the relief form ([forks F4](forks.md#f4)): keep additive `1 + gain·dot` or switch to clamped
      Lambert now that `N·L` is world-correct (addresses the user's bright-spots-in-shadow worry).
- [ ] Retire or keep the screen-space path (`__worldlight` default on). Perf holds (a handful of extra ALU
      per light per texel; no new textures). Docs + memory; note the shadow gather was untouched.
</content>
