# Deviations — z positioning

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## 2026-08-01 · P0a item 1 sourced now, ticked at P2

`todo.md` P0a asks for the tilt as "one source, **read by BOTH the record writer and the shadow
transform**". The source now exists (`worldTilt.ts`) and the axis is named, but **nothing consumes it
yet** — the record writer changes in P2b and the shadow transform in P2.

**Left unticked rather than ticked on a partial.** Injecting `WORLD_TILT_GLSL` into shaders that do
not yet use it would satisfy the wording with dead code, which is the opposite of what the acceptance
is for. The item closes when P2 wires the second reader.

**Why the plan had it this way:** P0a was written to pin unknowns *before* any transform existed, so
its acceptance reached forward to consumers that its own phase cannot create. Not worth restructuring
mid-flight — noting it is enough.
