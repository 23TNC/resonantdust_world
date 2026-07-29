# Issues — marigold-linked-normals

_Defects found during execution land here. Known inputs: the marigold venv must exist
(`bin/marigold build`; `bin/marigold config` reports torch/CUDA state); Marigold is
starved on flat art (the README's engine note) — expect weak signal on featureless cells;
the atlas.json inset (`GRID_INSET_FRAC`, default 0) and the DSL internal_padding are
DIFFERENT values (R5 of texture-generalization) — the checker must slice by the same cell
geometry the client samples._

## I1 · Per-cell inference STARVES the relief (user-reported post-delivery; FIXED 2026-07-29)

The shipped P1–P4 atlas lost the detail that makes walls present as walls. MEASURED: the
whole-atlas ORIG carried 23.4 % of pixels >15° from each cell's OWN flat frame (real
relief); the per-cell inference output carried 1.6 % — small replicated-edge crops read as
near-flat infinite surfaces, exactly Marigold's documented flat-art starvation. TWO metric
lessons banked into `atlas_check.py`: (1) a RELIEF metric measured against the cell's OWN
flat frame (vs +Z counts global tilt as detail — ORIG read "33 %" of which ~10° was tilt);
(2) frame-consistency numbers can look PERFECT on flattened output — consistency and
detail must both gate. PIVOT (all landed): `atlas_normals.py --engine whole` (default) =
ONE inference over the full atlas (keeps relief), the post passes fix its frame drift;
arm AVERAGING → donor STAMPING (the pure-run cells N|S / E|W donate; members carry the
donor's FULL detail, spread 0 by construction); the bevel gain is BOOST-ONLY (shrinking
strong cells toward the median discarded relief). SHIPPED: relief 23.9 % (ORIG 23.4 %),
flat 0.56°, arms 0.00°, seams E|W 1.70°/S|N 0.70° mean=max (baseline 8.31°/17.89°).
In-game at both zooms: bevels and faces read as walls again, runs stay continuous.
