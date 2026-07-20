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

---

## D-3 · Whole interim lighting/shadow build removed ("nuke and try again") — 2026-07-19

**Plan** ([`todo.md`](todo.md)/[`completed.md`](completed.md)): iterate the tiered build forward from the
3-lane interim (P2 bitfield re-attempt, P5 stopgap retire, etc.).

**What we did:** at the user's direction, DELETED the entire lighting + shadow stack instead — `LightRig`,
`lightingShader`, `lightingBakeShader`, `shadowPass`, `scatterShader`, `projectCaster`,
`warmCombineShader`, `coldLightTex`, the cold `lightmap`/`shadow` channels + their bakes, the cursor
light/shadow, and `OutlineCache`. The viewport display is now a new **unlit** `albedoBlitShader` that
still composites warm-over-cold (pawns draw). The G-buffer bakes (albedo/normal/surface/zdepth in
`SquareCache`) were **kept** as the retry foundation. Build + typecheck green; verified in-browser
(flat albedo terrain renders, no black, no console errors).

**Why:** the tiered port failed to land a working result; the user chose to reset to a clean unlit
base and re-plan the lighting from scratch rather than keep iterating the broken interim. The **Why** +
**Architecture** (README) and intent/design docs stay as the target; the phased `todo`/`completed`
here describe the reverted attempt and are pending re-planning (see the README status banner).
