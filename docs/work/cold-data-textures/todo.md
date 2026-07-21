# Todo — cold-data-textures (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Packed layouts are authoritative in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md) — never restate. Builds on
[`shadow-projection`](../shadow-projection/README.md) P0–P4 (the instanced fan) + delivers `caster-lut` C5's
data layer. Verify each phase in-browser at `?focus=100,50`._

**Done:** P0 (layouts, byte-checked; forks F1–F6 resolved) · P1 `prim_definition_data` + atlas write · P2
`cold_prim_data` + position codec · P3 `cold_light_data` + LUT · P4 **the shader reads all four textures**
(no instance attrs — the payoff). All readback-verified; shadows render from the textures. The **data-driven
cast is live** ([`completed.md`](completed.md)). Remaining is content-gated / a cross-check:

---

## P5 · Rotation → shadow regime (unblock shadow-projection P5) — 2026-07-21

- [ ] `cold_prim_data.rotation` drives the E/W vs N/S regime in the vertex shader — the data
      [`shadow-projection` P5](../shadow-projection/blockers.md#b-1) was blocked on. Wire the N/S branch (from
      `design/shadows.md`). (Live verification still needs an N/S caster — a mover / multi-facing thing.)

## P6 · Verify the whole cold-data system — 2026-07-21

- [ ] In-browser at `?focus=100,50`: shadows match the instance-attr result; pan/zoom stuck; light/LUT textures
      write only on change, `prim_definition_data` only on atlas add; per-frame CPU is just the draw. Read back
      a texel of each texture to confirm the packing matches [`VARIABLES.md`](../../VARIABLES.md). `caster-lut`
      C5's data layer, live.
