# Todo — lighting (execution order)

_Verifiable in-browser (the 3-light rig: red static, green dynamic, blue static). Each phase renders
a visibly different lit world; keep the `/overlayRT` debug channels working alongside. Model +
inputs in [`README.md`](README.md); decisions in [`forks.md`](forks.md)._

## P1 · Emission — bake the lightmap, composite into albedo
- [ ] A **lightmap RT** (resolution per [forks F5](forks.md#f5)) baked from the lights: per texel,
      world pos → tile presence → `ambient + Σ colour·intensity·falloff(dist, reach)`. Dirty-driven
      (reuse the shadow dirty rects) so only changed regions re-bake.
- [ ] Display composite = `albedo × lightmap` (the `AlbedoBlitShader` draw; warm-over-cold intact).
- [ ] Read lights from the data texture (set 2) by presence index; falloff ([forks F2](forks.md#f2)).
- [ ] VERIFY: bright coloured halos around red/green/blue over the albedo; green's halo follows it;
      halos blend on overlap; only dirty regions re-bake; display-cap holds.

## P2 · Albedo + normal — Lambert diffuse into the lightmap
- [ ] Fold per-light Lambert `max(0, N·L̂)` into the lightmap accumulation (before the sum). Settle
      the resolution/normal split ([forks F5](forks.md#f5)) + the normal convention ([forks F3](forks.md#f3)).
- [ ] VERIFY: surface relief reads — the lit side of a sprite is brighter; re-shades as green orbits.

## P3 · Shadows — mask each light in the lightmap
- [ ] Multiply each light's lightmap term by `(1 − shadowCoverage_i)` — slot i's u9 coverage from
      shadow-cold at the texel's unit position (presence slot i ↔ shadow slot i).
- [ ] VERIFY: shadows go dark (that light removed there); penumbra/umbra modulate smoothly; the
      moving light's shadows sweep correctly; a pixel lit by two lights keeps the unshadowed light.

## P4 · Polish + close
- [ ] Ambient level, falloff, tonemap/clamp ([forks F4](forks.md#f4)); warm-over-cold composite intact.
- [ ] Perf: the per-pixel light loop (≤14 presence) over the viewport; confirm display-cap.
