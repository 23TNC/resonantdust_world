# Todo — cold-data-textures (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Packed layouts are authoritative in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md) — never restate. Builds on
[`shadow-projection`](../shadow-projection/README.md) P0–P4 (the instanced fan) + delivers `caster-lut` C5's
data layer. Verify each phase in-browser at `?focus=100,50`._

**Done:** P0 — the four `RGBA32UI` layouts (`cold_light_data`, `cold_light_prim_data`, `prim_definition_data`,
`cold_prim_data`) + `position_anchor_reference` are authored + byte-checked in
[`VARIABLES.md`](../../VARIABLES.md); forks F1–F5 resolved ([`forks.md`](forks.md)).

---

## P1 · `prim_definition_data` + atlas integration — 2026-07-21

The generic, shared, longest-lived tier — stand it up first (the shader needs frames + page).

- [ ] A `prim_definition_data` `RGBA32UI` texture (1 px/sprite variant) + a `definition_index` allocator keyed
      by sprite stem+variant (a variant → one slot, shared by all its instances). On **atlas add**
      (`TextureResolver.packInto` / `LodPool.add`) write `prim_width/height` + atlas `frame x/y/w/h` +
      `frame_page`; `texSubImage2D` only the changed px. No eviction (mirrors the atlas).
- [ ] **Verify:** read back a few texels — W/H + frame + page correct for the resident variants.

## P2 · `cold_prim_data` — the placed-caster instances — 2026-07-21

- [ ] A `cold_prim_data` `RGBA32UI` texture (2 entries/px) + a `prim_data_index` allocator per placed standing
      prim. Write each caster's `position_anchor_reference` + `z` + `rotation`. Moving a caster updates **one**
      texel (`texSubImage2D`); no LUT edit.
- [ ] **Verify:** read back an instance — position/rotation match the prim; a moved prim updates one entry.

## P3 · `cold_light_data` + `cold_light_prim_data` (the LUT) — 2026-07-21

- [ ] `cold_light_data` (1 px/light) written on seed/change, NOT per frame: position, colour+intensity,
      radius+z, and the `(lut_index, lut_count)` run. `cold_light_prim_data` = each light's in-range casters as
      a contiguous run of `(definition_index, prim_data_index)` (4/px). Patch a run only on a **radius
      crossing** ([F5](forks.md#f5)); full rebuild on bulk (zone stream-in).
- [ ] **Verify:** a light's run lists the right casters; a spot shadowed by two lights appears in both runs; no
      per-frame light/LUT upload.

## P4 · The shader reads the four textures (replace instance attrs) — 2026-07-21

- [ ] Rework the shadow cast: the vertex shader `texelFetch`es, per **per-light draw** (`instanceCount =
      lut_count`, the draw's light = `cold_light_data[k]`), the LUT entry → `prim_definition_data[def]`
      (geometry + `frame_page`) + `cold_prim_data[inst]` (position + rotation); **decode**
      `position_anchor_reference` → px via the `*_DIM` constants; run the SAME `cornersWith`/`proj`/fan
      (`shadow-projection` P0–P4). Add the **radius safety check** (rectangle distance) so a stale LUT still
      culls. The alpha mask samples the atlas via the def's `frame` + `frame_page`.
- [ ] **Verify:** shadows render identically to the instance-attr path, but with **no per-frame caster/light
      upload** — only the texture writes on change; frame cost drops.

## P5 · Rotation → shadow regime (unblock shadow-projection P5) — 2026-07-21

- [ ] `cold_prim_data.rotation` drives the E/W vs N/S regime in the vertex shader — the data
      [`shadow-projection` P5](../shadow-projection/blockers.md#b-1) was blocked on. Wire the N/S branch (from
      `design/shadows.md`). (Live verification still needs an N/S caster — a mover / multi-facing thing.)

## P6 · Verify the whole cold-data system — 2026-07-21

- [ ] In-browser at `?focus=100,50`: shadows match the instance-attr result; pan/zoom stuck; light/LUT textures
      write only on change, `prim_definition_data` only on atlas add; per-frame CPU is just the draw. Read back
      a texel of each texture to confirm the packing matches [`VARIABLES.md`](../../VARIABLES.md). `caster-lut`
      C5's data layer, live.
