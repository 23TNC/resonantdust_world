# Shadow-edge refine — sharpen the coarse shadow against the fine caster silhouette — 2026-07-24

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the shadow gather (`shadowGather.ts`
`GATHER_FRAG`/`walkShadow`/`casterOne`) + the fine lightmap bake (`LIGHT_FRAG`). Phases in [`todo.md`](todo.md);
decisions in [`forks.md`](forks.md); caveats in [`issues.md`](issues.md). Sibling of
[`2026-07-24-lightmap-resolution`](../2026-07-24-lightmap-resolution/README.md) (which made the LIGHTMAP fine);
this makes the SHADOW *edge* fine without making the shadow *gather* fine._

## The problem (user, 2026-07-24)
The shadow map is coarse (`TEXTILE_UNIT` = 16/tile) and the fine lightmap (`TEXTILE_SQUARE` = 64/tile) upsamples
it **nearest**, so a cast shadow's edge shows the coarse texel grid — blocky blocks where a smooth silhouette
should be (visible under a bush: the shadow blob has hard 16/tile steps). Bumping the shadow *gather* to full res
would fix it but is **too costly** (see below), so we keep the coarse gather and sharpen the edge at bake time.

## Two different costs — why full-res gather is dear but the edge is cheap
The expensive part of the shadow pass is **NOT** the silhouette rasterization — it's the **corridor walk**: for
each texel, for each light, march the light→texel segment across the caster buckets hunting occluders. That is
what scales with texel count, so a 64/tile gather is ~**16× the walk**. That's the "never mind" (user).

But the blockiness is the **caster billboard's silhouette** (the shadow's *shape*), sampled at coarse-texel
centres — a half-covered 16/tile texel bakes ~0.5 and nearest-upsamples into a solid block. The **fine lightmap
bake already visits every fine texel** and already has the caster silhouette in the shared atlas (co-pack), so it
can **re-test the caster silhouette at the fine texel position** and get a near-pixel-perfect edge. **The walk
stays coarse (cheap); only the silhouette sample goes fine.** (User: "we calculate the billboards … but the
rasterization itself shouldn't be too expensive" — correct: rasterizing one caster is cheap, walking to find it
is not.)

## The approach — re-test the caster at fine res, gated to edges ([forks.md#f1](forks.md#f1) chose (b))
In `LIGHT_FRAG`, per fine texel, per presence light: read the **coarse** shadow coverage (already upsampled).
- Coverage `== 0` (full light) or `== 1` (full umbra): keep it — the interior needs no refinement (free).
- Coverage in `(0, 1)` (a penumbra/edge texel): **re-run the caster silhouette test at the FINE texel position**
  and replace the coarse value with the sharp one. Only edge texels pay, so the added bake cost is bounded by the
  shadow's perimeter, not its area.

Two ways to know WHICH caster to re-test — [forks.md#f2](forks.md#f2):
- **(a)** the gather records the dominant caster's **prim id** per coarse texel (a few more bits than today's
  7-bit caster *row*); the fine bake re-tests that one caster — cheapest per-texel, needs gather plumbing.
- **(b) — CHOSEN:** the fine bake **re-walks a small local bucket neighbourhood** at the fine texel to find the
  caster + re-test its silhouette. No new gather storage, self-contained in `LIGHT_FRAG`, and the `(0,1)` gate
  keeps it to edges. Reuses `casterOne`/`walkShadow` (move `walkShadow` into the shared block so `LIGHT_FRAG`
  can call it).

## What it composes with
- [2026-07-24-lightmap-resolution](../2026-07-24-lightmap-resolution/README.md): the fine bake + the co-packed
  atlas (the caster silhouette is on the bound `uSurface` page at every lod) are exactly what make the fine
  re-test possible — this rides them.
- The shadow **gather** + `shadow-cold` are UNCHANGED under (b) (still 16/tile, corridor↔brute identity intact) —
  we only change how the shadow is *sampled/refined* in the lightmap bake, not how it's *computed*.
- Distinct from the RECEIVER clip (a shadow landing ON a billboard): that path (`receiverCover`) is already
  fine-res; this is the CAST shadow's *shape* on the ground/receiver ([issues.md#i1](issues.md#i1)).
