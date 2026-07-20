# Todo — es300-hello (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. See [`README.md`](README.md),
[`forks.md`](forks.md), [`issues.md`](issues.md)._

---

**E1–E3 done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** A hand-written raw
`#version 300 es` program (using `uint`/`uvec2`/bitwise — syntax ES 1.00 can't compile) renders a
full-viewport checkerboard via `/es300`, coexisting with the ES 1.00 scene. **Key finding:** Pixi v8
supports ES 3.00 natively — just include `#version 300 es` in the fragment; no manual GL needed.

## E4 (optional) · `texelFetch` / integer sampler

- [ ] Prove the ES 3.00 sampler read (`texelFetch`, integer texel coords). Deferred to
      [`caster-lut` C5](../caster-lut/todo.md) where it's needed — the instanced GPU cast reads the
      light/LUT/caster textures. No reason to prove it in isolation here first.
