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
