# Todo — 2026-07-21-shadow-bitfield

_Newest-first. Component: `client/webgl`. Design + assessment in [`README.md`](README.md); the
fixed-size layout + culling/update model in [`forks.md`](forks.md) (F6 consolidation, F5 presence,
F1 gather). The plan below reflects the **2026-07-21 fixed-layout redesign** — it supersedes the
earlier rect-accumulation phases (F3 dropped)._

## P6 · Budget + verify — 2026-07-21

- Optional per-frame **budget**: cast only a subset of dirty tiles by masking the dirty-bit uniform
  (F6) across frames; `log()` deferrals. Default is single-pass (all dirty tiles at once).
- Verify in-browser: overlapping cold-light shadows show **combined bit-colours** in `/overlayRT`;
  stable on pan (toroidal wrap + overscan correct); moving a light re-casts only its dirtied tiles;
  presence cull correct (a tile out of a light's radius never gets its bit).

## P5 · Per-bit overlay (`usampler2D`) — 2026-07-21

- Extend `overlayShader` `OVERLAY_BITS` from float-mod on the RED byte to a real **`usampler2D`**
  decode of all 128 bits of `shadow-cold`; each set bit contributes its light's colour, **overlap =
  OR of colours**. Debug only, `/overlayRT shadow-cold`.

## P4 · The gather cast (single full-window pass) — 2026-07-21

- One pass over `shadow-cold`. Per fragment: derive my tile; sample **`shadow_dirty`** — `discard` if
  clean (persistent RT untouched, no ping-pong). If dirty: read `light_presence_cold` for my tile;
  iterate only its **set light bits**; for each light walk its LUT run in `light_data` rows 1–32
  (`u16` prim indexes, **stop at the `0` sentinel** or 256; `prim_data` → `prim_definition_data`, all
  `texelFetch`); per caster box-cull + **point-in-projected-silhouette** (reuse `shadow-projection`
  math); **OR** `1u << lightIndex` into a register; write the full `u128`.
- Retire the fan-scatter draw in `shadowCaster.ts` (its projection math moves into the fragment).

## P3 · Dirty-tile tracking + the `shadow_dirty` texture — 2026-07-21

- CPU dirty-tile set on the tile grid (window+overscan): a **light** add/move/radius dirties its
  reached tiles; a **caster** move maps index → its lights → their reached tiles (F5 caster-move
  dirtying, over-approximate). Pan-exposed tiles dirty on recenter (cf. `SquareCache.markStale`).
- Write the dirty set into **`shadow_dirty`** — an `R8UI` `128×64` (window+overscan) texture,
  nonzero = dirty (F6 — a texture, not a uniform). Feed to P4.

## P2 · `light_presence_cold` (per-tile light bitfield) — 2026-07-21

- Allocate `light_presence_cold` = **128×64 (window+overscan) `RGBA32UI`**, one tile/px, bit = a
  light reaching that tile. CPU-maintained: on a light add/move/radius change, set/clear its bit over
  the disk of tiles within its radius; upload the changed region. **Not** touched by caster moves.

_P1 (cold-texture reshape) done → [`completed.md`](completed.md). The `shadow-cold` RT allocation
moved into P4 (built with the gather that writes it)._
