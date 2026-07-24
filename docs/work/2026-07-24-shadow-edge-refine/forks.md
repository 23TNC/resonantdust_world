# Forks — shadow-edge refine

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Full-res shadow GATHER vs refine the EDGE at bake time {#f1}
**2026-07-24 — DECIDED: refine the edge (keep the coarse gather).** Bumping the gather to `TEXTILE_SQUARE`
(64/tile) fixes the blockiness but costs ~16× the **corridor walk** (the walk — marching light→texel across
caster buckets per light — is the expensive part, and it scales with texel count). The blockiness itself is the
caster **silhouette** sampled coarse; the fine lightmap bake already visits every fine texel and has the caster
silhouette on the bound co-pack page, so it can re-rasterize the silhouette at the fine texel for a sharp edge —
the walk stays coarse. Rasterization is cheap; the walk is not (user). So refine, don't re-gather.

## F2 · How the fine bake knows WHICH caster to re-test {#f2}
**2026-07-24 — DECIDED: (b) local re-walk in the bake; (a) is the fallback.**
- **(a) Store the caster prim id per coarse texel.** The gather records the dominant caster's prim id (today it
  stores only the caster ROW, 7 bits — not enough to re-rasterize); the fine bake re-tests that one caster.
  Cheapest per-texel, but needs the id plumbed through the gather + more `oCasterD` bits.
- **(b) — CHOSEN. Re-walk a small local bucket neighbourhood in `LIGHT_FRAG`.** The fine bake, only where the
  coarse coverage is a partial `(0,1)` edge value, re-walks a few tiles around the fine texel to find the caster
  and re-tests its silhouette. NO gather change (self-contained in the bake), and the `(0,1)` gate limits it to
  the shadow perimeter. Reuses `casterOne`/`walkShadow`. Chosen because it's self-contained + naturally bounded;
  (a) is the escape hatch if (b)'s per-edge walk measures too hot ([todo.md#p3](todo.md)).

## F3 · Refine ALL casters/lights at an edge, or just the dominant one {#f3}
**2026-07-24 — DECIDED: the dominant (shadowing) slot(s), not a full re-solve.** The coarse per-slot coverage
already tells us WHICH light-slots are partially occluded at this texel — only those get re-tested. Where two
casters' shadows overlap at an edge they share one sharpened boundary ([issues.md#i2](issues.md#i2)) — acceptable
for a forest; a full per-caster fine re-solve would defeat the "keep it cheap" premise.
