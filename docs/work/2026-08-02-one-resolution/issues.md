# Issues — one resolution

## I1 — the two "lod"s, every consumer named {#i1}

**ATLAS LADDER (dies this stream)**:

| file | what it holds |
|---|---|
| `textures/lod.ts` | `LOD_SIZES`, `pickLodForSize`, `lodUrl(size)` — also hosts `ZOOM_MIN/MAX` + `TexMap`, which MOVE, not die |
| `textures/TextureResolver.ts` | `targetPx`/`setTargetLod`, per-(stem, size) `packed`, `ensureCoPack(size)`, below/above pick, `FLOOR_LOD` preview kick, `lodStats` |
| `textures/LodPool.ts` | the pool (renames); per-size frames of one stem coexist today |
| `textures/previewCache.ts` | IndexedDB rows keyed `stem@size` |
| `textures/textureManifest.ts` | `maxSize` — KEEPS: becomes THE size (F2) |
| `textures/TextureAtlas.ts` | comment only; quadrant co-pack model keeps |
| `main.ts`, `DebugPanel.ts` | `lodStats` HUD + `TEX_ATLAS_SIZES` spans |
| `BuildPanel.ts` | icon fetch via `lodUrl(..., 512)` — re-points at the manifest max |
| `Viewport`/zoom path | `setTargetLod` call site |

**SLOT-GRID LEVEL (survives, renamed "partition level", P4)**: `squareMath.ts`
(`lodForZoom`, `LOD_LEVELS/MAX`, `tileTexels`), `SquareCache.ts` (`this.lod`,
re-partition), `Viewport.ts` (`win.lod` → lighting), `lightPass/shadowPass`
(`TEXTILE_* >> lod` uniforms), `WorldBridge.ts` (`lodForZoom` sizes the zone
subscription), `Camera.ts` (imports zoom bounds from `lod.ts` — import moves with
them), `albedoBlit/mrtBake/records/recordSync` (comments + `uLSlot`).

**BEFORE numbers** (cold loads at the fixture): zoom 0.25 → pool counts `{32: 5}` — the
ladder serving the 32-px tier; zoom 1 → `{128: 5, 32: 5}` — EVERY stem duplicated
across two sizes (the 32 tier is the FLOOR_LOD preview that never gets evicted), 1
page, preview count 5. Captures in the session record.

## I2 — the pow2 audit: one real violator, and how it hid {#i2}

Live pool dump: **zero violators** — every co-packed frame is square pow2 (64 at the
32 tier, 256 at 128). Disk audit of all 88 masters: every SERVED map is 512 pow2
except **`brick/wall/normal.l.0.png` at 320×320** beside 512 siblings — the folder's
`meta.json` already declared `square: 512, tile_px: 128`, so albedo/surface were
re-mastered to 512 at some point and the hand-painted normal never followed. A
mixed-size co-pack blits the 320 source into a 512 quadrant misregistered, and the
pool only WARNED on its other invariant, so nothing was loud.

Repaired: resampled 320→512 (LANCZOS — a resample, NOT a regeneration; the
hand-painted-normals lock convention is about not re-running laigter). Enforcement now
has two faces in `LodPool`: non-pow2-square frames are REFUSED, and a whole-blit
co-pack source that is not exactly `quadN²` is REFUSED (the brick/wall class).

**Not violations**: `sprite.l.0.png` (320/640/1024) and `diffuse.l.0.png` (320) are
unserved pipeline intermediates — outside the client's `TexMap` set; left as-is,
flagged for `bin/art` hygiene some other day. A stray `grass/sprite.l.0.png` sits one
folder above its stem — same category.

## I3 — the F4 split silently killed shadows; caught by the user's eyes {#i3}

The def page registry (`recordSync.defFields`) registered the ALBEDO frame's source.
Pre-split that was the one co-packed page; post-split it is the GRAPHICS twin — whose
surface quadrant is EMPTY, so `uSurfaceAtlas` bound a page where every `silhouetteHit`
read coverage 0 and the whole occlusion solve answered "none" (gather: 0 of 131 072
pairs, with 457 casters live). Nothing errored; the world just stopped casting.

Two-part fix, both halves necessary: the page registry takes the SURFACE frame's
SOURCE (the data twin), but frameX/frameY stay the ALBEDO quadrant's coordinates —
`silhouetteHit` reaches the surface by adding `+frameUnits` to the def frame, so
registering the surface quadrant's own coords double-offset the sample one quadrant
past the art (the first fix attempt; still zero). Verified: gather 355 occluding
pairs, walk = brute, silhouette shadows on screen at the fixture.

Lesson recorded: the def frame is ONE rect serving two pages and two addressing modes —
any future re-plumbing of `resolve()`/quadrants must re-run `__gather()` (zero
`nonZeroBefore` is the tell) before shipping; the P5 sweep checked `__lightexact`
(delta-path exactness, which stayed green) but not occupancy, and exactness alone
cannot see an all-zero shadow buffer.
