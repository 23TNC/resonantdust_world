# Todo — shadow-projection (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Each phase is
verifiable in-browser against the sandbox. Spec: [`design/shadows.md`](../../components/client/pixijs/design/shadows.md);
proof: [`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html). The fan is built
**on the GPU** — an instanced vertex shader, NOT CPU vertex buffers (delivers `caster-lut` C5 /
[webgl-engine W7](../webgl-engine/todo.md)). Geo tier / no lit render — the shadow stays a default-on debug
overlay ([D-2](../webgl-engine/deviations.md#d-2))._

---

## P0 · GLSL projection primitives (vertex-shader math) — 2026-07-21

Port the sandbox's `cornersWith` + `proj` into a **vertex shader** (not CPU functions).

- [ ] **Tilted billboard corner (GLSL)** — given a corner uv `(u,v)` + caster `{θ,W,H,ax,ay,facing,roll}`,
      build its caster-local world-px position (`R_x(θ)`; N/S rolled `R_y(±90)`), with the anchor/centre
      offsets, exactly as `cornersWith`. Output `z` = height off ground.
- [ ] **Per-corner radial projection (GLSL)** — `proj(p, L)`: `p.z≤0 → footprint`; else
      `t=min(Lz/(Lz−p.z), 8)`, `ground = L.xy + t·(p.xy − L.xy)`. **Each corner projects by ITS OWN z** (today:
      one factor for the whole quad — the key delta).
- [ ] **Root base to footprint (GLSL)** — the `BR/BL` roles emit the ground edge (E/W `(±W/2,0)`; N/S
      `(0,±W/2)`); `BC` = their midpoint.
- [ ] **Verify:** a single-instance test draw (one caster, one light, corners as points/lines) matches the
      sandbox side/top views at the same `θ/Lz/W/H/light`.

## P1 · The GPU-instanced 5-triangle fan — 2026-07-21

- [ ] **Instanced draw** `drawArraysInstanced(TRIANGLES, 0, 15, N)` — 5 triangles = 15 vertices per instance,
      one instance per (light, caster). A `gl_VertexID`→(triangle, corner-role) table in the vertex shader
      selects which fan vertex to emit: **T1** `(TL,TR,BC)` · **T2/T3** `+depth` `(TL,BL⁺,BC)/(TR,BR⁺,BC)` ·
      **T4/T5** `−depth` `(TL,BL⁻,BC)/(TR,BR⁻,BC)`. `off(±depth)` on **y** (E/W, `+`variant=`+0`) or **x** (N/S,
      symmetric); `dA`→BL, `dB`→BR. The vertex runs P0's primitives then applies the role's ±depth.
- [ ] World→clip via the viewport's `uProjection` (same as the display), so the fan is world-stuck + zoomed.
- [ ] **Verify:** the solid fan (single caster, driven by uniforms) matches the sandbox top view (solid mode),
      both facings; pan/zoom keep it stuck.

## P2 · Caster + light data channel — 2026-07-21

Feed the many (light, caster) instances without CPU-building geometry.

- [ ] Per-instance data — caster `x,y,W,H,θ,facing,roll,dA,dB` + atlas frame `(u0,v0,uw,vh)` + light
      `Lx,Ly,Lz` + the light's bit/colour — via **instance vertex attributes** or a **`caster-lut` data
      texture** (`RGBA32F`, VTF/`texelFetch`) ([F3](forks.md#f3)).
- [ ] The **CPU builds only the in-range (light, caster) pair list** (the `caster-lut` LUT — a cheap index
      cull, `hypot ≤ radius`), not geometry. `N` = pair count.
- [ ] **Verify:** all in-range casters cast from all lights, correct instance count; the per-frame CPU cost is
      just the pair list (the projection/geometry is entirely GPU).

## P3 · Depth-from-presence bake (one-time, feeds GPU data) — 2026-07-21

The only CPU pixel work — a per-sprite-facing bake, NOT per frame.

- [ ] Per **sprite-facing** presence map — threshold the silhouette (W4h's `surface.B` coverage, or master
      alpha) into a bitmap ONCE. Cache per def+facing.
- [ ] `dA/dB` = **½ · avg opaque extent over the half, ×H** (E/W: opaque **height** of left/right cols; N/S:
      opaque **width** of top/bottom rows); `depth = ½·(avgExtent/TS)·H`. Upload into the caster data (P2).
- [ ] **Verify:** a round caster gets a rounded, spread base; a thin one a narrow base — matching the
      sandbox's auto-depth.

## P4 · Alpha-masked (silhouette-shaped) fragment — 2026-07-21

- [ ] The vertex shader assigns `uv`s (E/W bottom-edge `TL(0,0) TR(1,0) BL±(0,1) BR±(1,1) BC(.5,1)`; N/S base
      = centerline `u=.5`, tip = the sprite edge nearest the light `u=0/1` by light side). The **fragment
      samples the sprite alpha** (atlas frame from the instance) as the shadow mask → discard outside. This is
      exactly the sandbox's affine `texTri`, now a real fragment.
- [ ] **Verify:** the shadow reads as the caster's silhouette (needled conifer edge), not a solid polygon.

## P5 · Two-regime facing wiring — 2026-07-21

- [ ] Get each caster's **facing/rotation** (0=S,1=E,2=N,3=W) into the instance data so the vertex shader
      picks E/W vs N/S + the roll sign — the same rotation that selects the sprite ([F1](forks.md#f1): prims
      carry `flipX`/`cell`, not rotation; add a facing field or derive).
- [ ] **Verify:** a side-facing caster and a front/back caster tilt + spread correctly (distinct regimes).

## P6 · Verify the whole GPU model vs the sandbox — 2026-07-21

- [ ] In-browser at `?focus=100,50`: shadows match the sandbox shape for both facings across `θ/Lz/light`, the
      round-tree base spread reads, pan/zoom stay stuck, overlaps combine, zero console errors — all from the
      instanced GPU cast (no per-frame CPU geometry). The instanced VTF cast **is** `caster-lut` C5 on the owned
      engine; the fan is ready to feed the [`shadows`](../shadows/README.md) screen-hot→world-cold bitfield.
