# Forks — shadow-tiered

_Decision points + options + which we chose + why. All 2026-07-20; the pipeline is the user's design,
refined with the feedback in [`issues.md`](issues.md)._

---

## F1 · Cast realtime shadows in SCREEN space (window-bounded) — 2026-07-20

The `shadow-world` bug was casting into the toroidal buffer by raw `mod`, aliasing off-window lights.
**Chose:** cast the realtime set in **screen space** — the screen *is* the window, so it's window-bounded
by construction (only on-screen shadows exist), exactly like the composites' per-rectangle bake only
touches resident squares. Then copy screen→world (F2/[T5](todo.md)). This is the design's `shadow-hot`.

## F2 · Four RTs, both tiers ping-ponged — 2026-07-20

`screen-shadow-a/-b` (screen, realtime) + `shadow-a/-b` (world, persistent). Both ping-pong because a
frame **reads** last frame's buffers (`shadow-b`, `screen-shadow-b`) while **writing** this frame's
(`shadow-a`, `screen-shadow-a`) — the read≠write constraint that forced the design (you can't RMW one RT
in a draw). 4 RTs is the minimum that respects it.

## F3 · ADD = OR via additive blend (disjoint bits) — 2026-07-20

Merging bitfields is `+`, and since each light owns a unique bit set by exactly one source, `+` = OR
(`b1010 + b0101 = b1111`). **Chose fixed-function additive blend** (`GL_ONE, GL_ONE`) for the merge: the
GPU adds source to destination through the **blend unit**, not a shader texture-fetch of the target — so
it is **not** a feedback loop and needs no ping-pong for the add itself. **Invariant (load-bearing):**
no two operands may set the same bit at a pixel, or the add carries into a *different* light's bit
([I-1](issues.md#i-1)). That's why the removal (F4) happens first.

## F4 · REMOVE = per-bit clear, not subtract — 2026-07-20

You can't subtract to clear bits (borrow crosses bits). **Chose** a per-bit conditional clear in a shader
RMW: for each light `i` to remove, `n -= (bit i set ? 2^i : 0)` (float-mod, ES 1.00 — the clear-bit op
the combine already uses). Done as the ping-pong read (`shadow-b`) → write (`shadow-a`) pass, so source ≠
destination. Removing the realtime set's *old* bits is what prevents a ghost when a light moves.

## F5 · The realtime set = actually-updated lights (a subset), not all-every-frame — 2026-07-20

`l-a`/`l-b` should be the lights that **changed** that frame (or a rotating subset), NOT all lights every
frame. If every light is realtime every frame, `shadow-*` (the world tier) is always emptied and the
system degenerates to pure screen-space — the world-space persistence is never exercised (and the alias
fix's whole point is moot). Drive `l` from moves (and/or rotate a subset) so both tiers do their job
([I-3](issues.md#i-3)).

## F6 · Display combines two spaces — 2026-07-20

The display ORs the persistent (world) + realtime (screen) bitfields. **Chose** to sample `shadow-*` at
the **world-aligned** UV (the existing `/overlayRT` display geometry) and `screen-shadow-*` at the
**screen** coord (`gl_FragCoord`) in one decode pass — they're in different spaces but both land at the
same on-screen pixel ([I-4](issues.md#i-4)). Both remain independently `/overlayRT`-inspectable.
