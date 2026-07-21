# Todo — cold-data-textures (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. The packed layouts
are authoritative in [`docs/VARIABLES.md`](../../VARIABLES.md) — define them there, never restate. Builds on
[`shadow-projection`](../shadow-projection/README.md) P0–P4 (the instanced fan) + delivers `caster-lut` C5's
data layer. Verify each phase in-browser at `?focus=100,50`._

---

## P0 · Define the three layouts in VARIABLES.md — 2026-07-21

- [ ] Resolve [F1](forks.md#f1) (coord space + z/radius units) first — it fixes the field meanings. Then
      author the **cold-light**, **caster-LUT**, and **prim** `RGBA32UI` layouts in
      [`docs/VARIABLES.md`](../../VARIABLES.md) (the authoritative home for cross-component packed layouts):
      each channel's bit fields, units, and packing. This is the contract every builder + the shader read from.

## P1 · Prim data texture + atlas integration — 2026-07-21

The generic, shared, longest-lived tier — stand it up first (the shadow shader already needs frames).

- [ ] A `prim` `RGBA32UI` texture (2 prims/px) + a `prim_index` allocator keyed by sprite stem (a stem →
      one slot, shared by all its casters). On **atlas add** (`TextureResolver.packInto` / `LodPool.add`),
      write the sprite's `prim_width/height` + the atlas `frame x/y/w/h` into its slot ([F2](forks.md#f2): the
      page identifier). `texSubImage2D` only the changed pixel. No eviction (mirrors the atlas).
- [ ] **Verify:** the prim texture holds correct W/H + frame for the resident sprites (read back a few texels).

## P2 · Cold light data texture — 2026-07-21

- [ ] A `cold-light` `RGBA32UI` texture (1 px/light). Write the current lights once (on seed / change), NOT per
      frame: position, colour+intensity, radius+z, and the `(lut_index, lut_count)` run (filled by P3).
- [ ] **Verify:** the shader `texelFetch`es a light + reproduces the current cast (lights render identically to
      the instance-attr path) with no per-frame light upload.

## P3 · Caster-LUT data texture — 2026-07-21

- [ ] A `caster-LUT` `RGBA32UI` texture (2 refs/px). For each light, cull its in-range casters (the cheap CPU
      pairing, as today) into a **contiguous run**; write each ref (position, `z`, `rotation`, `prim_index`)
      and record the run into the light's `(lut_index, lut_count)`. Rebuild on caster-set change
      ([F5](forks.md#f5)); no per-frame rebuild for static casters.
- [ ] **Verify:** each light's run lists the right casters; a spot shadowed by two lights appears in both runs.

## P4 · The shader reads the textures (replace instance attrs) — 2026-07-21

- [ ] Rework the shadow cast so the vertex shader `texelFetch`es light + LUT + prim by index instead of reading
      per-frame instance attributes: the draw enumerates (light, LUT-entry) instances ([F3](forks.md#f3):
      per-light draws or a `light_index` in the LUT), reads the caster ref + its `prim` geometry + the owning
      light, and runs the SAME `cornersWith`/`proj`/fan (`shadow-projection` P0–P4 unchanged). The alpha mask
      samples the atlas via the prim `frame`.
- [ ] **Verify:** shadows render identically to the instance-attr path, but with **no per-frame caster/light
      upload** — only the texture writes on change. Frame cost drops (no per-frame instance buffer rebuild).

## P5 · Rotation → shadow regime (unblock shadow-projection P5) — 2026-07-21

- [ ] The LUT ref's `rotation` (n/e/s/w) now drives the E/W vs N/S regime in the vertex shader — this is the
      data [`shadow-projection` P5](../shadow-projection/blockers.md#b-1) was blocked on. Wire it; the N/S
      branch (from `design/shadows.md`) drops into the same shader. (Verification still needs an N/S caster —
      a mover or multi-facing thing; the data path is ready regardless.)

## P6 · Verify the whole cold-data system — 2026-07-21

- [ ] In-browser at `?focus=100,50`: shadows match the instance-attr result; panning/zoom stay stuck; the
      light/LUT textures write only on change (cold), the prim texture only on atlas add; per-frame CPU is just
      the (unchanged) draw. Read back a texel of each texture to confirm the packing matches
      [`VARIABLES.md`](../../VARIABLES.md). This is `caster-lut` C5's data layer, live.
