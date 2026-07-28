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

## 2026-07-27 · P1 — registry v2 (normal-detail params)

`MaterialParams` (loader.rs) + two new wasm exports (`materialDetailFields`, `materialDetail`
stride-2) + `MaterialDef`/`fromWasm`/`packChannels` (chC vec4 per channel) + `uChC` through the
bake. Verified: wasm rebuilt (dsl visibly recompiled), tsc green, world pixel-unchanged with
all-zero chC (identity).

## 2026-07-27 · P2 — the seed lane

VARIABLES.md FIRST: `billboard_data.B` bits 0–7 = `u8 seed` (of the u14 reserved; A left whole —
F3). Stamped in `billboardDataFor` from the placement's deterministic `cellSeed`, quantised u8;
the bake's `uSeed` now consumes the SAME quantisation (one source). Verified: adjacent same-kind
trees visibly distinct; deterministic by construction (positional hash, no randomness in the path).

## 2026-07-27 · P3 — normal detail (RNM), weight-masked, seeded

Per-channel detail in `MRT_FRAG`: the field's two decorrelated channels as an (x,y) tilt,
amplitude × the channel's LAYER WEIGHT, RNM-reoriented onto the base normal; `oNormal.a` stays 1
(blend-factor rule). Verified: `/overlayRT normal-cold` shows granular canopy structure with
smooth trunks (weight mask working); silhouettes unchanged; amp-0 identity (pre-authoring reload
was pixel-unchanged). Bake GPU cost: 0.0079 → 0.0117 ms/draw with detail (+48 %), i.e. ~0.5
ms/frame ONLY during a budget-capped full-world rebake, zero steady-state.

## 2026-07-27 · P4 — colour placement (F1)

Four modes live: 0 uv / 1 world / 2 detail-field-keyed / 3 normal-keyed, per-channel via chC.w
with the global `__material(mode)` override (bake-side `uPlaceMode`). Default (F1 lean): mode 2
for detail-carrying materials, else the material's authored sampleSpace. A/B set produced at area1
(same trees, same seed): mode 0 broad patches · 2 fine needle-scale clumping coherent with the
relief · 3 variation following lit facets · 1 pattern pinned to world space. The USER's pick is
solicited ([issues I2](issues.md#i2)).

## 2026-07-27 · P5 — the conifer: pineneedle + bark

`needle` appended to `NOISE_FIELDS` + the atlas generator (lattice [9,48], sharpened into short
dashes — [issues I1](issues.md#i1): the plan's `noise_fields.py` doesn't exist; the runtime
generator is the source). Content: `::pineneedle` (needle colour field, calm 14° hue/0.05 chroma
swings, needle detail amp 0.6 scale 3) on the conifer's layer 0; `::bark` (mottle, ZERO hue swing,
mild detail 0.25) on layer 1; `strand` reverted to colour-only for its other users. Verified at
area1: foliage carries needle-grain colour + relief, calmer and finer than the strand test; trunks
keep their brown with mild roughness; adjacent trees distinct; silhouettes unchanged.

## 2026-07-27 · P6 — wrap; stream complete (15/15)

Every phase committed individually (c7bfd11, ee841b9, c3ed083, d1294fc + this). The one
deliberately-open thread: the F1 by-eye verdict + the "less plastic" stream verdict are the
user's, solicited in the report ([issues I2](issues.md#i2)). `bin/rd docs-check` green.
