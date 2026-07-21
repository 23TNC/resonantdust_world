# Forks — 2026-07-21-shadow-bitfield

_Decision points + options + which we chose + why._

---

## F1 · Bit write: gather vs scatter (ping-pong) — 2026-07-21 (RESOLVED)

Setting each light's bit in the integer `RGBA32UI` `shadow-cold`. Options:

- **(a) Gather** — one fragment pass per rectangle; each fragment loops the reaching cold lights,
  tests shadow, ORs their bits **in a register**, writes the full `u128` once. No blend, no
  ping-pong; overlap is just multiple bits.
- **(b) Scatter + ping-pong OR** — rasterize each light's projected fan, OR its bit into the target;
  overlapping fans are a read-modify-write GL can't bitwise-blend → ping-pong (the old hot-shadows
  F2 (a)).

**Decided (user, 2026-07-21): (a) gather.** "I do not believe this requires a ping/pong because we
are not performing a read/modify/write, just updating all lights that affect a rectangle in a single
pass." Strictly cleaner than (b); **retires hot-shadows F2**.

**Cost justification (why gather, not marginally — 2026-07-21).** With `A` = dirty-rect area,
`L` = cold lights, `L_reach` = lights reaching a pixel/rect (post box-cull), `C` = casters/light,
`T` = one point-in-silhouette test:

- **Gather:** `A · (L·cull + L_reach·C·T)`, **one pass**, no blend. Scales with **dirty area × lights
  reaching × casters-each**.
- **Scatter:** `L · (A copy + covered·raster) + L swaps`. The rasterization is cheap; the tax is that
  a correct **128-bit integer** bitfield forces **O(L) passes** — you can't bitwise-OR into an integer
  target, so every variant (per-light ping-pong copy+OR, stencil dedup, float additive-disjoint) pays
  per-light work + precision limits. **No cheap single-pass scatter into a 128-bit integer target.**

The regime decides it. Scatter wins for **few lights / huge silhouettes / whole-screen rebuild**
(hardware coverage beats per-pixel testing, small `L` → cheap ping-pong). **We are the opposite on
every axis:** dense many-lights (finite radius → tiny `L_reach`, but scatter's pass count = `L`, so
it's punished by the very thing we scale up); dirty-rect incremental (gather cost ∝ dirty area;
scatter re-pays `L·A` copies regardless); few casters/light (small inner term); world-space
persistent (gather recomputes a rect in isolation; scatter can't *subtract* a bit from an OR, so an
incremental change clears + re-scatters the rect's whole light set anyway — gather's work as N
passes). Gather's honest costs — testing whole-rect area incl. ultimately-unshadowed pixels, and warp
divergence — are bounded by the box-cull, the **per-rect light pre-cull** (F5), and modest,
spatially-coherent rects. **Verdict: gather, decisively.**

## F2 · Where the shadow-shape test lives (consequence of gather) — 2026-07-21 (RESOLVED)

Gather forces the shape test into the fragment. Options:

- **(a) Per-fragment predicate** — each fragment, per reaching light, loops the light's casters and
  runs a point-in-projected-silhouette test against the cold prim textures (`texelFetch`). Reuses
  `shadow-projection`'s math as the predicate.
- **(b) Keep the rasterized fans** — impossible under gather without reintroducing scatter+ping-pong.

**Decided (2026-07-21): (a).** The projection formula is reused per-fragment; the fan-scatter draw in
`shadowCaster.ts` retires. Cost is per-fragment × per-reaching-light × per-caster — bounded by the
box cull, LUT-bounded casters, the **per-rect light pre-cull** (F5), and the per-frame budget (P5).

## F3 · Dirty-rect grouping granularity + clean-square swallow — 2026-07-21 (open)

The user wants dirty squares merged into larger pass rectangles, **including clean squares** when one
larger pass beats many small passes. Open detail: the **cost heuristic** for "swallow a clean square."

- **(a) Bounding-box merge** — group a connected dirty cluster into its AABB (on the toroidal grid),
  swallowing every clean square inside. Simplest; can over-include for sparse/L-shaped clusters.
- **(b) Greedy row/strip merge** — merge dirty squares into maximal horizontal (or 2D) strips, only
  swallowing clean squares whose swallow-cost < the setup saved. Tighter, more logic.

**Lean: (a) to start** (AABB per connected cluster, capped size), measure, tighten to (b) only if
over-inclusion shows up in the budget. Decide during P3.

## F4 · Bit capacity + bit assignment — 2026-07-21 (RESOLVED for cold-only)

`RGBA32UI` = 128 bits. Cold-only: **bit = the light's index** in `cold_light_data` (0..127) —
**no `shadow_bit_index` field** needed. 256 lights = a 2nd `RGBA32UI` later; the explicit `u8`
`shadow_bit_index` stays a **hot-tier** concern ([`hot-shadows`](../hot-shadows/README.md), tabled).

## F5 · Per-rectangle light pre-cull (the lever) — 2026-07-21 (open)

The cost win (F1) hinges on the fragment looping only a **few** lights, not all 128. Because a pass
rectangle's pixels share ~the same reaching set, compute **which lights reach the rect** once per
rect (box-test each light's radius vs the rect AABB) and feed the fragment only that list — so the
per-pixel outer loop is `L_rect` (a handful), not `L`. Options:

- **(a) CPU pre-cull → per-rect light-index list** passed as a small uniform array (rect emits it in
  P3). Simplest; the light list is tiny; the fragment still `texelFetch`es each light's full data.
- **(b) GPU pre-cull** (compute/transform-feedback building per-rect lists) — premature; movers/hot
  aren't here yet.

**Lean: (a).** The rect-accumulation (P3) emits each pass-rect's reaching-light list alongside its
bounds. **Sizing tension (feeds F3):** a *bigger* rect reaches *more* lights (longer per-pixel loop +
more warp divergence) but costs fewer passes; a *smaller* rect loops fewer lights but more passes.
The grouping heuristic should weigh added lights, not just added area. Tune during P3/P5.
