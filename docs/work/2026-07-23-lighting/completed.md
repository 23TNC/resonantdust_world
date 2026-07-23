# Completed — lighting

_Delivered + browser-verified. Newest first. Mechanisms in [`README.md`](README.md); decisions in
[`forks.md`](forks.md)._

---

## P1–P4 — the baked lightmap, delivered 2026-07-23

The world is lit from the 3 debug lights (red static / green dynamic / blue static) via a **baked
LIGHTMAP** ([F1](forks.md#f1)); the display composites `albedo × lightmap`. All in `client/webgl`
(`game/viewport/shadowGather.ts` bakes it, `albedoBlitShader.ts` composites, `Viewport.ts` wires it).
Held **120 fps (display cap)** throughout — 3 lights, 1 moving, per-px relief, soft shadows.

- **P1 · Emission** — a lightmap RT (world-space toroidal, `rgba8unorm`, `cols·TEXTILE_UNIT ×
  rows·TEXTILE_UNIT`, **TEXEL-aligned with shadow-cold** — same fc→world mapping, same `uDirty` gate →
  clean tiles persist), baked in `ShadowGather` right after the shadow gather. Per texel: presence →
  light record → `ambient + Σ colour·intensity·falloff`. The blit samples it by world position (window
  mapping via `SquareCache.window` + `TEXTILE_UNIT`) and multiplies. `uLightEnable 0` = the old UNLIT
  blit verbatim (fallback). Falloff = `smoothstep(reach,0,dist)` ([F2](forks.md#f2)).
- **P2 · Normal relief** — MRT: att1 = the irradiance-weighted mean horizontal lit-from direction. The
  blit applies the HIGH-freq normal PER-PX: `relief = 1 + strength·dot(N.xy_world, ldir)`, flat-neutral
  ([F3](forks.md#f3), [F5](forks.md#f5)). Reads as dimensional foliage shading; flat ground stays clean.
  `__relief(n)` tunes live (default 1.1).
- **P3 · Shadows** — the lighting pass reads shadow-cold at the aligned texel (presence slot i ↔ shadow
  slot i) and masks each light by `(1 − u9 coverage)` — before both the irradiance sum AND the relief
  drive. Soft radial shadows from each light, penumbra/umbra modulating, grounded at caster bases.
- **P4 · Polish** — F4 = LDR clamp (stylised, [F4](forks.md#f4)); ambient floor 0.12; warm (mover) tier
  lit + shadowed through the same blit path (warm-mixed albedo/surface/normal). Only the dynamic light
  re-bakes per frame (~600–730 tiles, its reach box); static lights + their shadows persist.

**Verification:** three coloured lights emitting with smooth falloff over the albedo (sand/trees read
through); per-px relief on sprites, flat ground neutral; soft radial per-light shadows; the green light
re-shades as it orbits; 120 fps; no console/GLSL/framebuffer errors.
