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

## The misalignment (why this is work, not just a doc)
`shadowCover` / `casterCover` model a caster as a card **leaning back**: top at `A.y − 0.5·H·cos65` with
elevation `H·sin65`. Dividing elevation by that screen-offset gives **`2·tan65` ≈ 4.3** height-per-
screen-unit — against the confirmed **`sin65` ≈ 0.91**. That's ~**4.7× apart**. The leaning card is a
*tuned projection fiction*, not our geometry, and `SHADOW_LIFT` (3 units) exists partly to paper over the
seating error it causes ([`issues.md`](issues.md)).

## Why now
[`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md) needs a receiver's **true**
elevation. If casters keep projecting on the leaning card while receivers use the confirmed model, the
two vertical frames disagree and every downstream constant gets tuned against a fiction. Align the model
first, then build prim shadows on solid ground.
