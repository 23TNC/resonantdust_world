# analytic-wall-normals — exact normals for regular walls; the exact one seeds the rest

## What

Generate the smooth wall's linked normal atlas MATHEMATICALLY (user, 2026-07-29: "this
first set is smooth with predictable geometry, so we should be able to handle this set
mathematically"), replacing learned inference for regular geometry — then use that first
CORRECT normal as ControlNet conditioning to generate normals for the irregular wall kinds
(brick, plank, metal, flecked — all already have masters).

## Why this is the right call (assessment, 2026-07-29)

The whole day's numbers argue for it. Learned inference on this art is fundamentally
noisy: run-to-run variance alone moved relief ±4 pts on IDENTICAL input; processing
resolution off the model's trained 768 collapses relief to ~0 %; edge-to-edge input reads
as an infinite plane; and every consistency defect (frame tilt, piece drift, seam jumps)
existed only because a MODEL guessed at what is actually a beveled prism whose true normal
map is exactly computable. Analytic construction makes every atlas_check consistency
metric ZERO BY CONSTRUCTION — no alignment passes, no stamping, no seed pinning — and the
relief is whatever the profile says, deterministically. The Marigold+alignment pipeline
(the two prior streams) remains the path for kinds with genuinely unpredictable geometry;
for regular walls it was solving the wrong problem well.

## The model

A top-down wall is a PRISM with a beveled top: a height field `H(x, y) = profile(d)` where
`d` is the distance into the wall from its drawn boundary and `profile` is the wall's
cross-section (outline → face → bevel → flat top), swept along each cell's connectivity
shape (arms from the D1 bits + the hub). Normals fall out as the height-field gradient,
encoded in the corpus convention (view-space, +Y-up, #8080FF flat). Because every cell
sweeps THE SAME profile over shapes that agree at window edges, cells tile exactly —
seams, piece identity, and frame are perfect without any post pass. The profile
PARAMETERS are fitted once against the drawn art (the analytic bevel must sit where the
art draws its bevel — registration is measured, not assumed).

## The ControlNet leg (exploratory, bounded)

The box already has `sdxl/controlnet-union-promax.safetensors` (union CN — depth/normal
modes among others). Recipe to spike: condition on the ANALYTIC normal (the macro
geometry every wall shares) + the target kind's diffuse, generate that kind's normal —
learned surface detail on top of correct macro shape. Fallbacks if union-CN normal
conditioning underwhelms: depth-mode conditioning from the analytic HEIGHT field (free
by-product), or img2img with the analytic normal as the base at moderate denoise. The
spike is one phase with a hard acceptance (atlas_check consistency + relief ≥ the smooth
baseline on ONE kind — brick); productionising all kinds is a follow-on stream.

## Current facts (verified in tree)

- Wall kinds with masters: smooth, brick, plank, metal, flecked (+ blueprint, the derived
  sibling). The smooth diffuse is seam-fixed (comfy-linked-tiling P2).
- The oracle: `marigold/atlas_check.py` — relief (tilt-immune), flat frame, piece spread,
  normal + albedo seams. Gates everything here too.
- Engine dispatch: `bin/art normal` records `normal_engine` per leaf and routes
  `--marigold` grid kinds through `atlas_normals.py`; `--analytic` joins that dispatch.
- Geometry helpers: D1 bits/window math in `atlas_check.py`; layout machinery in
  `bin/lib/retile_linked.py`.
- ComfyUI client: `bin/lib/generate.py` (checkpoint, CN loader plumbing, upload/run).

## Forks pre-resolved

- **F1 — shape source.** The wall SHAPE per cell derives from the D1 connectivity +
  profile parameters (analytic), NOT from tracing the art's alpha (noisy, and the art is
  full-bleed). Registration with the art is enforced by the P0 fit + register check
  instead.
- **F2 — placement.** `marigold/atlas_geometry.py` (pure numpy, no venv) beside the
  oracle it must agree with; `bin/art normal --analytic <kind>` dispatches to it and
  stamps `normal_engine=analytic`.
- **F3 — the height field ships too.** `H` is computed anyway; emit it as the leaf's
  depth/AO input (`occlusion` via the existing surface pack) ONLY if free — else defer.

## Out of scope

Productionising ControlNet across all wall kinds (follow-on); non-wall linked kinds;
replacing Marigold for standing sprites (it remains the right tool there).
