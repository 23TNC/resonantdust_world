# comfy-linked-tiling — seam-inpaint the linked atlas SOURCE so cells truly tile

## What

Make the linked wall atlas's DIFFUSE art itself tile-consistent by feeding it through
ComfyUI for a slight, seam-targeted modification (user, 2026-07-29: "feed it through
comfyui to get it to modify it slightly so it tiles properly"). The normal-side stream
([marigold-linked-normals](../2026-07-29-marigold-linked-normals/README.md)) made the
NORMALS consistent, but it compensates for per-cell variation in the source art: the run
arm in one cell isn't the arm in another, and in-world-abutting edges don't quite continue.
That leaves (a) visible albedo seams the normal work can't touch, and (b) a subtle divorce —
stamped normals say "identical run" while the albedo still varies per cell, so shading and
texture can disagree. Fixing the SOURCE makes the whole downstream chain coherent for free.

## Design stance

**Inpaint on assembled in-world LAYOUTS, never on the raw atlas.** Atlas order is not world
adjacency (the lesson both prior streams hit). The tool composites cells the way the D1
table actually places them — a horizontal run, a vertical run, each corner/T/cross in
context — masks the SEAM BANDS, and runs low-denoise masked img2img so the model blends the
joints players actually see; then slices the cells back out.

**Modify slightly = masked + low denoise.** Only seam-band pixels may change (outside the
mask stays bit-identical by construction of masked inpainting); denoise ~0.3–0.45 preserves
cell identity.

**The one hard constraint — canonical edges.** A cell edge appears in MANY pairings (any
E-cell can abut any W-cell), so a seam fixed in one layout must hold in all. Edge bands are
FROZEN once finalized: layouts process in dependency order, each inpainting only
not-yet-frozen band sides; a second round runs only if the metric hasn't stabilised.

**Numbers gate, eyes ratify.** `marigold/atlas_check.py` (relief + normal seams) already
exists; this stream adds the ALBEDO seam section so the ComfyUI pass gates itself the same
way. After the source is fixed, the normal pipeline re-runs on it — and if source
consistency makes arm STAMPING unnecessary, it is weakened to recover per-cell character
(that trade was I1's compromise, not a goal).

## Current facts (verified in tree)

- ComfyUI client plumbing: `bin/lib/generate.py` (`COMFYUI_URL`, default
  `http://172.16.10.10:8188`; graph submit `/prompt`, image upload, `/view` fetch,
  KSampler i2i with `--dn` denoise). `generate_tile.py` = the terrain-sheet precedent
  (and records that the box's one circular-padding node segfaults — irrelevant here,
  we inpaint seams, not wrap planes).
- The D1 cell semantics + geometry: `marigold/atlas_check.py::cell_bits` and the window
  math (512² atlas, 128px cells, 112px client-sampled window, 8px internal pad).
- The wall masters: `textures/biome-tile/default/smooth/wall/{diffuse,albedo,layers,
  normal,surface}.l.0.png` + `atlas.json`; downstream maps regenerate via `bin/art`
  (delight / split_layers / normal --marigold / surface).

## Forks pre-resolved

- **F1 — where the seam truth lives.** Chosen: canonical FROZEN edge bands enforced by the
  slicer (deterministic), with the diffusion model only asked to blend toward them.
  Rejected: iterate-until-agreement across layouts (non-deterministic convergence).
- **F2 — code placement.** `bin/lib/retile_linked.py` behind `bin/art retile-linked
  <kind>` — it is an ART-pipeline pass (peer of generate-tile/split_layers), not a
  marigold tool; it imports the ComfyUI client helpers from `generate.py` and the cell
  geometry from `marigold/atlas_check.py`.
- **F3 — which maps get inpainted.** The DIFFUSE only; every other map REGENERATES from it
  through the existing chain (delight → split_layers → marigold normals → surface).
  Inpainting derived maps independently would desynchronise them.

## Out of scope

New wall kinds / brick / plank (the tool generalizes; this stream proves it on smooth);
ground-sheet tiling (generate_tile's modulo scheme, already solved); template-library
sprite-gen work.
