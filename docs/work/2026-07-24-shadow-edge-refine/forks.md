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

## F4 · The refine is a fine-px SELECTION fused into the lightmap write (not a separate pass) {#f4}
**2026-07-24 — user framing, strengthens (b).** "We could just select the proper px because we're already
writing out the lightmap anyway." The shadow is the caster silhouette PROJECTED — and that silhouette is a fine
texture on the bound co-pack page. So the refine isn't a heavy new computation: at the fine lightmap texel we're
ALREADY writing, we **select the proper caster-silhouette px** (project the fine point to the light, sample the
caster's fine silhouette — exactly what `casterCover` does) and that px IS the sharp shadow value. It fuses into
`LIGHT_FRAG` with no extra pass and no upsample. Implications:
- Reinforces [F2](#f2) **(b)** — the bake already runs `receiverAt`/`casterCover`, so the machinery is in hand;
  the only added work is the local re-walk to identify the caster + the fine silhouette sample, gated to `(0,1)`.
- Weakens [F2](#f2) **(a)** — since the bake can select the px inline, plumbing a caster-id through the gather is
  likely unnecessary (keep (a) only as the escape hatch if (b)'s per-edge walk measures too hot).
- Still needs to know WHICH caster to project to (the walk / stored id) — "select the proper px" is the SAMPLE,
  not the caster search; the search is the one cost that stays.

## F3 · Refine ALL casters/lights at an edge, or just the dominant one {#f3}
**2026-07-24 — DECIDED: the dominant (shadowing) slot(s), not a full re-solve.** The coarse per-slot coverage
already tells us WHICH light-slots are partially occluded at this texel — only those get re-tested. Where two
casters' shadows overlap at an edge they share one sharpened boundary ([issues.md#i2](issues.md#i2)) — acceptable
for a forest; a full per-caster fine re-solve would defeat the "keep it cheap" premise.
