# Forks — es300-migration

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · The shared builder — how the raw ES 3.00 vertex gets Pixi's camera

- **Consume Pixi's `globalUniforms` block (chosen).** Pixi v8's WebGL2 path binds a `globalUniforms` UBO
  (`uProjectionMatrix`, `uWorldTransformMatrix`, `uWorldColorAlpha`, `uResolution`) + a per-object local
  transform. Declare the matching block in our ES 3.00 vertex and Pixi binds it — same values the high-shader
  got, so output is identical. Cost: reverse-engineer the exact block layout/names ([I-1](issues.md#i-1)).
- **Per-shader hand-rolled uniforms.** Each shader declares its own `mat3 uTransform` and we set it manually.
  More control, but duplicates the plumbing 8× and diverges from how Pixi drives the mesh.
- **Clip-space / pre-transformed positions only.** Works for full-screen quads (es300-hello) but NOT the
  bakes (they need the world→slot transform Pixi composes) — so it can't be the general answer.

**RESOLVED — none of the above needed.** Pixi's high-shader templates are version-agnostic, so a one-bit
`#version 300 es` injection reuses Pixi's own vertex/uniform plumbing verbatim. No builder, no UBO
reverse-engineering ([D-1](deviations.md#d-1)). — 2026-07-20

## F2 · Bit-op cleanup — real `uint` now, or leave float-mod

- **Convert to `uint` during the migration (chosen).** Read the unorm8 byte, `uint(v*255.0+0.5)`, then real
  `& | ^ <<`. Behaviour-identical on unorm8, and it's the whole point of "use the new features". Also makes
  the eventual integer-texture switch (`caster-lut` C5) a smaller diff.
- **Leave float-mod, migrate dialect only.** Smaller migration, but keeps the fakery we're unifying to shed,
  and the `uint` change would happen anyway at the integer-texture step.

**DEFERRED — kept float-mod.** Dialect unification is met without it; float-mod on unorm8 is
behaviour-identical, so the `uint` rewrite lands with the integer-texture switch (caster-lut C5) where it's
load-bearing ([D-2](deviations.md#d-2)). — 2026-07-20

## F3 · Rollout — incremental vs big-bang

- **Incremental, shader-by-shader (chosen).** ES 1.00 and ES 3.00 coexist ([I-4](issues.md#i-4)), so each
  shader migrates + verifies on its own with the tree green; a regression is isolated to one shader. Order:
  builder → screen shaders → shadow → bakes (cost-ascending).
- **Big-bang (all at once).** One sweep, but a single render regression is hard to bisect across 9 shaders.

**PICKED:** incremental — albedoBlit first (proof), then screen/shadow/bakes. The bake transform plumbing
was a non-issue (D-1), so all landed cleanly. — 2026-07-20

## F4 · Do the bakes get migrated too?

- **Yes — full unification (chosen, per the ask).** The goal is *one* standard; leaving the four bakes on
  high-shader would keep the split alive. They gain nothing from ES 3.00 *today* (their win is MRT later),
  but unifying them is the point, and it's what unlocks the MRT collapse in-standard.
- **Defer the bakes.** Migrate only the shaders that benefit now (shadow/overlay/display); leave the bakes.
  Faster, but *not* unification — reopens the "two standards" state the user asked to end.

**PICKED:** yes — all four bakes migrated (M4). Their cost turned out trivial (one-line swap each). — 2026-07-20
