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
