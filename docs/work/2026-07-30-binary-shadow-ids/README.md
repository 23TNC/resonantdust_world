# Binary occlusion + stored caster ids — fine shadow edges for less — 2026-07-30

_Component: [`client/webgl`](../../components/client/webgl/) · `client/webgl/src/game/viewport/shadowGather.ts`
is the whole stream. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md)._

## The user's design

Trade soft shadows — which do not currently appear on screen — for a **stored caster identity**, and spend
the savings on a **4× finer shadow edge**.

1. **Occlusion becomes binary.** Coverage drops from `u7` to 1 bit; the analytic λ interval, the
   chord→area curve and `frac` are deleted.
2. **Each shadow texel stores the prim id that occludes it, per light.** 16 lights × `u16` = 256 bits =
   two px per shadow texel, ~4 MiB per class ([F2](forks.md#f2)).
3. **The incumbent is tested first.** Re-walking a texel starts by re-running the stored caster; if it
   still occludes, stop. Under binary that is **exact**, not approximate — any occluder is a complete
   answer.
4. **The lighting pass places the caster's silhouette at FINE resolution.** With the id known there is
   no search: one record fetch, one `solveCentre`, one `sampleCard` per fine texel. The shadow edge
   becomes 64/tile instead of 16/tile.

## Why this is cheap, when the same idea cost 85% before

The shadow-edge refine was deleted on 2026-07-27 (moving-lights F7) at **9.29 ms of a 10.88 ms pass**.
It re-ran the **full `walkShadow` corridor** per fine texel — DDA, per-tile bucket fetches, `casterCover`
over 8 caster slots — to rediscover an identity the coarse pass had already computed and thrown away.
`max` keeps *how much* and discards *which*.

Storing the winner deletes the search, which is where the cost was:

| factor | F7 | here |
|---|---|---|
| `FINE_RATIO` | 8 (64× texels) | 4 (16×) — **÷4** |
| per-texel work | full corridor walk | **one** caster — **÷20+** |
| `casterCover` | pre-plane-intersection | analytic — **÷3.5** |

That puts the refine somewhere near **0.05–0.15 ms** against today's 0.286 ms lighting pass. The
arithmetic is an extrapolation and [P0](todo.md) measures the baseline it will be judged against.

## The consequences that fall out for free

- **`max` becomes `any`**, which is **order-independent** — which is exactly what `DIFFERENTIAL_WIRED`
  needs. The differential is built and gated off, and `coldShadowPrevRT`/`hotShadowPrevRT` are already
  allocated, never written, never read: **4 MiB of ping-pong buffers idling**, waiting for this.
- **7 bits per slot are freed** by binary coverage, which may remove the need for a second id texture
  entirely ([F3](forks.md#f3)).

## What is knowingly sacrificed

**A prim that occludes a slot's centre may not occlude all of it**, and only one prim is remembered
(N=1). The user's call: start at N=1, expand the px per slot if it bites. There is a companion gap the
user did not raise and N-per-slot does not fix — the refine can only ever *remove* shadow, never add it,
so a coarse texel whose centre is lit stores no id and its fine texels stay lit even where the true edge
passes through them. [F4](forks.md#f4) resolves that by reading the 2×2 id **neighbourhood** the
bilinear path already fetches, which is N≈4 where it matters for no extra memory.

## The number this stream exists to move

> **How many moving lights at reach 16, zoom 1, fit inside 8 ms** — half a 60 fps frame.

Measured before ([P0](todo.md)) and after ([P5](todo.md)) on the same harness. Everything else is
supporting evidence.

## Open question, settled first and non-blocking

The user reports no visible penumbra and has decided on binary regardless. But the geometry predicts a
**~21-texel** (1.3 tile) gradient for a conifer at `radius 0.35`, and a histogram earlier in the session
(at radius 2.0) found **61% of shadowed texels partial** — so `frac` was varying then. That gap is too
large to be a scale limit. [P0](todo.md) histograms the live coverage: it costs five minutes and decides
whether this stream deletes *working* code or *dead* code. It does **not** gate the work
([F1](forks.md#f1)).

## Done when

The headline light count is measured before and after on an identical harness, the shadow edge is
visibly finer at area1, corridor↔brute identity holds, and the id-based early-out's hit rate is a
recorded number rather than an assumption.
