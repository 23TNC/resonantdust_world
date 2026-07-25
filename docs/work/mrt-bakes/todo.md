# Todo — mrt-bakes (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Edits
`SquareCache.ts` + the bake shaders. See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

## B1 · Spike — DONE (moved to completed.md)

- [x] Build a `RenderTarget({ colorTextures: [t0,t1,t2,t3] })` (slot-sized) and render a trivial ES 3.00
      mesh whose fragment writes four DISTINCT colours to `layout(location=0..3) out`. Confirm each attachment
      got its colour (blit each to screen / `/overlayRT`). Nails how `renderer` targets a multi-attachment
      `RenderTarget` and that `drawBuffers` is wired ([I-1](issues.md#i-1)). Gate the rest on this.

## B2 · Universal material — DONE (moved to completed.md)

- [x] Extend the per-prim "material" to a superset (albedo residual/layers + surface + normal + depth +
      tint) and make every channel `resolve` produce one. The flat/geo case = a **solid material** (white
      maps + flat-up + full coverage + `geoColor` tint + tile depth), so no branch ([F2](forks.md#f2)). Decide
      solid-vs-real **once** per prim. This unifies the albedo bake even pre-MRT and is verifiable on its own
      (flat/geo prims still bake identically through the material path).

## B3 · DONE (completed.md)

- [x] One ES 3.00 fragment (raw program likely cleaner — the high-shader template hard-codes a single
      `finalColor` out; [I-2](issues.md#i-2)) with four outs computing `albedo` + `surface` + `normal` +
      `zdepth_world` from the one universal material, one prim draw. Shared `discard` on coverage
      ([I-6](issues.md#i-6)). Watch the sampler-unit budget ([I-5](issues.md#i-5)).

## B4 · DONE (completed.md)

- [x] Replace the per-channel loop with: resolve each prim's one universal material, render the square's prims
      ONCE into the 4-attachment scratch (MRT), then blit each attachment to its channel's slot + apron (keep
      the existing apron logic — [F1](forks.md#f1), [F3](forks.md#f3)). Drop the per-channel scratch renders.

## B5 · DONE (completed.md)

- [x] Each channel **pixel-identical** to before: `/overlayRT albedo-cold|surface-cold|normal-cold|
      zdepth-world-cold`, plus the albedo display + `/shadowcast` (casters come from the surface bake). Pan +
      LOD-swap still bake correctly.
- [x] Confirm the draw-count drop (the point): `lastBaked` squares now cost ~1 MRT render each, not 4. Log
      or eyeball the bake cost.


---

**B1–B5 all done + verified (2026-07-20).** Four G-buffer bakes collapsed to one MRT pass. Follow-up: remove the now-dead per-channel node methods / pools / single-attachment scratch + the 4 orphaned bake-shader files (spawned as a background task).
