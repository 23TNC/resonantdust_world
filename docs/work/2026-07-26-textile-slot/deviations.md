# TEXTILE_SLOT — deviations

_Where execution departed from the written plan, logged AT THE MOMENT of deviating. Chronological append._

### D-1 — "zoom IN marks 0 tiles dirty" was the wrong acceptance; 0 FRESH is the right one (2026-07-26)
**Planned (P3):** "Zoom IN: upscale + clip retained tiles, dirty nothing. Acceptance: a zoom-in step marks 0
tiles dirty."

**Measured (lod 1 → lod 0):** `carried 384 / total 384, fresh 0, dirty 384`.

**Why the plan was wrong as written.** Nothing needs baking *from black* — that part is met exactly, and it is
what kills the flash. But the carried content is a **rescale of the other lod's bake**, so at lod 0 it is
lod-1 art upscaled 2×: correct in position and colour, blurry in detail. It must still be re-baked at the new
lod to gain the detail the finer slot can now hold. Marking those clean would freeze the world at whatever
resolution it was first drawn.

So the acceptance splits into two numbers, and only one of them should be zero:
- **`fresh` = 0** — nothing is empty, nothing flashes. This is the real acceptance.
- **`dirty` = 384** — every carried tile is STALE and queued at `PRIO_STD`, below the `PRIO_HIGH` of a tile
  with no content at all. That priority split is exactly what [P5](todo.md) formalises.

**Zoom OUT is unaffected and met as written:** 384 carried, 1152 fresh — only the genuinely newly-exposed
ring bakes, where previously all 1536 were cleared and re-baked.

**Consequence for the plan:** P3's acceptance is restated in terms of `fresh`, and P5's empty-before-stale
ordering is promoted from a nicety to the thing that makes this behave well — without it, a zoom-in would
re-bake 384 stale tiles at the same priority as genuinely missing ones.
