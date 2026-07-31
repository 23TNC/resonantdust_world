# Blockers — strip the lighting + shadow system

## B1 — What replaces it is yours to author (does NOT block the strip)

**Filed open deliberately**, because the question must be visible — but it gates nothing in
[`todo.md`](todo.md). The strip is designed to complete without an answer, ending at a renderer that
draws unlit albedo with the seam documented and the budget measured. Read this as *the question the
strip is clearing the ground for*, not as a stop.

**Why it is yours.** Every prior lighting stream was executed against a model you authored, and the
one time the model got re-derived mid-stream — the two-axis penumbra interval — it broke the render
twice and was reverted. The re-think is a design decision about what the game should look like and
what it should cost, which is not recoverable from the code.

**What the strip hands you to decide against.** By P5 each of these is a measured number or a written
list rather than an estimate:

- the **ms budget** the whole system was spending (P1 measures the frame with it dark);
- the **capability checklist** of everything the old system actually delivered (P0);
- the **G-buffer contents** already available at the seam — albedo, normal, depth, z-row;
- the **record bands** that survive — the primitive graph, self-positioning after F8;
- **~83 MiB** of freed GPU memory, and what it was buying.

**The questions worth having answers to, before the replacement starts:**

1. **Deferred or forward?** Today's is deferred (a G-buffer, then lighting passes). The per-pixel
   z-order in [I4](issues.md#i4) is a consequence of that choice, not of lighting.
2. **How many lights, and authored or dynamic?** The design says _"dense AUTHORED point lights, not a
   sun"_. Eight per tile was a bit-packing artefact, not a design ceiling — worth restating
   deliberately rather than inheriting.
3. **Do shadows return at all, and cast by what?** Silhouette-projection was the whole architecture.
   Tile/wall casting was never built and kept being the thing that did not fit.
4. **Baked, incremental, or per-frame?** The cold/hot tier split, the dirty-rect machinery and the
   never-wired differential ([I5](issues.md#i5)) all exist to avoid re-computing. A per-frame model at
   today's resolutions may simply be affordable now — that is measurable, and P0/P1 measure it.
5. **What resolution?** Lighting is pinned at 64/tile, shadow at 16/tile, art at 128. Those ratios
   are load-bearing in a dozen places and none of them has to survive.

**Recommendation: answer nothing yet.** Let the strip run first. The numbers it produces are better
inputs than the ones available today, and the strip's value does not depend on the answer — which is
precisely why it was worth doing separately.
