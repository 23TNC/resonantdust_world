# Todo — es300-migration (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. See [`README.md`](README.md),
[`forks.md`](forks.md), [`issues.md`](issues.md)._

---

**M1–M5 done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** Every client shader is now
raw GLSL ES 3.00 via `compileHighShaderGlProgramES300` (a one-bit `#version 300 es` wrapper — the planned
hand-rolled builder was unnecessary, [D-1](deviations.md#d-1)). One standard; no ES 1.00 high-shader remains.

## Remaining (optional cleanup)

- [ ] Convert the shadow/overlay bit math from float-mod to real `uint`/bitwise ([D-2](deviations.md#d-2)).
      Behaviour-identical on unorm8, so purely cosmetic here — it lands naturally with the **integer-texture**
      switch in `caster-lut` C5, where `uint` is actually load-bearing. Not worth a
      standalone regression pass before then.
