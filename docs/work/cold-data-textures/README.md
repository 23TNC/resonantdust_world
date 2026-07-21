# Work — cold-data-textures (cold lights + prims as GPU data textures)

_Opened 2026-07-21. Move the **cold (static) light + caster data off per-frame instance attributes and into
persistent GPU data textures** read by the shader (VTF / `texelFetch`), so the shadow cast doesn't re-upload
static data every frame. This is the data layer of **`caster-lut` C5** ([webgl-engine W7](../webgl-engine/README.md))
and executes [`shadow-projection` F3 sub-choice (b)](../shadow-projection/forks.md#f3) (the data texture over
instance attributes). Component: `client/webgl`. The packed layouts are authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md) (never restated here)._

## The idea (the user's design)

Three `RGBA32UI` (128-bit, integer, `texelFetch`) data textures + a light→LUT→prim **indirection**:

- **cold-light** — one px per light: position, colour+intensity, radius+height, and a **run** `(lut_index,
  lut_count)` into the LUT = this light's shadow-casting casters.
- **caster-LUT** — 2 caster refs per px: each ref = a caster **instance** (position, `z`, **rotation** =
  n/e/s/w → the shadow regime) + a `prim_index` into the prim texture.
- **prim** — 2 prims per px: **generic** per-sprite geometry (billboard `width/height` + the atlas
  `frame x/y/w/h`), shared by every caster using that sprite. Written when a sprite is **added to the atlas**,
  evicted only if the atlas evicts it (which we don't do yet). Exact bit layouts: [`VARIABLES.md`](../../VARIABLES.md).

**Why it's the right shape:** cold lights don't move and casters are static, so their data is written **once**
(on zone load / atlas add) and read every frame by the shader for free — no per-frame CPU→GPU upload. The
indirection deduplicates: many casters share one `prim` entry; a light names a contiguous LUT run instead of
repeating caster geometry. This is exactly `caster-lut`'s light → LUT → prim model, now as GPU textures. It
also **unblocks [`shadow-projection` P5](../shadow-projection/blockers.md#b-1)** — the LUT carries `rotation`,
which is what picks the E/W vs N/S regime.

## Relationship to what exists

[`shadow-projection`](../shadow-projection/README.md) P0–P4 already cast the GPU-instanced silhouette fan, but
rebuilds the per-(light,caster) **instance attributes every frame** on the CPU. This stream replaces that
upload with the three persistent textures: the same instanced draw, but the vertex shader `texelFetch`es the
light + LUT + prim data by index instead of reading re-uploaded attributes. The projection/fan math is
unchanged.

## Assessment (feedback, per the request)

**This is a good, standard design — not a bad implementation.** The instinct ("load data in textures visible
to the shader so we don't pass cold data every frame") is exactly how scalable GPU-driven rendering works:
`texelFetch` on an integer texture is an exact by-index read; the texture lives in GPU memory and is updated
(`texSubImage2D`) only on change, so per-frame there's zero transfer. The 128-bit `RGBA32UI` packing is clean
and every one of the three layouts fits its pixel exactly. The light→LUT→prim indirection is the correct way
to avoid repeating geometry. Proceed — the open items below are details to pin, not flaws (see
[`forks.md`](forks.md)):

- **Coordinate range ([F1](forks.md#f1)).** `u16` x/y = 0..65535 px ≈ 1024 tiles ≈ 64 zones. A larger world
  overflows it → make x/y **zone-relative** (a zone is 16 tiles = 1024px, fits `u16` with room), the natural
  scope for cold/zone data, adding the zone origin at read time. Same for `u8 z` (255) and `u8 radius` (255):
  pick units so they don't clip (radius in **tiles**; z coarser or zone-relative — my debug `Lz`=480 overflows
  `u8`-as-px).
- **Atlas frame needs a PAGE id ([F2](forks.md#f2)).** `u10` frame coords fit a 1024² page, but the large-LOD
  pool uses 2048², and the layout carries **no page/texture identifier** — the shader can't know which atlas
  page to sample. Either commit shadow-casting sprites to one 1024² page, or add a page index (spend some
  `reserved` bits) + bind an atlas texture array.
- **Light ↔ LUT-entry association for the draw ([F3](forks.md#f3)).** A LUT ref doesn't carry its light, but
  the draw needs each caster-instance to know its light. Draw **per light** (`instanceCount = lut_count`,
  `lightIndex` uniform, read `LUT[lut_index + gl_InstanceID]`), or store a `light_index` in the LUT ref's `u6`
  reserved and draw all entries at once.
- **LUT rebuild on caster-set change ([F5](forks.md#f5)).** Contiguous per-light runs mean adding a caster to
  light k shifts every later run — a full LUT rebuild. Fine for **cold** (rare changes); note it, incremental
  later.
- **Light source ([F4](forks.md#f4)).** These are still the 6 debug-seeded lights written INTO the texture;
  real DSL-authored cold lights are a separate, later source that fills the same texture.

## Scope

In: the three data-texture definitions (in `VARIABLES.md`) + their builders, the atlas→prim-texture update,
writing cold lights + LUT on zone/light change, and the shadow shader reading them via `texelFetch` (replacing
the per-frame instance attrs). Out: DSL-authored cold lights (F4), atlas eviction, incremental LUT patching,
the world-cold bitfield persistence (the `shadows` stream).
