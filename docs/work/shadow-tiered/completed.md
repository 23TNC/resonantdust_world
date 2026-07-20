# Completed — shadow-tiered

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## T1 · Window origin exposed — 2026-07-20

`SquareCache.bufferMapping()` now returns `winCol`/`winRow` (resident window's world-square origin) — the
piece `shadow-world` lacked ([I-5](issues.md#i-5)).

## T2 · The four RTs + light groups — 2026-07-20

`screen-a`/`-b` (viewport-sized, `nearest`, A=1) + `world-a`/`-b` (fixed-buffer, share the cache mapping),
both ping-pong. Realtime set = the moved lights (seed = all 5, then one/sec). `texFor("shadow-a")` samples
the CURRENT world buffer, world-aligned.

## T3 · Screen-space cast — 2026-07-20

`castScreen` projects each realtime light's in-radius prims' billboard shadows, maps world→screen, and
rasterises into `cur-screen` via additive-blend `Graphics` (each light → its RED-byte bit). Window-bounded
by construction — no aliasing.

## T4+T5 · Merge (remove + add) in one pass — 2026-07-20

Single `ShadowMergeShader` pass: `cur-world = (prev-world with the dirty bits cleared) OR (prev-screen,
remapped screen→world)`. Reads `prev-world` + `prev-screen`, writes `cur-world` — no feedback, no blend
([I-6](issues.md#i-6)). The remove is per-bit float-mod; the screen remap uses the **cast frame's** camera
([I-8](issues.md#i-8), stashed in `camA/camB`). Collapsed T4+T5's clear-pass-plus-4-blits into one
inverse-mapped pass (simpler, one draw).

## T6 · Display — 2026-07-20

`ShadowTDisplayShader` (full-viewport): forward-maps screen→world→buffer to sample `cur-world`, samples
`cur-screen` directly, decodes both RED-byte bitfields → 5 colours, additive overlap. Built into the
`ShadowCast` container (below the markers) plus `/overlayRT shadow-a|shadow-b`.

## T7 · Verified in-browser — 2026-07-20

`?focus=100,50&shadowcast`: shadows cast from in-radius prims, coloured per light, clustered on the
markers. **Pan far → shadows stay in the (100,50) zone, no aliased copies** (the `shadow-world` bug, gone).
Moving a light clears its old shadow (no ghost) and re-casts same-frame. See [I-9](issues.md#i-9) for the
stale-on-pan follow-up found + fixed during this verify.

## I-9 fix · Leading-edge slot invalidation — 2026-07-20

The persistent carry-forward was a verbatim per-slot copy that never noticed a slot's resident world
square rotating as the window pans, so a freshly-entered zone briefly showed the evicted zone's shadow.
Fixed: the merge now zeroes `wN` when a slot's resident square differs from the frame that wrote
`prev-world` (`prevWinCol`/`prevWinRow` threaded through `uMapC`). Verified: panning to fresh zones shows
no ghost. See [I-9](issues.md#i-9).
