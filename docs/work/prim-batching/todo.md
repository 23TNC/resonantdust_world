# Todo — prim-batching

> **⛔ CLOSED — see [`README`](README.md) banner.** The bake is amortized (6 draw calls with shadows
> off); the draw-call cost is the shadow pass. P2–P5 below are moot for the bake; the `PageBatch` +
> attribute-batching design carries over to a **shadow-batching** stream instead. Kept for the design.

_Phased. Items move to `completed.md` as they land + verify. Design: [`README`](README.md) ·
decision [`forks.md`](forks.md)._

---

## P1 · Baseline (informal — [B1](blockers.md#b1) resolved: no precise measurement needed)

- [x] Baseline = the DebugPanel's **~460** draw calls (~200 shadow, deferred). The batching delta will be
      self-evident when the geometry lands; measure then. (Precise per-pass attribution de-scoped.)
- [ ] Confirm the per-prim-mesh count: for a typical forest square, how many prims × which channels take
      the `materialNode`/`surfaceNode`/`depthNode`/`normalNode` (mesh) path vs `spriteNode` (already
      batched). This sizes the win.

## P2 · Batch infrastructure (`PageBatch`)

- [ ] A `PageBatch` builder in `viewport/`: given a channel's resolved prims for a square, **bucket by
      `resolvedTexture.source`**; per bucket build **one `Geometry`** — quad positions (world px, the
      bake `transform` maps → slot) + interleaved per-prim attributes: `aUVFrame` (from `.frame`),
      `aWorldRect`, `aTint`, `aSeed`, `aFlip`. One pooled `Mesh` per bucket, sharing the channel's shader.
- [ ] Pool the buckets across squares/frames (reuse `Geometry`/`Buffer`, grow-only) — the bake runs
      often during zoom; no per-bake allocation churn (mirror the existing `bakePool`/`materialPool`).
- [ ] Unit-check the geometry math against one prim (positions/UVs/flip match the current
      `spriteNode`/`materialNode` placement incl. the west `flipX` origin shift).

## P3 · Convert the channels (simplest → hardest; each pixel-identical)

- [ ] **`depth`** first (one texture, a couple of scalars) — rewrite `DepthBakeShader` to read the P2
      attributes instead of per-prim uniforms; route `depthNode` through `PageBatch`. **Prove one draw
      per page** + byte-identical `zdepth` vs the per-prim path (RT compare via `/showRT`).
- [ ] **`surface`** — same for `SurfaceBakeShader` (presence-premultiply).
- [ ] **`normal`** — `NormalBakeShader` (soft-α normal).
- [ ] **`material`** (hardest) — `MaterialBakeShader` samples **two** atlas textures (`residual` +
      `layers`/weight) + per-material channel arrays. Same-page batching covers `residual`; resolve
      `layers` (second sampler, or a per-material sub-batch) + move the channel params to attributes.

## P4 · Unify + retire the per-prim path

- [ ] Fold `spriteNode` / geo-tier into the same `PageBatch` path (one batched code path for every
      channel + tier). Delete the per-prim pools (`bakePool` / `materialPool` / …) + the per-prim node
      builders.
- [ ] Re-measure at `x=100,y=50` (P1 protocol): a dirty-square bake drops from `~prims × 5` to
      `~pages × 5`; a full-viewport pan re-bake from thousands to tens. Record vs baseline.

## P5 · Verify

- [ ] Browser: forest pan/zoom renders identically (albedo/normal/surface/depth/shadow all correct) —
      no seams (wrap-apron), correct west facings, correct LOD swaps (page migration moves quads).
- [ ] Update `completed.md`; note the residual idle-frame cost is the **shadow pass** (separate stream).

---

**Done when:** the bake is `O(atlas pages × channels)` draws per dirty square, pixel-identical, with
amortization + wrap-apron intact — browser-verified in the forest.
