# Todo — shadow-projection (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Each phase is
verifiable in-browser against the sandbox. Spec: [`design/shadows.md`](../../components/client/pixijs/design/shadows.md);
proof: [`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html). Geo tier / no lit
render — the shadow stays a default-on debug overlay ([D-2](../webgl-engine/deviations.md#d-2))._

---

## P0 · Projection primitives (per-corner, θ-tilted, two-regime) — 2026-07-21

Replace the single-factor trapezoid math with the sandbox's `cornersWith` + `proj`.

- [ ] **Tilted billboard corners** — `cornersWith(orient, {θ,W,H,ax,ay})`: the quad tilted by ground angle
      `θ` (`R_x(θ)`), N/S additionally rolled `R_y(±90)`; anchor + centre offsets as the sandbox. Output the 4
      corners `[TL,TR,BR,BL]` in caster-local world px (z = height off ground).
- [ ] **Per-corner radial projection** — `proj(p)`: `p.z≤0 → footprint`; else `t=min(Lz/(Lz−p.z), 8)`,
      `ground = L.xy + t·(p.xy − L.xy)`. Each corner projects by ITS OWN z (today: one factor for the quad).
- [ ] **Root base to footprint** — override `BR/BL` onto the ground edge (E/W `(±W/2,0,0)`; N/S `(0,±W/2,0)`),
      `BC` = their midpoint.
- [ ] **Verify:** a single caster's projected corners `sh=[TL,TR,BR,BL]` + BC match the sandbox side/top views
      at the same `θ/Lz/W/H/light`.

## P1 · The 5-triangle fan — 2026-07-21

- [ ] Build **T1** `(TL,TR,BC)` + **T2/T3** `+depth` `(TL,BL⁺,BC)/(TR,BR⁺,BC)` + **T4/T5** `−depth`
      `(TL,BL⁻,BC)/(TR,BR⁻,BC)`, with `off(pt,s,d)` offsetting on **y** (E/W, `+`variant = `+0`) or **x** (N/S,
      symmetric). `dA`→BL, `dB`→BR.
- [ ] Emit the fan as triangle geometry (positions in world/screen px) — the unit the raster pass consumes.
- [ ] **Verify:** the fan matches the sandbox top view (solid-triangle mode) for both facings.

## P2 · Depth from presence (auto rule) — 2026-07-21

- [ ] Per **sprite-facing** presence map — alpha-threshold the silhouette (W4h's `surface.B` coverage, or the
      master alpha) into a bitmap ONCE (measure geometry, never per-frame). Cache per def+facing.
- [ ] Compute `dA/dB` = **½ · avg opaque extent over the half, scaled ×H** (E/W: avg opaque **height** of
      left/right cols; N/S: avg opaque **width** of top/bottom rows). `depth = ½·(avgExtent/TS)·H`.
- [ ] **Verify:** a round caster gets a rounded, spread base; a thin caster a narrow one — matching the
      sandbox's auto-depth.

## P3 · Rasterize the fan into the shadow field — 2026-07-21

- [ ] Replace the fullscreen per-pixel analytic loop: draw the 5-tri geometry per (light, caster) into the
      screen-space shadow field, masked to the light's channel/bit (the current per-light colour, later the
      bitfield). Batched/instanced ([F3](forks.md#f3)).
- [ ] **Verify:** shadows render as the projected fan (shape from P0–P2), still world-stuck + zoom-correct,
      overlaps combine, at acceptable cost (geometry, not per-pixel brute force).

## P4 · Alpha-masked (silhouette-shaped) triangles — 2026-07-21

- [ ] Give the fan `uv`s + sample the sprite **alpha as the shadow mask** (E/W bottom-edge UVs; N/S centerline
      + light-nearest edge, `u=0/1` by light side). A flat projected triangle → the fragment does the affine
      UV interp the sandbox's `texTri` proves. Solid-tri (P3) is the floor; this shapes the edge.
- [ ] **Verify:** the shadow reads as the caster's silhouette (needled conifer edge), not a solid polygon.

## P5 · Two-regime facing wiring — 2026-07-21

- [ ] Get the caster's **facing/rotation** (0=S,1=E,2=N,3=W) to the `ShadowCaster` so it picks E/W vs N/S +
      the roll sign — the same rotation that selects the sprite ([F1](forks.md#f1): prims carry `flipX`/`cell`,
      not rotation; add a facing field or derive).
- [ ] **Verify:** a side-facing caster and a front/back caster tilt + spread correctly (distinct regimes).

## P6 · Verify the whole model vs the sandbox — 2026-07-21

- [ ] In-browser at `?focus=100,50`: shadows match the sandbox shape for both facings across `θ/Lz/light`;
      the round-tree base spread reads; pan/zoom keep it stuck; overlaps combine; zero console errors. Then the
      triangles are ready to feed the [`shadows`](../shadows/README.md) screen-hot→world-cold bitfield.
