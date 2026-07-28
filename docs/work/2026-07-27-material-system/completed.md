# Completed — material system

## 2026-07-27 · P0 — the noise atlas lives; the existing material system wakes up

Ported `noiseAtlas.ts` from pixijs to a raw `Uint8Array` → engine `Texture` (rgba8unorm, LINEAR):
same mulberry32 seeds, same wrapping-lattice value noise + 2-octave fbm, same field characters
(`mottle` 6×6 isotropic, `strand` 4×24 streaks) — byte-comparable generation, no canvas. Wiring
fixed en route: `setNoiseAtlas(texture, rows = 1)` was replaced by `Viewport.buildNoiseAtlas()`
(the viewport owns the GL context) which passes the REAL row count (`NOISE_FIELDS.length` — the
old default of 1 would have folded every field onto row 0) and binds BOTH tiers (warm included,
for future mover materials).

**Verified at area1:** the conifers' authored `strand` material — inert since the webgl port —
came alive on reload: every tree shows green hue/chroma foliage variation, adjacent trees
DISTINCT (the `cellSeed` instance offset working), trunks unchanged (tint-only channel), ground/
bushes unchanged (delta-form identity for unbound channels — mathematical, plus visual check).
A/B pair = any earlier session screenshot (stub build) vs this reload. tsc green. NOTE: the
variation reads strong/saturated — those are the authored swings showing for the first time;
tuning is content and lands with P5's conifer material rework.
