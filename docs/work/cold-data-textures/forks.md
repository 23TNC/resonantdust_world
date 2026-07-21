# Forks — cold-data-textures

_Decision points + options + which we chose + why. All resolved 2026-07-21 in the design pass with the user._

---

## F1 · Coordinate space + z/radius units — 2026-07-21 (resolved)

**Decided:** a full **spatial address + sub-tile anchor** — `u32 position_anchor_reference` =
`region_reference | zone_reference | tile_reference | anchor_reference` (each `u8 = x:4 | y:4`), reusing the
existing `position_reference` shape (§Where it is / [`VARIABLES.md`](../../VARIABLES.md)) with `anchor`
replacing the low `layer` byte. Min unit `SQUARE/16` (tile/16); `anchor=(8,8)` centres a prim. Kills the world
cap entirely (no `u16`-px ceiling) and matches the existing addressing + `shared/codec/object` helpers. Units:
**one unit for everything world-space** — `1 unit = SQUARE/16 = 4px`, a **compile-time constant** (`TILE = 16
units`, derivable; no per-frame scale uniform). `anchor` (u4) = 0..15 units = one tile; `z` (u8) =
0..255 units ≈ 16 tiles; `radius` (**u12**) = 0..4095 units ≈ **16 zones** (256 tiles, far past the ~8-zone
view); `prim_width/height` (u10) ≈ 64 tiles. The shader works in units, converting to px (×4) only at the clip
transform. The **only** non-unit fields are the atlas `frame_*` (texture px, they index the atlas). For true
global ambient, a **no-cull flag** beats a max radius.

## F2 · The atlas frame page identifier — 2026-07-21 (resolved)

**Decided:** grow `prim_definition_data` to a **full `RGBA32UI` px** (was 2 prims/px) and spend the new space
on `frame_page` (`u10`) + reserved (`u22` + `u32`, for materials later). Defs are **shared per sprite variant**
(~16 px for a conifer's variants, referenced by hundreds of instances), so the extra 64 bits/def is nearly
free. Resolves the multi-page atlas gap — the shader knows which page to sample. No cap on distinct caster
sprites.

## F3 · Light ↔ LUT-entry association for the draw — 2026-07-21 (resolved; my framing was wrong)

**Decided:** per-light draw — the association is **inherent**, not a problem. Each light owns a contiguous LUT
run (`lut_index, lut_count`); drawing light k iterates ITS run, so the light index IS the draw. The shader
fetches `cold_light_data[k]` for the light's position + `rgb`/`intensity` (the shadow's debug colour), and the
LUT names the caster. No `light_index` in the LUT.

## F4 · The cold-light source — 2026-07-21 (resolved)

**Decided:** lights are still **hand-authored debug lights** (lighting isn't fully implemented) written into
`cold_light_data`. **DSL-driven cold lights** come later and fill the same texture — out of scope here.

## F5 · LUT churn on caster movement — 2026-07-21 (resolved by the normalization)

**Decided:** the four-table split solves it. A caster's position lives ONLY in `cold_prim_data` (indexed by
`prim_data_index`); the LUT holds indices, not position. So a prim **moving stays in range** = update **one**
`cold_prim_data` texel, and every light referencing it sees the new position — **no LUT edit**. Only a prim
crossing a light's **radius** patches that light's run (add/remove). Plus a **shader radius safety check**
(rectangle / Chebyshev distance where Euclidean isn't needed) tolerates a slightly-stale LUT — a prim that left
range before its run was patched still culls in the shader. Full LUT rebuild only on bulk change (zone stream
in); incremental patching later if needed.

## F6 · Where the shadow base-spread depth (dA/dB) lives — 2026-07-21 (open, user; blocks P4)

The `shadow-projection` fan uses a per-caster **base spread** `dA/dB` (the 5-triangle ±depth) derived from the
sprite silhouette (its presence bake). The **cold-data layout has no field for it** — `prim_definition_data`
carries geometry + frame, `cold_prim_data` carries position + rotation, neither has depth. For P4 (the shader
building the fan from the textures) the depth must come from somewhere. Options:

- **(a) Store `dA/dB` in `prim_definition_data`** — it's **per-def, generic** (all conifers share a base
  spread), exactly like `prim_width/height`. Spend the spare `A` channel: e.g. `u10 dA | u10 dB | u12
  reserved` (units). Compute from the presence bake (the existing `depthFor`) at atlas-add. Natural home;
  a small layout addition to the currently-reserved `A`.
- **(b) Derive in the shader from `prim_width/height`** — a heuristic (e.g. `dA=dB=prim_width·k`); no layout
  change, but loses the silhouette-accurate spread the sandbox's auto-rule gives.
- **(c) Drop the ±depth in the cold-data version** — render only the body triangle (T1); a narrower shadow, no
  base spread. Simplest, a visible regression from `shadow-projection` P3.

**Lean:** (a) — depth is generic per-sprite geometry, so it belongs beside `prim_width/height` in
`prim_definition_data`'s spare `A` channel; the presence bake already computes it. Needs the user's nod (it
extends the authoritative layout in `VARIABLES.md`).
