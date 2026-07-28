# material-system — per-layer materials: colour variation + normal detail

_Work stream, opened 2026-07-27. Components: `client/webgl` (`material.ts`, `noiseAtlas.ts`,
`mrtBakeShader.ts`), `content/material/`, `shared/dsl` (material defs), `docs/VARIABLES.md`
(the seed lane). User's brief: per-layer materials, solid-white default, albedo colour variation,
textured normals to fix the plastic look of GENERATED normals, per-object seed in billboard data,
first application = conifer pine needles on layer 0 (the maps' red channel)._

## What exists, verified 2026-07-27 (build on it, don't re-invent)

- **The material registry is real and half-alive**: `<material>` DSL defs
  (`content/material/materials.rd`) → wasm → `material.ts` registry; per-channel bindings
  (`&thing.packed.<i>.material` / `.tint`); OKLab hue/chroma jitter in the bake (NEVER lightness —
  it must read as pigment, not light); delta-form identity (unbound channel ⇒ exactly 0).
- **It is currently INERT in webgl**: `noiseAtlas.ts` is the W4d stub returning `null`, so the
  jitter samples the 0.5 fallback everywhere. The pixijs generator (104 lines, fields from
  `bin/lib/noise_fields.py`: `mottle`, `strand`) is the port source. P0, before anything else.
- **"Three layers" = the `layers` map's RGB** (`PACKED_CHANNELS = 3`) — a material binds per
  channel; the channel's weight map says WHERE it applies. This is the same mechanism the needle
  detail rides: layer 0 (R) is the conifer's foliage weight.
- **Seed**: `cellSeed(tx, ty)` already derives a stable per-cell seed (never swims across
  reloads); the bake takes it as a CPU uniform. What's missing is the DURABLE lane the user asked
  for: `billboard_data` has `u14 reserved` in B and all of A free ([F3](forks.md#f3)) —
  VARIABLES.md is authoritative and gets the lane FIRST.

## Design stance

- **A material = per-channel params for TWO effects**: albedo jitter (exists) + **normal
  detail** (new — a tiling detail field rotated onto the base normal, amplitude per material,
  weighted by the channel's layer weight so needles perturb foliage and never trunk). The
  generated-normal problem is smoothness; detail restores the high-frequency structure Laigter
  can't infer. Blend method is [F2](forks.md#f2).
- **Solid white is the default and identity is the contract**: no material bound ⇒ bit-identical
  bake output. The delta form already guarantees this for colour; normal detail must hold the
  same property (amplitude 0 ⇒ base normal untouched).
- **Where the colour goes is the user's open question** ("could flow along the normal maybe?") —
  held as an OPEN fork ([F1](forks.md#f1)) with candidate modes built behind a live switch and
  decided BY EYE at area1: UV-space noise, world-space noise (both exist as `sampleSpace`),
  **detail-field-keyed** (colour rides the SAME field that carves the relief — clumps cohere),
  and **normal-keyed** (hue varies with facet direction). Lean: detail-field-keyed, because
  colour and relief sharing one cause is what makes needle clumps read as clumps.
- **Everything lands at BAKE time.** The whole system rides the budgeted MRT bake — zero
  per-frame display cost, and re-variation only on re-bake. The one blend-alpha rule from
  lighting-feel F3 applies: attachment alpha is a BLEND FACTOR; detail writes must keep A = 1.
- **Seed is deterministic, never random**: the lane is stamped from `cellSeed` (world-cell hash),
  so two adjacent trees differ but each is fixed to where it stands, forever.

## Acceptance (stream-level)

Conifers at area1, A/B against the pre-stream build: needle texture visible on foliage (normals
overlay shows high-frequency structure where layer-0 weight is high; silhouette unchanged), colour
variation across adjacent trees (distinct, stable across reloads), trunk unaffected, and the
user's verdict that the trees read less plastic. Identity: with no material bound the bake is
bit-identical. Bake cost stays within the existing budget (GPU-timed).
