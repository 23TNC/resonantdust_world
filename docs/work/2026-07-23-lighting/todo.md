# Todo — lighting (execution order)

_Model + inputs in [`README.md`](README.md); decisions in [`forks.md`](forks.md); delivered work in
[`completed.md`](completed.md)._

**P1–P4 are DELIVERED** (the baked lightmap: emission + per-px normal relief + per-light shadows, 120fps
— see [`completed.md`](completed.md)). Remaining:

## P3b · Shadow ceiling — don't shadow billboards above the shadow's height
- [ ] [F6](forks.md#f6): the pass masks by GROUND shadow-cold, so a tall billboard in a low shadow is
      darkened whole (head too). Gather emits a per-light ceiling (`max caster_height·f(t)`, `t` already
      computed in `casterCover`); the mask becomes `(1 − coverage_i) OR (pixel_height ≥ ceiling_i)`.
      **First verify** `zdepth` = height vs ground-depth. Machinery we own; deferred until flat shadows
      proved out (they now have).
