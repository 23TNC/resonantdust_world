# Deviations — lighting

_Plan-deviations logged at the moment they happen. "Less churn" is never a reason (see the
`deviations-file` memory). Format: what the plan said → what we did → why._

---

## D-1 · Cold shadow is an RGB=3 map; the design is a 32-bit bitfield — 2026-07-19

**Design** ([intent/tiered-lighting.md](../../components/client/pixijs/intent/tiered-lighting.md),
[F7](forks.md#f7)): `shadow-cold` is a **32-bit occlusion bitfield** — up to **32** cold shadow-casters
per rect, read by the bake via `bf_bit`.

**Code:** the built `bakeColdShadowSquare` projects the ≤3 nearest cold lights into **R/G/B lanes** of a
`shadow-cold` composite; the bake samples RGB (`i==0?r : i==1?g : i==2?b : 0`), so only **3** cast a shadow.

**The gap:** RGB=3 vs the design's 32. Interim only — [`todo.md`](todo.md) P2 swaps the RGB sample for the
bitfield (keeping the `projectCaster`/scatter machinery, now writing bits instead of lanes). Live-verified
working in the meantime.

---

## D-2 · `shadowPass.nsProject` uses the wrong roll axis vs the converged model — 2026-07-19

**Design** ([`design/shadows.md`](../../components/client/pixijs/design/shadows.md)): an N/S billboard
rolls **`R_y(±90)·R_x(θ)`** (edge-on, `worldX=0`, rooted on the centerline) before projecting.

**Code:** `shadowPass.ts` `nsProject` still uses a `θ`-on-X/Z axis (`worldX = eCenter + off·cosθ;
z = |off|·sinθ`), a rotation about **Y**, not the roll-then-tilt above. Also still carries the older
"wedge" silhouette from [`lighting.md` §D](../../components/client/pixijs/design/lighting.md), and
does not yet sample the sprite via the per-triangle UVs the model specifies.

**Why (still open):** the model converged in the sandbox *after* the current `shadowPass` was written;
the code predates it. Not yet reconciled — flagged so the geometry is ported deliberately (not the
stale axis) when the shadow pass is next touched. Fix: port the 5-triangle fan + presence-derived
depths + UV sampling from `design/shadows.md`.
