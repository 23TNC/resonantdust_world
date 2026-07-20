# Deviations — es300-migration

_Logged at the moment of deviating, per the docs convention._

---

## D-1 · The linchpin was a one-bit version injection, not a hand-rolled builder — 2026-07-20

**Plan ([M1](todo.md), [README](README.md)):** hand-roll a shared ES 3.00 program builder that reverse-
engineers and re-implements Pixi's camera plumbing (`globalUniforms` block, transform, roundPixels), because
"a raw ES 3.00 shader must supply that itself."

**Reality (much simpler):** Pixi's high-shader GL *templates are version-agnostic* — they already use
`in`/`out`/`texture`/`finalColor`, which Pixi *shims down* to ES 1.00 with `#define in varying` only when the
source isn't ES 3.00. So the whole plumbing (vertex, `globalUniformsBitGl`, `localUniformBitGl`,
`roundPixelsBitGl`, textureBit) is **already valid ES 3.00**. The entire builder collapsed to a **one-bit
wrapper** (`es3HighShader.ts`, `compileHighShaderGlProgramES300`) that injects `#version 300 es` into the
fragment; Pixi's `GlProgram` detects it, strips the WebGL1 shims, and re-inserts the directive as line 1.
Migration per shader = swap `compileHighShaderGlProgram(` → `compileHighShaderGlProgramES300(`. No
reverse-engineered UBO, no re-implemented vertex. [I-1](issues.md#i-1)'s "#1 risk" evaporated.

## D-2 · Kept float-mod bit ops (deferred the `uint` cleanup) — 2026-07-20

**Plan ([F2](forks.md#f2)):** convert the shadow/overlay bit math from float-mod to real `uint`/bitwise
during the migration.

**Done:** left the float-mod as-is. The ask was *dialect unification* (one standard), which is fully met —
every program is ES 3.00. Float-mod on unorm8 is behaviour-identical, so converting it is a separable, purely
cosmetic cleanup with its own regression surface; not converting keeps the migration strictly
behaviour-preserving ([I-5](issues.md#i-5)) and the diff minimal. The `uint` rewrite lands naturally with the
**integer-texture** switch (`caster-lut` C5), where it's actually load-bearing. Recorded as remaining in
[`todo.md`](todo.md), not lost.
