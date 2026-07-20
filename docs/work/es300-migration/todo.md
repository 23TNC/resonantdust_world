# Todo — es300-migration (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Order: build the
shared helper, migrate cheapest→costliest, bakes last (highest transform cost). See [`README.md`](README.md),
[`forks.md`](forks.md), [`issues.md`](issues.md)._

---

## M1 · The shared ES 3.00 program builder (the linchpin) — 2026-07-20

- [ ] `makeEs3Program(fragBody, opts)` (+ a standard vertex): a raw `#version 300 es` program that consumes
      Pixi's `globalUniforms` block (`uProjectionMatrix`/`uWorldTransformMatrix`/…) + the mesh's local
      transform and does the standard world→clip transform + roundPixels, exposing a fragment-body slot and
      texture/uniform conventions. Nail Pixi v8's exact global uniform block layout ([I-1](issues.md#i-1),
      [F1](forks.md#f1)). Prove it by re-rendering **one** existing mesh (albedoBlit) through it, pixel-identical.

## M2 · Screen-space display + debug shaders — 2026-07-20

- [ ] `albedoBlitShader` → ES 3.00 via the builder (dialect only). The live display must look identical.
- [ ] `overlayShader` → ES 3.00; convert the BITS-mode float-mod to real `uint`/bitwise (reading the same
      unorm8 bytes). `/overlayRT` each channel — identical. **Fixes the stale `uint` overlay error for real.**

## M3 · Shadow shaders — 2026-07-20

- [ ] `ShadowMergeShader` + `ShadowTDisplayShader` → ES 3.00; bit clear/OR/decode become `uint`
      ([F2](forks.md#f2)); optionally `texelFetch` the light-data texture (exact index reads). Shadows +
      colours pixel-identical; pan still ghost-free (I-9 invariant holds).
- [ ] `makeShadowDecodeFilter` → an ES 3.00 filter (needs an ES 3.00 filter vertex too — [I-6](issues.md#i-6)).
      `/showRT` thumbnails identical.

## M4 · Bake shaders (highest transform cost, do last) — 2026-07-20

- [ ] `materialBakeShader`, `surfaceBakeShader`, `normalBakeShader`, `depthBakeShader` → ES 3.00 via the
      builder (they render prims through a world→slot transform — the builder must carry it). Each channel's
      bake must be **pixel-identical** — verify per channel via `/overlayRT albedo|surface|normal|depth`.

## M5 · Verify + retire ES 1.00 — 2026-07-20

- [ ] Full sweep: albedo display, every `/overlayRT` channel, `/shadowcast`, `/showRT`, `/es300` all render
      identically; console clean. `grep compileHighShaderGlProgram` returns **nothing** — one standard.
- [ ] Update the `rendering-platform.md` design doc + the `webgl2-es300` memory to "all shaders ES 3.00".
