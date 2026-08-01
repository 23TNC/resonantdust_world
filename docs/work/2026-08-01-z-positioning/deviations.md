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


## 2026-08-01 · P2's "screen→world transform for `occludes()`" was not needed

The item reads *"Give `occludes()` the screen→world transform it has never had"*. **Built nothing
there**, deliberately.

Once [F9](forks.md#f9) made `unit.z` mean one thing, every term in the height test is a **drawn**
quantity — `Lz`, `cElev`, `subHi`, `targetH` — so they already compare correctly. Adding a uniform
`sin(θ)` to all of them would scale both sides of `hBot ≤ h ≤ hTop` and change nothing, at the cost of
implying the terms were previously inconsistent *there* rather than at `N·L`.

The transform went where a height actually meets a horizontal distance: the two `ldir` expressions.
The item's intent — "stop comparing screen quantities to world ones" — is met; the location it named
was wrong because it was written before [I8](issues.md#i8) was resolved.
