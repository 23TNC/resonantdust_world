# Todo — mrt-bakes (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Edits
`SquareCache.ts` + the bake shaders. See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

## B1 · Spike — render 4 outputs into a 4-attachment target — 2026-07-20

- [ ] Build a `RenderTarget({ colorTextures: [t0,t1,t2,t3] })` (slot-sized) and render a trivial ES 3.00
      mesh whose fragment writes four DISTINCT colours to `layout(location=0..3) out`. Confirm each attachment
      got its colour (blit each to screen / `/overlayRT`). Nails how `renderer` targets a multi-attachment
      `RenderTarget` and that `drawBuffers` is wired ([I-1](issues.md#i-1)). Gate the rest on this.

## B2 · The combined MRT bake shader — 2026-07-20

- [ ] One ES 3.00 fragment (via `compileHighShaderGlProgramES300` or a raw program) with four outs computing
      `albedo` (material reconstruction) + `surface` (presence/AO/coverage) + `normal` (silhouette-keyed) +
      `zdepth_world` (tile depth), from one prim draw. Shared `discard` on coverage ([I-6](issues.md#i-6)).
      Per-prim **tier branch** (material vs flat tint) via a uniform ([F2](forks.md#f2)). Watch the
      sampler-unit budget ([I-5](issues.md#i-5)).

## B3 · Restructure `bakeSquare` — one MRT render — 2026-07-20

- [ ] Replace the per-channel loop with: resolve each prim's combined inputs once, render the square's prims
      ONCE into the 4-attachment scratch (MRT), then blit each attachment to its channel's slot + apron (keep
      the existing apron logic — [F1](forks.md#f1), [F3](forks.md#f3)). Drop the per-channel scratch renders.

## B4 · Verify — 2026-07-20

- [ ] Each channel **pixel-identical** to before: `/overlayRT albedo-cold|surface-cold|normal-cold|
      zdepth-world-cold`, plus the albedo display + `/shadowcast` (casters come from the surface bake). Pan +
      LOD-swap still bake correctly.
- [ ] Confirm the draw-count drop (the point): `lastBaked` squares now cost ~1 MRT render each, not 4. Log
      or eyeball the bake cost.
