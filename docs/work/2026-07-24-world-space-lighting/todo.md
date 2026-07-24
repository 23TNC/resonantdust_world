# Todo — world-space lighting (execution order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md). Verify in-browser on the
debug light rig; A/B world-space vs the current screen-space path at every step (a toggle, e.g.
`__worldlight`). Freeze the subject light + a zero-reach keep-alive so the change-gated loop keeps ticking._

## P0 · Derive + write the world-space lighting model (NO shader code yet) — DONE
- [x] Derived the true world-3D light→point vector + the N–S factor → [`model.md`](model.md). Result:
      **true N–S = screen Δy / cos65** (E–W 1:1; light height `Lz` adds an up-term). The `sin65` (fictional
      height) tension is flagged — high confidence on `cos65` but NOT sim-verified, so it's live-tunable.
- [x] **Factor RESOLVED — `1/cos65` by derivation** (the `Z·sin65` cross-terms cancel; [`model.md`](model.md)).
      `sin65` is the height coefficient, never a falloff competitor. The only remaining eyeball is whether the
      art is actually 65° — a `__tilt(deg)` sweep, and it does NOT gate P2. B-1 downgraded.
- [x] **Live angle dial (`__tilt(deg)`)** — the world tilt now lives in the DATA MAP constants (centidegrees,
      A reserve; [`VARIABLES.md`](../../VARIABLES.md)), read by the shadow projection with no burned uniform;
      the shadow tilt was a compile-time literal before. `__tilt(deg)` re-tilts the whole geometry live
      (shadow cos/sin + elevation `sin` + falloff `1/cos` + caster lean). Makes the factor A/B and any
      "what if the world were 60°/70°" look-test a live dial instead of a recompile.

## P1 · Elliptical falloff — IMPLEMENTED (behind `__worldlight`, default on)
- [x] `LIGHT_FRAG`: `dist = sqrt(Δx² + (Δy·uNsInv)² + Lz²)` (ground `Pz=0`) under `uWorldLight`; screen circle
      when off. `uNsInv` = `__nsfactor` (default 1/cos65 ≈ 2.366). TS clean.
- [ ] VERIFY (user, browser was disconnected on my end): reach reads as an **oval** flattened N–S; ground
      gradient natural; `__worldlight(false)` A/Bs back to the circle; **oval shape identical across zooms**
      (foreshorten is a world property — [issues.md#i3](issues.md#i3)). Tune `__nsfactor` to taste; that
      value settles [forks F2](forks.md#f2).

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
