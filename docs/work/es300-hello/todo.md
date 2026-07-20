# Todo — es300-hello (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. New file(s) under
`client/pixijs/src/game/viewport/`. See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

## E1 · Raw ES 3.00 `GlProgram` — 2026-07-20

- [ ] Hand-write a vertex + fragment source pair, both starting with `#version 300 es` on **line 1**, and
      wrap them in a Pixi `GlProgram` (`new GlProgram({ vertex, fragment })` or `GlProgram.from`). **Confirm
      Pixi passes the version directive through as the first line** with no injected preamble before it —
      the #1 risk ([I-1](issues.md#i-1)). If it injects, strip/bypass ([F1](forks.md#f1)).

## E2 · Full-viewport quad mesh + the ES-3.00-only fragment — 2026-07-20

- [ ] A clip-space quad `Geometry` (`aPosition` at `[-1,1]`) + a `Mesh`/`Shader` over the raw program,
      added to the viewport container (the `shadowCast` custom-mesh pattern). Fragment = the `uint`/bitwise
      checkerboard ([README](README.md)). Toggle with a `/es300` command in `WorldScene`.

## E3 · Verify — 2026-07-20

- [ ] `/es300` on: the checkerboard renders full-viewport; **console shows NO `uint`/compile/link error**;
      the rest of the scene (ES 1.00 batches) still renders — ES 1.00 + ES 3.00 coexist ([I-4](issues.md#i-4)).
- [ ] Negative control: we already know ES 1.00 rejects `uint` (the overlay/bitfield history); optionally
      flip the source to `#version 100` once and confirm it fails to compile, so the pass is unambiguously
      the ES 3.00 path ([I-6](issues.md#i-6)).

## E4 · (Stretch) `texelFetch` / integer sampler — 2026-07-20

- [ ] Optional: extend the fragment to `texelFetch` a texture (integer texel coords, no filtering) — the
      construct `caster-lut` C5 actually needs. Proves the sampler path, not just arithmetic. Not a gate.
