# Forks — prim-batching

_Decision points, options, what we chose, why._

---

## F1 · Per-square page-batch, not a global persistent per-page mesh (2026-07-18)

**Context.** The bake emits one `Mesh` per prim. Two ways to batch to "one mesh per atlas page":

**Options.**
- **(A) Per-square page-batch** — inside `bakeSquare`, group the square's prims by `texture.source`
  and emit one batched mesh per (channel, page). The toroidal per-square scratch → wrap-apron blit
  stays exactly as is (batching happens inside the scratch render). Draws/bake =
  `channels × pages_in_square` (~1–3), down from `channels × prims`.
- **(B) Global persistent per-page mesh** — one long-lived mesh per atlas page holding **every** prim's
  quad, quads added/removed as tiles go dirty (how the idea was first framed). Lowest possible draw
  count (`channels × total_pages`). But: it fights the **per-square wrap-apron** (the composite is a
  toroidal RT blitted per square, not a flat world buffer), and a persistent mesh re-rendered each
  frame would **re-draw the whole world every frame** — destroying the amortization the RT design
  exists for. Making it amortized-and-global needs the toroidal layout reworked to re-render page
  batches region-by-region on dirty — a much larger change.

**Decision: (A).** It removes the actual defect (per-prim draws) while keeping the **proven,
amortized, toroidal** design untouched, and it isolates the genuinely hard part — the
**shader-attribute refactor** (per-prim uniforms → vertex attributes) — behind a small, verifiable
surface. (B) is deferred: revisit only if, after (A), the per-square page count is still a measurable
cost — and only alongside a deliberate rework of the toroidal per-square blit.

**Not a regression risk for z-order:** batching by page reorders draws vs the current zIndex sort, but
the G-buffer is **opaque and `zdepth`-resolved** — overlaps are decided by the depth channel, not draw
order — so atlas interleaving is safe here. (It would **not** be for an alpha-blended sprite layer;
this is specific to the deferred design.)
