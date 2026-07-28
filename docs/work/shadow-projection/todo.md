# Todo — shadow-projection (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Each phase is
verifiable in-browser against the sandbox. Spec: [`design/shadows.md`](../../components/client/webgl/design/shadows.md);
proof: [`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html). The fan is built
**on the GPU** — an instanced vertex shader, NOT CPU vertex buffers (delivers `caster-lut` C5 /
[webgl-engine W7](../webgl-engine/todo.md)). Geo tier / no lit render — the shadow stays a default-on debug
overlay ([D-2](../webgl-engine/deviations.md#d-2))._

---

**Done (in [`completed.md`](completed.md)):** P0 GLSL projection primitives · P1 the GPU-instanced
5-triangle fan · P2 the caster+light data channel — the GPU projected fan renders (E/W, constant θ,
rough depth, solid tris). Remaining phases refine the shape + blending:

**Done (in [`completed.md`](completed.md)):** P0 primitives · P1 the GPU fan · P2 data channel · P3
depth-from-presence · P4 the alpha mask. **The stream's core is delivered** — GPU-instanced,
silhouette-masked projected fans matching the sandbox. Remaining are non-core refinements:

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
