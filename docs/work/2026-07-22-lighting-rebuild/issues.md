# Issues — lighting rebuild

_Problems hit + candidate solutions + which we chose + why. Chronological. This file is the record
of **why the previous approach is being torn out** so we don't rebuild the same failures._

---

## Why the old corridor march is removed — 2026-07-22

The `2026-07-22-shadow-corridor` 1-tile Amanatides–Woo march failed in several independent ways.
The rebuild must not reproduce any of them; P6 acceptance is a diff against the brute-force output.

1. **Wedge bug (the big one).** The corridor is one tile wide and steps tile-by-tile, but a caster's
   projected **quad spans tiles the march never visits**. Texels whose march happened to cross the
   caster's tile got the full quad; adjacent texels whose march stepped around it got nothing →
   triangular **wedges missing** from every shadow. Proven by swapping the march for a brute-force
   walk of all in-reach casters: the wedges vanished and the quad rendered solid. **The quad math
   was always correct; the march granularity was eating the wedges.** → rebuild corridor last, as a
   pure optimization validated against brute force.

2. **Shadows capped at ~3 tiles.** The march's step/test caps (`m < 24`, `tested >= 64`) plus the
   removed length-cull left shadows woefully short — a light throwing 6–12 tiles produced ≤ 3-tile
   shadows.

3. **Picked up casters outside the corridor.** Shadows appeared whose caster the 1-tile corridor
   should never have reached (a tree not on the marched column still cast into it) — the
   bucketing/corridor tile mapping was inconsistent.

## Why we dropped the `u/v` inversion — 2026-07-22

The occlusion test inverted the projection to solve card params `(u,v)` for each ground point, then
range-checked them. The solve divides by `denom` (→ 0 in a horizontal band through the light's row)
and by the foreshortening `s` (→ ∞ near the shadow tip). Both blow up as shadows elongate, producing
`v`-length clips (horizontal, thickening with y-distance) and `u`-width clips (radial). **Replaced
by point-in-quad membership** (four cross-products), which has no per-pixel division to blow up and
**cannot produce out-of-range** — a point is simply in the quad or not.

## Sweep loop miscompile — no shadows {#reach-walk} — RESOLVED 2026-07-23

- **Real root cause (found 2026-07-23):** the brute-force sweep's outer loop
  `for (int dy = -16; dy <= 16 && cov < 1.0; dy++)` put a **loop-body-modified variable (`cov`) in
  the for-condition**. The GLSL compiler mishandled it and **silently skipped iterations**, so the
  sweep never reached the caster's tile — every value was correct end-to-end (bucket held the prim,
  GPU texel matched the mirror, `decodePos` was right, hardcoding the id rendered), yet the lookup
  never fired. This produced a *pile of contradictory probes* (direct `pmod` read returned 192, but
  the walk's `lc+(dx,dy)` iteration at that tile never executed, with `reachT` provably ≥13).
- **Fix:** rewrote the sweep in slot-space with a **pure constant-bound loop** — no body-dependent
  loop condition, no `break`. Iterate the caster texture around the light's slot
  (`ls = pmod(lc, cols)`), wrap with `pmod`. Shadows render; verified at zoom 0.5/1/2.
- **My earlier (wrong) diagnosis** below — a toroidal-basis mismatch — was a red herring; the
  indexing was actually aligned. But [`map-model.md`](map-model.md) still stands as the right rigid
  backbone (it's what makes this class of lookup bug impossible to reintroduce), and the sweep
  rewrite conforms to it.

### (superseded diagnosis) reach-walk reads the wrong toroidal tile

- **2026-07-22:** A pinned light at tile (54,21) cast **no shadow** despite every value checking out
  end-to-end: the caster bucket at tile (52,18) held prim 192 (JS mirror **and** GPU readback), prim
  192's `pos` texel decoded to the right anchor, and hardcoding that anchor through `decodePos`
  rendered the correct shadow. A shader debug that read tile (52,18) **directly** via
  `pmod(52, uCols)` returned 192 **everywhere** (green) — so the bucket + toroidal indexing are
  aligned and readable. But the brute-force **reach-walk** (`cur = lc + (dx,dy)`, `lc =
  floor(L.xy/UPT)`) **never** read prim 192 — it walks a different set of tiles than the direct read
  lands on.
- **Root cause:** the caster buckets / reach-walk and the shadow-cold RT don't index on the **same
  toroidal basis** — the reach-walk's `lc`-centred tile coords don't line up with the buckets'
  `pmod(worldTile, cols)` cells. The `u/v`-correct, data-correct path still misses because the two
  maps disagree on *which tile is which texel*.
- **Fix:** [`map-model.md`](map-model.md) — put **every** map on one shared `cols × rows` TILE grid
  with one tile-level `pmod` wrap, so the walk's tile coords equal the buckets' by construction.
  Tracked as **P1.5** in [`todo.md`](todo.md).

## Zoom-dependent missing shadow {#zoom}

- **2026-07-22:** A tree cast **no shadow at certain zoom levels**. Cause: `ColdShadowData.definitionFor`
  gated the caster def on the **surface texture LOD resolving** (`if (surf.geo || !surf.frame) return -1`),
  and `buildCasters` skips any caster with `def < 0`. When a tree's texture LOD hadn't loaded at the
  current zoom, it silently cast no shadow.
- **Fix (partial, already applied):** pure-quad shadows need only the billboard **W/H** (from prim
  geometry, not the atlas), so `definitionFor` no longer gates on the surface. This must stay true
  through the rebuild — **shadow presence must never depend on texture loading** (README
  non-negotiable). NOTE: P3 re-introduces a *frame* dependency (opaque-bbox from the surface) — that
  is a **sizing refinement**, and must degrade gracefully (fall back to prim W/H until the frame
  resolves) rather than dropping the shadow.
