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

**PICK:** the shared builder over Pixi's global block — one place, matches Pixi's own binding. _(pending)_

## F2 · Bit-op cleanup — real `uint` now, or leave float-mod

- **Convert to `uint` during the migration (chosen).** Read the unorm8 byte, `uint(v*255.0+0.5)`, then real
  `& | ^ <<`. Behaviour-identical on unorm8, and it's the whole point of "use the new features". Also makes
  the eventual integer-texture switch (`caster-lut` C5) a smaller diff.
- **Leave float-mod, migrate dialect only.** Smaller migration, but keeps the fakery we're unifying to shed,
  and the `uint` change would happen anyway at the integer-texture step.

**PICK:** convert bit ops to `uint` as part of the migration (still on unorm8 textures — formats unchanged).
_(pending)_

## F3 · Rollout — incremental vs big-bang

- **Incremental, shader-by-shader (chosen).** ES 1.00 and ES 3.00 coexist ([I-4](issues.md#i-4)), so each
  shader migrates + verifies on its own with the tree green; a regression is isolated to one shader. Order:
  builder → screen shaders → shadow → bakes (cost-ascending).
- **Big-bang (all at once).** One sweep, but a single render regression is hard to bisect across 9 shaders.

**PICK:** incremental. Bakes last, so the high-value/low-risk shaders are unified even if the bake transform
plumbing proves fiddly ([I-1](issues.md#i-1)). _(pending)_

## F4 · Do the bakes get migrated too?

- **Yes — full unification (chosen, per the ask).** The goal is *one* standard; leaving the four bakes on
  high-shader would keep the split alive. They gain nothing from ES 3.00 *today* (their win is MRT later),
  but unifying them is the point, and it's what unlocks the MRT collapse in-standard.
- **Defer the bakes.** Migrate only the shaders that benefit now (shadow/overlay/display); leave the bakes.
  Faster, but *not* unification — reopens the "two standards" state the user asked to end.

**PICK:** migrate the bakes too (M4), last in order so their cost can't block the rest. _(pending)_
