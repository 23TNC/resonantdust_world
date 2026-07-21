# Forks — cold-data-textures

_Decision points + options + which we chose + why. All resolved 2026-07-21 in the design pass with the user._

---

## F1 · Coordinate space + z/radius units — 2026-07-21 (resolved)

**Decided:** a full **spatial address + sub-tile anchor** — `u32 position_anchor_reference` =
`region_reference | zone_reference | tile_reference | anchor_reference` (each `u8 = x:4 | y:4`), reusing the
existing `position_reference` shape (§Where it is / [`VARIABLES.md`](../../VARIABLES.md)) with `anchor`
replacing the low `layer` byte. Min unit `SQUARE/16` (tile/16); `anchor=(8,8)` centres a prim. Kills the world
cap entirely (no `u16`-px ceiling) and matches the existing addressing + `shared/codec/object` helpers. Units:
`radius` = **tiles** (`u8` = 255), `z` = **`SQUARE/16`** — `u8` in both `cold_light_data` and `cold_prim_data`
(0..1020px, covers `Lz`=480; `z` sits on the top byte, byte-aligned) — proposed in `VARIABLES.md`, tune if the
projection wants finer.

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
