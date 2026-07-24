# World geometry — one model, code aligned — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` (and anywhere else a geometric
assumption leaks). Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md)._

Establish **ONE canonical geometric model** of our world, write it down, and conform the code to it. The
model below was derived with the user and **confirmed against their simulation**. The shadow projection
currently runs on a *different* (tuned, leaning-card) fiction — so "what is our geometry" has no single
answer in the codebase today, which is exactly what this stream fixes.

## The model (ratified 2026-07-23 — user + simulation)
- **Billboards are drawn PARALLEL to the view plane** — which is why they appear as rectangles.
- A billboard makes **65°** with the ground ⟹ **the ground makes 65° with the view plane**.
- We **never define a sprite's distance to the view plane** — everything is projected onto it. So each
  sprite effectively carries its own ground, and a "height" is only meaningful **relative to a chosen
  ground** (normally a prim's base).
- **Fictional height:** `z = sin(65°) · (px.y − base)`, per prim, against **that prim's own base**.
  Below the base → **negative** (you're not on that prim). Measuring against another prim's base is
  measuring against a different ground, and gives the wrong height for this one.
- **Shadow projection parameter** (ray light→point, extended to the ground): `s = z_light / (z_light −
  z_point)` — pure heights, therefore **pure y**.
- **x is separable.** The entire height/y chain is x-free; the shadow's x is a parallel interpolation
  `G.x = light.x + s·(prim.x − light.x)` reusing the *same* `s`. One-way: **x never influences y**.
- **Left/right y-equality (simulation-confirmed):** the y result is identical on the left and right of a
  billboard ⟹ **one y calculation per billboard**. Project only the **top-left / top-right x**; the
  **bottom two points are just the billboard's bottom corners**. That is the whole quad.

## Audit result (P1) — the model is already mostly aligned
The P1 audit ([`issues.md#i4`](issues.md#i4)) found the codebase is **more aligned than first claimed**:
- **Elevation ALREADY matches.** `shadowCover`'s caster top elevation `Zt = H·sin65` is exactly the
  ratified `sin65·Δ` for a drawn height `H`. Elevation-per-drawn-offset is `sin65` in both.
- **The one open item is the north offset.** The card places the top at `0.5·H·cos65` north; a strictly
  parallel-to-view billboard is `H·cos65` (a `0.5` factor). That `0.5` may be the shadow-design "wedge"
  `±depth` half — i.e. **deliberate, not a bug** — so it's a design call ([forks F2](forks.md#f2)).
- **`SHADOW_LIFT` is orthogonal** — it seats the shadow on the sprite's *visible* base past its transparent
  padding ([`issues.md#i2`](issues.md#i2)); it is NOT a card symptom and won't go to 0.

**The original "~4.7× misalignment" was a misread** — I divided elevation by the *north* offset and
compared it to elevation-per-*drawn*-offset (two different ratios). Retracted; see
[`issues.md#i1`](issues.md#i1) / [#i5](issues.md#i5).

## What this leaves
Much smaller than first framed. **P0 (write the model down + name the tilt once) still stands** — the
"firm understanding" is the point, and the model doc is worth having regardless. The only real *decision*
is [F2](forks.md#f2): keep the wedge's `0.5` north offset or conform to strict parallel-to-view — a look
call, testable in-browser, low risk (elevation unchanged). And because the **elevation already matches**,
[`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md) does **not** have to wait on a
big conform — its receiver elevation shares the caster's `sin65` frame today ([forks F5](forks.md#f5)).
