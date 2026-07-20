# Completed — caster-lut

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## C1 · Standard 1024×12 float texture + addressing — 2026-07-20

`mkDataTex()` builds a `1024×12` `RGBA32F` `BufferImageSource` (`nearest`) over a `Float32Array`. Shared by
all three textures. `defBase(k, p)` gives the float base-index of px `p` of def `k` (band `⌊k/1024⌋`, col
`k%1024`, rows `3·band+p`); the LUT is indexed by entry directly (`lutData[i]`, since entry `i` = px `i>>2`
channel `i&3` → flat float index `i`).

## C2 · Light texture → 3 px (start_index, count) — 2026-07-20

The light data moved from the `light-data-texture` 5×2 to the standard `1024×12`: px0 `world_x/_y/_z,
radius`, px1 `rgba` (colour, re-rolled once/sec), **px2 `start_index, count, —, —`**. The display shader
samples the colour at the new addressing (`(k+0.5)/1024, 1.5/12`).

## C3 · Prim (caster) data texture — 2026-07-20

`buildCasterData()` writes each resident standing prim (`standingPrims()`) into its 3-px def: px0
`world_x/_y/_z, facing`, px1 `width, height, depth1, depth2`, px2 `frame_*`. Rebuilt when the caster count
changes (zones stream in). depth/facing/frame reserved for the fuller wedge model (0 today).

## C4 · LUT + per-light (start, count) — 2026-07-20

Per light, `buildCasterData` culls the in-range casters (`hypot ≤ radius` — the cull moved out of the cast)
into a contiguous LUT run and records `(start, count)` into the light's px2. `log()`s (never silently
truncates) on caster-slot overflow (> 4,096) or LUT overflow (> 49,152). Uploads are gated: prim + LUT on
rebuild, light on colour-roll or rebuild.

## C5 (partial) · The light → LUT → caster indirection drives the cast — 2026-07-20

`castScreen` now walks the indirection for each dirty light: read `(start, count)` → walk the LUT run →
fetch each caster's rect from the prim data → project the wedge. **Reading the CPU mirrors** (byte-identical
to the uploaded textures). The GPU-side read of these textures (a raw ES-3.00 instanced cast with
vertex-texture-fetch) is deferred — see [D-1](deviations.md#d-1); the remaining C5 stays open in
[`todo.md`](todo.md).

## C6 · Verified in-browser — 2026-07-20

`?focus=100,50&shadowcast`: shadows identical to the pre-refactor cast (coloured per light, clustered on
the markers), now produced entirely by the light → LUT → caster lookup. Colours still work from the
`1024×12` light texture, once/sec. **Pan far** → new casters stream in, the prim texture + LUT rebuild, and
shadows are correct in the new zones with **no ghosts** and **no `caster-lut` warnings** (no overflow). The
three `RGBA32F` textures created + uploaded cleanly (no format errors).
