# Completed — marigold-linked-normals

_Dated entries, appended as items land: what landed and how it was verified. P0's baseline
numbers live here — every later stage compares against them._

## 2026-07-29 · P0 — the consistency oracle (3/3)

`marigold/atlas_check.py` (numpy/PIL, no venv needed): slices the atlas by the SAME
window the client samples (cell − 2·internal_padding; 512² → 128px cells, 112px window,
8px pad), masks by diffuse alpha, decodes OpenGL +Y-up normals. Three sections — FLAT
(iterated-trim dominant cluster per cell vs +Z, plus the spread vs the atlas's own mean
frame), PIECE (D1-derived arm regions, spread about the cross-cell mean, worst pair),
SEAM (every valid in-world adjacency: 64 E|W + 64 S|N strip pairs). `--json` for gates.

**BASELINE (smooth wall, current Marigold atlas):**
- FLAT: mean 10.62° off +Z, worst cell 8 at 13.35° — but frame spread only 1.16°:
  the error is ONE SHARED TILT of the whole atlas, not per-cell chaos. P2's rotation
  fixes most of this in one move.
- PIECE: N 1.45° / S 1.23° (fine) vs E 5.04° / W 6.44° / hub 8.81° — the
  inconsistency lives in E/W-running geometry; worst pair hub 3 vs 14 at 18.40°.
- SEAM: S|N mean 2.26° max 4.81° (fine) vs **E|W mean 8.31° max 17.89°** (worst pair
  14|1) — the horizontal-run per-tile jump the user sees, now a number.

## 2026-07-29 · P1–P4 — per-cell inference, alignment, symmetry, shipped (9/9) — STREAM COMPLETE

**P1 — per-cell inference** (`marigold/atlas_normals.py`, `bin/marigold atlas-normals`):
slices the diffuse atlas by the atlas.json grid, pads each cell with REPLICATED edges
(context without ever showing the model its atlas neighbour — and replication depicts
exactly what's true in-world for run arms), one model load, a fresh generator with the
SAME pinned seed per cell, flat #8080FF fill outside the silhouette (normals.py's
conventions). `--post-only` re-runs just the post passes for cheap A/B iteration.
`bin/art normal --marigold` routes `.l.` diffuse leaves through it (`--align
--symmetrize`); non-linked diffuses still collect into the whole-sprite normals.py call —
code-reviewed, deliberately NOT exercised live (a live run would overwrite art-128's
in-flight sprite masters). NUMBERS (P1 alone vs baseline): flat mean 10.62°→5.60°, hub
8.81°→2.43°, E 5.04°→2.77°, W 6.44°→2.37°, E|W seam mean 8.31°→3.49° (max 17.89°→7.48°).

**P2 — frame alignment** (`marigold/atlas_align.py::align_atlas`): per-cell Rodrigues
rotation taking the window-estimated flat-cluster mean exactly onto +Z (applied to the
whole cell), then a gentle bevel-gain match toward the atlas median over non-flat pixels
(clamped ±25 %). NUMBERS: flat mean 5.60°→0.17° (worst 0.29°), E 2.77°→1.32°,
W 2.37°→1.22°, hub 2.43°→0.88°, E|W seam 3.49°→1.50°.

**P3 — symmetry + seams** (`symmetrize_atlas`): N and S arms averaged independently
across their 8 member cells; E and W unified under the mirror transform (flip columns AND
negate normal-x — vector-aware); hubs NOT averaged (each connectivity class is different
art — recorded rationale). Seams: one canonical cross-section per axis (E|W, S|N), every
member edge set to it and feathered ~half a unit inward. NUMBERS: arms 0.00° spread;
seams E|W 0.23°/0.23° and S|N 0.16°/0.16° mean/max across all 128 valid pairs (baseline
8.31°/17.89°).

**P4 — shipped + drilled**: `bin/art normal --marigold biome-tile/default/smooth/wall`
ran the whole path and overwrote the master (md5 08744ec9… → 29967e7b…; the old atlas
backed up in the session scratchpad); the edge re-derives on mtime and the client
revalidates by hash. FINAL shipped numbers = the P3 numbers (re-checked on the master).
IN-GAME at zoom 2 + zoom 1: the wall compounds read as CONTINUOUS surfaces — the
per-tile shading jumps and hard lighting seams from the pre-stream captures are gone;
the torch-side rooms glow coherently. THE USER'S EYES ARE THE FINAL ORACLE — one watch
item for them: arm averaging homogenizes run detail by design (F1's trade); if the runs
now read too uniform, the dial is averaging weight, not a return to whole-atlas inference.
