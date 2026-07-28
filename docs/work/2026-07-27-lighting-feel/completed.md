# Completed — lighting feel

## 2026-07-27 · P0 — audits: AO is EMPTY, emissive partially exists

AO: surface-G is exactly 255 everywhere across all 42 disk leaves (conifer/flora/wolf) — full
table in [issues I1](issues.md#i1). Consuming it now would be a no-op; P3 routes through the
`bin/art` bake first. Emissive: 12 real leaves exist, all wolf glowing-eye variants
([I2](issues.md#i2)) — the art path exists, the torch flame just isn't authored yet.

## 2026-07-27 · P1 — colour temperature + specular glints

`accumulateLights` gains: (a) temperature — light colour lerps from a desaturated rim
(`mix(lum, col, uRimSat)`) to a warmed core (`col·(1+w, 1, 1−w)`) by the existing falloff value;
(b) Blinn N·H glints on things (world-frame view vector hoisted per fragment), riding the same
falloff + shadow inside the per-light exactness clamp. Live hooks `__lighttemp(warm, sat)` /
`__glint(str, pow)`; identity values (0,1)/(0) are the A/B off-switch and both force a rebake.
Defaults 0.12/0.65 and 0.25/16.

**Verified at area1** (torch cluster, penumbra experiment stashed first — shadows have their
shapes back): A/B screenshots recorded — B is visibly warmer gold at torch cores with muted rims;
`__glint(2, 4)` exaggeration proves the specular term renders (strong sheen), then defaults
restored. **Timing:** 2× runs each way on the 3-moving-lights orbit — identity lightCold
0.233/0.246 ms vs features 0.238–0.300 ms, while the UNCHANGED gather varied 0.72–0.87 across the
same runs — the feature cost (≲0.04 ms under 3-light motion) is within fixture noise, and exactly
0 on static frames (bake-once). tsc green.

## 2026-07-27 · P2 — the decay lightmap: torches flicker, the gather never runs

Landed across `gl/` + `shadowGather.ts` + the blit + the content stack:
- `rgba16float` TexFormat + a `mulConstant` blend mode (`blendFunc(ZERO, CONSTANT_COLOR)` +
  `blendColor`) — the in-place `dst *= k` fade, one trivial draw, no ping-pong (F2).
- The coarse decay RT (shadow-RT geometry, RGBA16F) + `decayAndSplat` in `tick`: dt-derived
  `k = exp(−dt/τ)` fade, then one jittered splat per flickering light per frame, deposit scaled by
  `dt·60` so equilibrium brightness is FRAME-RATE INDEPENDENT (caught live: per-frame emission vs
  per-second decay inflated a pumped run 5×; the τ/str defaults were then derived from the
  equilibrium algebra — mean ≈ rate·E[i]·τ, fluctuation ∼ 1/√(rate·τ) → τ 0.22, str 0.05).
- Shadow-STAMPED splats (F4): the splat shader finds the parent light's presence slot and
  multiplies by (1 − its u7 shadow coverage) — minimal helper block, NOT GATHER_COMMON (shader
  compile time is a known load hazard).
- Blit: `decaySample` — MANUAL 4-tap bilinear whose taps each fold through the toroidal window
  independently (hardware LINEAR would blend across the wrap seam), added inside the display clamp.
- Content: `&thing.light.flicker` → loader.rs flags bit 2 → wasm rebuilt (dsl visibly recompiled)
  → `PrimitiveLight.flicker` → carriedLights mirror (extended to carry colour/intensity/hot/
  flicker). Both torch kinds author `flicker 1`. Hooks: `__flicker(on, tau, str)`, `__flickerall`.

**Verified at area1** (3 content torches, all `flicker: true` from content — the flag flowed the
whole stack):
- Splats emit (2/frame in-window) with **`debugClassDraws` 0 on every frame** — flicker never
  touches the accumulators; the stream's flagship acceptance.
- **Stamp, quantitatively:** decay readback along west→east through a torch: 0.058/0.299/0.436/
  0.259/**0** — the profile dies inside the tree's east cast shadow while extending 2× further
  west. Visually the glow hugs the shadow wedge.
- **Decay stage cost: 0.0354 ms/frame** GPU (fade + splats, 240-frame timer) — under the ≤0.05
  acceptance.
- **Empty-map identity:** flicker off → max residual 1.6e−6 after 2.5 s (sub-1-LSB of 8-bit
  display ⇒ pixel-identical; f16 flush reaches exact 0 asymptotically).
- **Zoom sweep** 1→0.5→0.25→1 with flicker live: no GL errors, splats resume, no ghosts.
- Screenshots recorded: warm breathing pools around all three torches.

One probe lesson recorded for future sessions: `win.slotPx` says SQUARE is **64** in the live
build — a probe hardcoding 128 chased a phantom coordinate bug; the code itself imports the
constant and was right throughout.
