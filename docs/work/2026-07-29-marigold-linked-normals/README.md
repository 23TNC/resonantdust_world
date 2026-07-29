# marigold-linked-normals — consistent normal frames across a linked atlas's cells

## What

Marigold normal generation for LINKED (grid) atlases is close but inconsistent: the model
predicts view-space normals over the WHOLE atlas image as if it were one scene, so each of
the 16 autotile cells lands in a slightly different normal frame — flat wall tops that
should all encode the identical up vector drift cell-to-cell, equivalent geometry (the same
run/corner piece appearing in several cells) gets different slopes, and edges that abut
in-world under autotiling don't carry matching normals across the seam. On the lit walls
(texture-generalization P4 — `tileNormal` samples these cells per world tile) the drift
reads as per-tile shading jumps inside what should be one continuous wall.

## Why now

The consumer just landed: walls light through their def's normal via the cold path, and the
in-shader D1 autotile picks cells per world tile — so cell-to-cell normal inconsistency is
now a VISIBLE lighting artifact, not a latent art quirk.

## Current facts (verified in tree)

- `bin/art normal --marigold <kind>` → `bin/marigold normals` → `marigold/normals.py`
  (MarigoldNormalsPipeline, view-space, OpenGL +Y-up encoding, #8080FF fill outside the
  silhouette). The WHOLE diffuse atlas is one inference input.
- Grid kinds: `GRID_CATS="linked"`, 4×4 derived cell size (`bin/art` ~l.180); grid tiles
  never pitch (`_resolve_tilt` — their normals are already the +Z-up ground frame).
- The 16-cell adjacency semantics live in `client/webgl/src/game/world/linkedCell.ts`
  (D1: x = N+2E, y = 3−(S+2W)) — it defines which cell EDGES must agree in-world.
- The wall normal master today: R/G σ ≈ 52/57, B mean ≈ 221 (real directional content).

## Design stance

**Keep Marigold, fix the frame.** The per-pixel detail is good ("we are very close");
what's missing is a deterministic post-alignment that puts every cell in ONE canonical
frame and makes in-world-adjacent edges agree exactly. Composition-from-canonical-pieces
(generating one master piece set and assembling all 16 cells) is the FALLBACK if alignment
cannot converge — it guarantees consistency by construction but discards Marigold's
per-cell detail (recorded as fork F1).

The pipeline this stream builds, all inside the `marigold/` toolset so `bin/art normal
--marigold` stays the one entry point:

1. **Measure first** — `marigold/atlas_check.py`: per-cell flat-region mean normal (vs the
   canonical up), same-piece cross-cell deviation, and in-world seam continuity from the
   D1 adjacency table. These numbers are the acceptance oracle for every later stage.
2. **Inference hygiene** — per-cell crops with replicated-edge padding (a cell must never
   see its atlas neighbour — atlas order is NOT world adjacency), pinned seed, ensembling,
   one pipeline load for the batch; reassemble.
3. **Global frame alignment** — per cell: find the flat-top region (dominant normal
   cluster), compute the rotation taking its mean to +Z, apply to the whole cell;
   then per-channel gain alignment of bevel slopes against the reference cell.
4. **Symmetry + seam enforcement** — average equivalent pieces across cells under their
   mirror/rotation transforms (transforming the VECTORS, not just the pixels), then blend
   in-world-adjacent edge strips to exact agreement.
5. **Regenerate + drill** — the wall atlas re-exported, served, and eyeballed lit in-game
   at ≥2 zooms beside a torch. The user's eyes are the final oracle.

## Forks pre-resolved

- **F1 — align vs compose.** Chosen: post-alignment of Marigold output (preserves detail,
  matches "very close"). Fallback: canonical-piece composition if P3's numbers won't
  converge below the P0-set thresholds. Revisit only with the measurements in hand.
- **F2 — where the code lives.** Chosen: `marigold/atlas_normals.py` (+ `atlas_check.py`)
  in the existing venv-backed toolset, invoked from `bin/art`'s marigold branch when the
  kind is a grid kind — NOT a standalone script; the leaf's engine stamp keeps recording
  `marigold`.

## Out of scope

Laigter-engine grid handling (its own tileable setting is GUI-only — pre-existing);
non-grid sprite normals (unchanged); depth/`normal_depth` integration for grid kinds.
