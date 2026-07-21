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

## F2 · Where the shadow-shape test lives (consequence of gather) — 2026-07-21 (RESOLVED)

Gather forces the shape test into the fragment. Options:

- **(a) Per-fragment predicate** — each fragment, per reaching light, loops the light's casters and
  runs a point-in-projected-silhouette test against the cold prim textures (`texelFetch`). Reuses
  `shadow-projection`'s math as the predicate.
- **(b) Keep the rasterized fans** — impossible under gather without reintroducing scatter+ping-pong.

**Decided (2026-07-21): (a).** The projection formula is reused per-fragment; the fan-scatter draw in
`shadowCaster.ts` retires. Cost is per-fragment × per-reaching-light × per-caster — bounded by the
box cull, LUT-bounded casters, and the per-frame budget (P5).

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
