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
