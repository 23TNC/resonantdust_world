# Completed — es300-migration

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## M1 · The shared ES 3.00 helper — one-bit version injection — 2026-07-20

`es3HighShader.ts` → `compileHighShaderGlProgramES300`: calls Pixi's own `compileHighShaderGlProgram` with an
extra bit that plants `#version 300 es` in the fragment. Pixi's `GlProgram` then compiles native ES 3.00
(strips the WebGL1 shims, re-inserts the directive as line 1), and its version-agnostic templates carry the
whole camera/uniform/roundPixels plumbing along unchanged. The planned hand-rolled builder was unnecessary —
see [D-1](deviations.md#d-1). Proven on `albedoBlit` first: the albedo display renders pixel-identical.

## M2 · Screen display + debug shaders — 2026-07-20

`albedoBlitShader` and `overlayShader` swapped to `compileHighShaderGlProgramES300`. Verified: the albedo
display shows green textured trees identically; `/overlayRT albedo-cold` (overlay ES 3.00) shows the
composite correctly. Bit ops left as float-mod ([D-2](deviations.md#d-2)).

## M3 · Shadow shaders — 2026-07-20

`ShadowMergeShader` + `ShadowTDisplayShader` swapped; `makeShadowDecodeFilter` migrated by injecting
`#version 300 es` into its fragment (Pixi's `defaultFilterVert` is `in`/`out` style, so it goes ES 3.00 too —
[I-6](issues.md#i-6) handled). Verified: `/shadowcast` casts the 5-colour shadows around the markers
correctly (merge + display both ES 3.00).

## M4 · Bake shaders — 2026-07-20

`materialBakeShader`, `surfaceBakeShader`, `normalBakeShader`, `depthBakeShader` swapped. Verified via the
render they feed: green tree albedo (material), tree silhouettes casting shadows (surface), and the normal
overlay — all correct. The world→slot transform rode along in Pixi's plumbing, so no per-bake work beyond the
swap ([D-1](deviations.md#d-1)).

## M5 · Verified + ES 1.00 retired — 2026-07-20

`grep compileHighShaderGlProgram\b` outside the wrapper returns **nothing** — the client is unified to one
shader standard. Full in-browser sweep (albedo display, `/overlayRT`, `/shadowcast`) renders identically;
console shows no new compile/link errors (only stale `9:35:54` cache entries from a pre-migration build).
Design doc + `webgl2-es300` memory updated to "all shaders ES 3.00".
