# Work — cold-data-textures (cold lights + prims as GPU data textures)

_Opened 2026-07-21. Move the **cold (static) light + caster data off per-frame instance attributes and into
persistent GPU data textures** read by the shader (VTF / `texelFetch`), so the shadow cast doesn't re-upload
static data every frame. This is the data layer of **`caster-lut` C5** ([webgl-engine W7](../webgl-engine/README.md))
and executes [`shadow-projection` F3 sub-choice (b)](../shadow-projection/forks.md#f3) (the data texture over
instance attributes). Component: `client/webgl`. The packed layouts are authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md) (never restated here)._

## The idea (the user's design, resolved 2026-07-21)

**Four** normalized `RGBA32UI` (128-bit, integer, `texelFetch`) data textures with a light → LUT →
definition / instance indirection. Exact bit layouts are authoritative in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md); in brief:

- **`cold_light_data`** — one px/light: position (`position_anchor_reference`), colour+intensity, `radius`+`z`,
  a **`cast_shadows`** flag (0 → lights without casting: no LUT run, skipped in the cast), and a **run**
  `(lut_index, lut_count)` into the LUT = this light's shadow casters.
- **`cold_light_prim_data`** (the LUT) — the light → caster association, **indices only**: each entry =
  `(definition_index, prim_data_index)`. A light's casters are the contiguous run `[lut_index, +lut_count)`.
- **`prim_definition_data`** — one px per sprite **variant**: **generic** geometry (billboard `width/height` +
  atlas `frame x/y/w/h` + `frame_page`), shared by every instance of that sprite. Written on **atlas add**;
  evicted only if the atlas evicts (it doesn't yet). ~16 px for a conifer's variants; a spare `u32` for
  materials later.
- **`cold_prim_data`** — one entry per **placed** caster instance: its position + `z` + `rotation` (n/e/s/w).

**Position** = `position_anchor_reference` (`region | zone | tile | anchor`, each `u8 = x:4|y:4`) — a full
spatial address at `SQUARE/16` sub-tile resolution; no world-size cap.

**Why this shape is right — the normalization is the key move.** Cold lights don't move and casters are static,
so the data is written **once** and read every frame for free (no per-frame CPU→GPU upload). Splitting the
per-instance position (`cold_prim_data`) from the light→caster association (the LUT) and the generic geometry
(`prim_definition_data`) means a caster's position lives in **one** place: **moving it updates one texel**, and
every light referencing it (via the index) sees the new position — instead of editing every light's run. This
is `caster-lut`'s light → LUT → prim model, normalized, as GPU textures. It also **unblocks
[`shadow-projection` P5](../shadow-projection/blockers.md#b-1)** — `cold_prim_data.rotation` picks the E/W vs
N/S regime.

## Relationship to what exists

[`shadow-projection`](../shadow-projection/README.md) P0–P4 already cast the GPU-instanced silhouette fan, but
rebuilds the per-(light,caster) **instance attributes every frame** on the CPU. This stream replaces that
upload with the four persistent textures: the same instanced draw, but the vertex shader `texelFetch`es the
light + LUT + prim data by index instead of reading re-uploaded attributes. The projection/fan math is
unchanged.

## Assessment (feedback, per the request)

**This is a good, standard design — not a bad implementation.** The instinct ("load data in textures visible
to the shader so we don't pass cold data every frame") is exactly how scalable GPU-driven rendering works:
`texelFetch` on an integer texture is an exact by-index read; the texture lives in GPU memory and is updated
(`texSubImage2D`) only on change, so per-frame there's zero transfer. The 128-bit `RGBA32UI` packing is clean —
every layout fits its pixel exactly — and the four-table **normalization** (position in `cold_prim_data`, not
repeated in the LUT) is the right move: one texel to move a caster, not every light's run. The open items are
all **resolved** ([`forks.md`](forks.md)): F1 the `region|zone|tile|anchor` address (no world cap), F2
`frame_page` in the full-px prim def, F3 per-light draw (the association is inherent), F4 debug lights now /
DSL later, F5 the normalization + a shader radius safety check (rectangle distance). Proceed.

## Scope

In: the four data-texture definitions (in `VARIABLES.md`) + their builders, the atlas→prim-texture update,
writing cold lights + LUT on zone/light change, and the shadow shader reading them via `texelFetch` (replacing
the per-frame instance attrs). Out: DSL-authored cold lights (F4), atlas eviction, incremental LUT patching,
the world-cold bitfield persistence (the `shadows` stream).
