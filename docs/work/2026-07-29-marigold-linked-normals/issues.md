# Issues — marigold-linked-normals

_Defects found during execution land here. Known inputs: the marigold venv must exist
(`bin/marigold build`; `bin/marigold config` reports torch/CUDA state); Marigold is
starved on flat art (the README's engine note) — expect weak signal on featureless cells;
the atlas.json inset (`GRID_INSET_FRAC`, default 0) and the DSL internal_padding are
DIFFERENT values (R5 of texture-generalization) — the checker must slice by the same cell
geometry the client samples._
