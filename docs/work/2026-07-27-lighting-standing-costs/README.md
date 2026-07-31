# lighting-standing-costs — cut the lighting path's fixed taxes, then make cost proportional

_Work stream, opened 2026-07-27. Component: [`client/webgl`](../../components/client/)
(`game/viewport/shadowGather.ts`). Sibling of `light-budget` —
see "composition" below._

## What — three sources of waste the per-pair number hides

The analytic-interval build measures a flat ~0.0015 ms per tile-light pair
(`light-budget I2`). That marginal cost is honest, but the
code-read (2026-07-27) found three costs it does not capture:

1. **The standing tax.** `tick` calls `classPass` for BOTH classes unconditionally
   (`shadowGather.ts:1766`). Each submits: a full-RT prev-shadow blit, a 512×256 gather draw, and a
   **4096×2048 fine lighting draw** whose 8.4 M fragments all fetch the constants row + dirty texel
   and discard. A fully static frame rasterizes ~17 M prologue fragments and copies ~8 MB to do
   nothing. Worse, two of the payloads are DEAD: `uShadowPrev` has **no consumer** (the differential
   is unwired — the blit feeds nothing), and the gather's second attachment `oCasterD` (`:944`,
   "#3 vestigial") has **no reader** of `textures[1]` — it doubles gather write bandwidth, the prev
   blit, and RT memory across four RTs.
2. **The discard tax.** The dirty gate is a per-texel fetch-and-discard on a fullscreen draw. At the
   reach-4 measurement, 110 dirty tiles ≈ 1.8 M fine texels do work while 6.6 M rasterize purely to
   discard — a fixed tax that dominates exactly when the dirty set is small, i.e. the case every
   other optimisation drives toward. The CPU already owns the exact dirty mirror before the draw.
3. **The receiver rescan.** `receiverAt` + `billboardNormal` run per fine texel — a 6-row bucket
   scan with full billboard/def record decodes per occupied slot, plus the gather's 3-corner
   `allBillboard` test (`:887`). All of it is **light-independent**: when a tile is dirtied by light
   MOTION (the dominant case) its receiver geometry is unchanged, yet every fine texel re-derives it
   in both shaders, every re-bake. This is a large share of the 0.0015 ms constant itself.

## Design stance

- **Delete before optimising.** P1 removes the dead attachment and gates the dead blit before
  anything is made faster — a smaller baseline makes every later number attributable.
- **The dirty mirror becomes the draw list.** P2 replaces fullscreen-discard with merged dirty-rect
  quads for both the gather and the fine draw. Rects are tile-aligned so coverage is exact; the
  `uDirty` texel gate stays on during the transition as belt-and-braces, then retires. This also
  makes the `light-budget` admission model exact: admitting a
  tile = drawing its quad, no discard tax smearing the cost model.
- **Receiver geometry is baked, not rescanned.** P3 adds a persistent FINE receiver map
  (billboard id, coverage, baseY, world normal) with its OWN dirty channel fed only by
  billboard/prim changes — never light moves ([F2](forks.md#f2)). Both shaders then read one texel
  where they scan six tiles today. In-family and frame-indexed, so zoom-stable by construction
  (the [map-compatibility](../2026-07-24-map-compatibility/README.md) rule).
- **Identity is the contract.** None of this changes WHAT is computed. Every phase holds
  corridor↔brute `debugReadShadow` identity at 0 mismatches, and P2/P3 additionally require
  bit-identity against the pre-change build on a static scene.

## Composition with light-budget

Do P1 first — it is near-free and shrinks the baseline the budget will be sized against. The
reach-dependence of the pair constant (0.00150 → 0.00171 ms across reach 8 → 12, from `walkShadow`'s
distance-scaled DDA) is logged as
`light-budget I3`; the final phase here re-runs the
budget's P0 sweep on the improved constants so the allowance isn't sized against stale numbers.

## Out of scope (already owned elsewhere)

- **F4 delta-threshold** (shrink the dirty set) — `light-budget F4`.
- **Coarser-lod hot bakes** — `moving-lights` lever +
  [textile-slot P4](../2026-07-26-textile-slot/todo.md).
- **Inverting the walk** (per-(light, caster) rasterization, O(shadow area)) — the deferred
  instanced-cast item in `hot-shadows`/`caster-lut`;
  reach for it only if this stream + the planned levers fall short.
- **CPU presence/caster rebuilds** — `incremental-presence`.
