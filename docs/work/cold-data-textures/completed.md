# Completed — cold-data-textures

_Done + verified. Items move here from [`todo.md`](todo.md) (append-only history; authoritative for what's
done). Nothing yet — the stream opened 2026-07-21._

## P0 · The four layouts, authored + byte-checked — 2026-07-21

The design pass with the user, formalized. All five forks resolved ([`forks.md`](forks.md)) and the four
`RGBA32UI` layouts + `position_anchor_reference` authored in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md), byte-checked to 128-bit pixels:
`cold_light_data` (1 px/light) · `cold_light_prim_data` (the LUT, 4 entries/px, indices only) ·
`prim_definition_data` (1 px/sprite variant, generic; `frame_page` + a spare `u32` for materials) ·
`cold_prim_data` (2 entries/px: `position_anchor_reference` + `u8 z | u2 rotation | u22 reserved`). The
normalization — a caster's position lives only in `cold_prim_data` — is the load-bearing choice: move a caster
= one texel, not every light's run. Refined in the same pass: one world **unit** = `SQUARE/16` (compile-time,
everything world-space; atlas `frame_*` stay texture px), `radius` widened to **`u12`** (≈16 zones), and a
**`cast_shadows`** flag (0 → the light lights without casting: no LUT run, skipped in the cast).

## P1 · prim_definition_data + atlas integration — 2026-07-21

New `ColdShadowData` module owning the shadow cast's GPU data textures. `prim_definition_data` (`RGBA32UI`,
1 px/sprite variant): `prim_width/height` in units (px/`UNIT`, `UNIT=SQUARE/16=4`), the surface atlas frame
`x/y/w/h` (atlas px), `frame_page` (0 — single page for now). Allocated + written lazily on first sight of a
caster sprite from the resolver's surface frame, cached per stem+cell; uploaded only when a def lands. The
`ShadowCaster` populates it in its tick; the render still uses the instance-attr path. **Verified (readback):**
conifer = 32 units (2 tiles), flora = 8 units (0.5 tiles), correct frames + page 0.

## P2 · cold_prim_data (placed caster instances) + position codec — 2026-07-21

`cold_prim_data` (`RGBA32UI`, 2 entries/px): each placed caster's `position_anchor_reference` + `z` +
`rotation`, allocated per `prim.id` + cached (cold things static → written once). `encodePosition`/
`decodePosition` pack world px into `region|zone|tile|anchor` via the `ZONE_DIM`/`REGION_DIM` constants (anchor
= sub-tile units). `rotation` from `flipX` (E=1/W=3) as a placeholder until the prim carries a real facing
(F1/P5); `z=0`. **Verified (readback):** `encode(6432,3330)` round-trips to `[6432,3328]` (x exact, y
unit-quantised to 4px); 658 casters, `z=0`, `rotation=1`.

## P3 · cold_light_data + cold_light_prim_data (the LUT) — 2026-07-21

`cold_light_data` (`RGBA32UI`, 1 px/light): encoded position, colour+intensity (u8), radius+z in **units** +
`cast_shadows`, and the `(lut_index, lut_count)` run. `cold_light_prim_data` (the LUT, 4 entries/px): each
`cast_shadows` light's in-range casters as a contiguous run of `(definition_index, prim_data_index)`,
Chebyshev-culled (F5). `buildLights` builds both + populates the def/prim caches; run only on change (caster
set / lights / a LOD landing), not per frame. **Fixed a rebuild-gate deadlock** — the build only re-ran when
`defCount` changed, but that only changes inside the build; now a `resolver.onLoad` subscription re-dirties it
when a LOD lands, so sprites resolving after the first build get their defs. **Verified (readback):** 6 lights
ringed around (100,50), correct per-light colours, intensity 255, z=120 (480/4), radius=64 (256/4),
`cast_shadows=1`, contiguous LUT runs `[0,25][25,22][47,17]` listing each light's in-range casters.

_Data layer (P1–P3) complete + verified. P4 (the shader read) is blocked on [F6](forks.md#f6): the layout has
no home for the fan's base-spread depth (`dA/dB`)._
