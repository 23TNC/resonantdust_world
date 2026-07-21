# Todo — shadow-projection (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Each phase is
verifiable in-browser against the sandbox. Spec: [`design/shadows.md`](../../components/client/pixijs/design/shadows.md);
proof: [`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html). The fan is built
**on the GPU** — an instanced vertex shader, NOT CPU vertex buffers (delivers `caster-lut` C5 /
[webgl-engine W7](../webgl-engine/todo.md)). Geo tier / no lit render — the shadow stays a default-on debug
overlay ([D-2](../webgl-engine/deviations.md#d-2))._

---

**Done (in [`completed.md`](completed.md)):** P0 GLSL projection primitives · P1 the GPU-instanced
5-triangle fan · P2 the caster+light data channel — the GPU projected fan renders (E/W, constant θ,
rough depth, solid tris). Remaining phases refine the shape + blending:

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
