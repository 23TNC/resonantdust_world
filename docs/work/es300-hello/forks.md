# Forks — es300-hello

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · Getting raw ES 3.00 source into the pipeline

- **Pixi `GlProgram` + `Mesh`/`Shader` (chosen).** Wrap the raw source in a `GlProgram`, drive it with a
  custom `Mesh` (the `shadowCast` pattern). Stays inside Pixi's render loop (batching, RT targets, the
  scene graph). Risk: Pixi may inject a preamble before `#version` ([I-1](issues.md#i-1)) or assume
  high-shader uniform bits ([I-5](issues.md#i-5)).
- **ES 3.00 `Filter`.** Filters already use `GlProgram.from` with raw source (the decode filter). But a
  filter needs BOTH stages ES 3.00 — Pixi's default filter vertex is ES 1.00, and you can't link an ES 1.00
  vertex with an ES 3.00 fragment ([I-2](issues.md#i-2)) — so we'd override the filter vertex too, inside
  the filter framework's own conventions. More assumptions, not fewer.
- **Manual GL (bypass Pixi).** Compile the program directly on `renderer.gl`, own VAO + draw. Zero Pixi
  interference (guaranteed the version passes through), but mixes awkwardly with Pixi's render loop and
  state. The fallback if Pixi's `GlProgram` won't pass `#version 300 es` through cleanly.

**PICKED:** `GlProgram` + `Mesh` — worked first try. Pixi passed the ES 3.00 source through cleanly
([I-1](issues.md#i-1)); the only wrinkle was the mesh pipe wanting a `texture` accessor on the shader
(trivial no-op stub). No manual GL needed. — 2026-07-20

## F2 · Which ES-3.00-only feature proves it

- **`uint` + bitwise (chosen for E2).** Minimal, and it's *exactly* what ES 1.00 rejects (`'uint' :
  undeclared identifier` — the error we keep seeing). A rendered checkerboard from `uint` math is
  unambiguous proof.
- **`texelFetch` / `usampler2D` (E4 stretch).** What `caster-lut` C5 actually needs (integer texel reads).
  More moving parts (bind a texture, integer coords); prove after the arithmetic path works.

**PICK:** `uint`/bitwise first; `texelFetch` as the stretch. _(pending)_

## F3 · What to draw

- **Full-viewport clip-space quad (chosen).** Passthrough vertex at `[-1,1]`, checker keyed off
  `gl_FragCoord` — no projection matrix, no world alignment needed. Obvious on/off with `/es300`.
- **Small world-placed quad.** Would need the camera projection wired into the raw vertex — extra plumbing
  for no extra proof at this stage.

**PICK:** full-viewport quad. _(pending)_
