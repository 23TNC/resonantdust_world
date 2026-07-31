# Todo — cold-data-textures (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Packed layouts are authoritative in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md) — never restate. Builds on
`shadow-projection` P0–P4 (the instanced fan) + delivers `caster-lut` C5's
data layer. Verify each phase in-browser at `?focus=100,50`._

**Done:** P0 (layouts, byte-checked; forks F1–F6 resolved) · P1 `billboard_definition_data` + atlas write · P2
`cold_billboard_data` + position codec · P3 `cold_light_data` + LUT · P4 **the shader reads all four textures**
(no instance attrs — the payoff) · P6 whole-system cross-check. All readback-verified; shadows render from
the textures; the **data-driven cast is delivered** ([`completed.md`](completed.md)). The ONE remaining item
is content-gated:

---

## P5 · Rotation → shadow regime (unblock shadow-projection P5) — 2026-07-21

- [ ] `cold_billboard_data.rotation` drives the E/W vs N/S regime in the vertex shader — the data
      `shadow-projection` P5 was blocked on. Wire the N/S branch (from
      `design/shadows.md`). (Live verification still needs an N/S caster — a mover / multi-facing thing.)
