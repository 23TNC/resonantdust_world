# Todo — lighting (execution order)

_Verifiable in-browser (the 3-light rig: red static, green dynamic, blue static). Each phase renders
a visibly different lit world; keep the `/overlayRT` debug channels working alongside. Model +
inputs in [`README.md`](README.md); decisions in [`forks.md`](forks.md)._

## P1 · Emission — lights illuminate the world
- [ ] Lighting pass at the display seam: per pixel, world pos → tile presence → Σ light colour ·
      intensity · falloff(dist, reach); `out = albedo · light` + a small ambient floor.
- [ ] Read lights from the data texture (set 2) by presence index; falloff curve ([forks F2](forks.md#f2)).
- [ ] VERIFY: bright coloured halos around red/green/blue over the flat albedo; green's halo follows
      it; halos blend where they overlap; display-cap holds.

## P2 · Albedo + normal — shaded lighting
- [ ] Add Lambert diffuse from the normal map (`max(0, N·L̂)`), light dir per pixel; normal
      convention ([forks F3](forks.md#f3)).
- [ ] `out = albedo · (ambient + Σ colour·intensity·falloff·diffuse)`.
- [ ] VERIFY: surface relief reads — the side of a sprite facing a light is brighter; correct with
      the moving light (relief re-shades as green orbits).

## P3 · Shadows — lights respect shadow-cold
- [ ] Mask each light's term by `(1 − shadowCoverage_i)` — slot i's u9 coverage from shadow-cold at
      the pixel's unit-texel (presence slot i ↔ shadow slot i).
- [ ] VERIFY: shadows go dark (that light removed there); penumbra/umbra modulate smoothly; the
      moving light's shadows sweep correctly; a pixel lit by two lights keeps the unshadowed light.

## P4 · Polish + close
- [ ] Ambient level, falloff, tonemap/clamp ([forks F4](forks.md#f4)); warm-over-cold composite intact.
- [ ] Perf: the per-pixel light loop (≤14 presence) over the viewport; confirm display-cap.
