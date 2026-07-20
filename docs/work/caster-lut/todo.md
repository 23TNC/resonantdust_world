# Todo — caster-lut (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Extends
`shadowCast.ts` + `shadowCastShaders.ts`. See [`README.md`](README.md) for the layouts,
[`forks.md`](forks.md), [`issues.md`](issues.md)._

---

## C1 · Standard 1024×12 float texture + the addressing helpers — 2026-07-20

- [ ] A small helper to build a `1024×12` `RGBA32F` `BufferImageSource` (`nearest`) over a
      `Float32Array(1024·12·4)`, shared by all three textures. Fix the addressing once ([I-5](issues.md#i-5)):
      def `k` → band `⌊k/1024⌋`, col `k%1024`, rows `[3·band .. 3·band+2]`; LUT entry `i` → px `i>>2`,
      channel `i&3`; texel-centre sampling.

## C2 · Extend the light texture to 3 px (add start_index, count) — 2026-07-20

- [ ] Grow the light data from 2 px to 3 px (px 2 = `start_index, count, —, —`) and adopt the 1024×12
      standard (or keep lights at 1024×3 — [F1](forks.md#f1)). The display keeps reading colour from px 1.

## C3 · Prim (caster) data texture — 2026-07-20

- [ ] Fill from the resolved standing prims (`standingPrims()` + each prim's world rect / silhouette
      frame): px0 `world_x/_y/_z, facing`, px1 `width, height, depth1, depth2`, px2 `frame_*`. Keep def
      indices **stable** or rebuild the LUT with the texture ([I-3](issues.md#i-3)). Patch in place via
      `texSubImage2D` ([I-2](issues.md#i-2)).

## C4 · Build the LUT + per-light (start, count) — 2026-07-20

- [ ] CPU cull (the existing `Math.hypot` in-range test) → for each light, append its in-range caster
      indices to a running LUT array and record `(start, count)` into the light texture. Upload only the
      used prefix `[0, Σcount)`. `log()` if `Σcount` would exceed 49,152 ([I-4](issues.md#i-4)).

## C5 · Prove the indirection drives the cast — 2026-07-20

- [ ] A shader cast that reads light `k` → its LUT run → each caster from the prim texture → projects the
      wedge, reproducing the shadows `shadow-tiered` already casts (start by matching the current billboard
      output). This is the end-to-end proof the three-texture indirection works on the GPU ([F3](forks.md#f3)).

## C6 · Verify — 2026-07-20

- [ ] `?focus=100,50&shadowcast`: shadows identical to the JS cast, now driven entirely by the textures.
      Move a light → its LUT run rebuilds, shadows update. Console clean (no format / `uint` / overflow).
      Confirm f32 indices round-trip exactly (no off-by-one from precision).
