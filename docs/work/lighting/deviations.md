# Deviations — lighting

_Plan-deviations logged at the moment they happen. "Less churn" is never a reason (see the
`deviations-file` memory). Format: what the plan said → what we did → why._

---

## D-1 · Cold shadows: a materialized `shadow-cold` map (interim), not the inline sweep — 2026-07-19

**Design** ([intent/tiered-lighting.md](../../components/client/pixijs/intent/tiered-lighting.md),
[F7](forks.md#f7)): cold occlusion is an **inline per-pixel sweep** in the bake — no shadow map,
unbounded shadow-casting lights.

**Code:** the built path **materializes** a `shadow-cold` `SquareCache` composite — `bakeColdShadowSquare`
projects the ≤3 nearest cold lights' silhouettes (`projectCaster`) into R/G/B lanes, and the cold lightmap
bake samples + subtracts them. Reuses `projectCaster` (the shared cold+dynamic shear) + the existing
derived-composite machinery, which got cold shadows working end-to-end fast.

**The gap:** the 3-lane RGBA cap limits it to **3 shadow-casting cold lights per square** — the exact
limit the inline sweep exists to remove (a gather has no channel cap; the cap is a *rasterized-scatter*
constraint, dynamic-only). Interim only: replaced by the inline sweep in [`todo.md`](todo.md) P4, gated on
[B3](blockers.md#b3). Live-verified working in the meantime.

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
