# Completed — one resolution

## 2026-08-02 · P1–P3 — the ladder dies; the atlas splits; mips carry minification

**P1, the resolver collapse**: one co-pack per stem at the manifest's `maxSize` (F2 —
the conifer immediately upgraded to its TRUE 256 master, which the old
`BASE_LOD_PX = 128` clamp had been capping); `packed` is `Map<stem, frame>`;
`LOD_SIZES`/`pickLodForSize`/`targetPx`/`setTargetLod`/`FLOOR_LOD`/`bestLoaded` and the
orphaned `lodTier`/`lodArtPx`/`realUrl`/`previewUrl`/`metaUrl` deleted; IndexedDB keys
collapsed to `(stem, map)` (DB v3 — old per-size rows orphan harmlessly); `LodPool`
renamed `SpritePool`; `lodStats` is `{pages, frames}` and the HUD's per-size ladder rows
became one "packed frames" row. Verified live: textured cold load at zoom 1; a fresh
reload after cache priming issued ZERO `/lod/` fetches (the whole set packed from
IndexedDB); pool 1 page / 5 frames with NO 32-px duplicates (P0 baseline: every stem
×2 sizes).

**P2, the records hold still**: def count 34 CONSTANT across a live zoom 2 → 0.25
sweep with the atlas page registry frozen at 1 page — the "bound atlas page switches
with zoom" gotcha is dead by construction. `__lightexact` bit-identical. Grep-clean of
the ladder names (two history-referencing comments remain, allowed).

**P3, the SPLIT atlas** ([F4](forks.md#f4), the user's mid-flight correction — my
single-atlas mip plan would have FILTERED the surface silhouette): one packer rect,
twin 2048² pages — GRAPHICS (albedo + normal + layers; mip chain capped at level 2,
trilinear MIN, LINEAR mag) and DATA (surface alone; NEAREST, no mips, exact bytes).
Normal classed graphics by judgement (its sole consumer renormalizes; unfiltered
normals shimmer); layers moved to graphics on the user's follow-up with the
linear-in-weights proof. Frame coordinates identical on both pages, so no def or
shader addressing changed — `resolve()` re-sources graphics-map quadrants via the
pool's twin lookup. Verified live: albedo and surface frames reference DISTINCT
textures; a mid-silhouette surface row reads pure 255s (unblended); `__lightexact`
bit-identical; `__zprobe` answers correctly at lod 0 and lod 2; zoom 0.25 renders the
full window from max masters through the mip chain — visibly cleaner than P0's 32-px
ladder tier (honest note: the P0 capture was mid-stream-load, so the A/B leans on the
pool evidence plus the visual).

## 2026-08-02 · P0 — the concepts pinned, the law enforced

**The inventory** ([I1](issues.md#i1)): every consumer of both "lod"s named — the atlas
ladder's ten files vs the slot-grid level's nine, with the two stowaways flagged
(`ZOOM_MIN/MAX` + `TexMap` live in `lod.ts` but belong to the survivor side).
BEFORE numbers: zoom 0.25 packs `{32: 5}`, zoom 1 packs `{128: 5, 32: 5}` — every stem
duplicated (the FLOOR_LOD preview never evicts), 1 pool page. Captures in the session
record.

**The audit** ([I2](issues.md#i2)): live pool — ZERO violators; disk — one real one,
`brick/wall/normal.l.0.png` at 320×320 beside 512 siblings (its `meta.json` already
declared 512; the hand-painted normal never followed the re-master, and a mixed-size
co-pack lands the quadrant misregistered SILENTLY because the pool only warned).
Repaired by LANCZOS resample 320→512 (a resample, not a regeneration — the lock
convention is about laigter re-runs). The 320/640 `sprite`/`diffuse` files are
unserved pipeline intermediates, recorded not repaired.

**The law** ([F3](forks.md#f3)): SQUARE POW2 ≥ 16, chosen over free-size with pros/cons
recorded (every addressing invariant assumes it; ES 3.0 NPOT support only buys margin
bytes). Sign-off: the user armed the stream after reading F3's recommendation; the
fork stays cheap to flip if their verdict differs. Enforcement has TWO faces in the
pool, both REFUSE + `console.error`: non-pow2-square frames, and whole-blit co-pack
sources that are not exactly `quadN²` (the brick/wall class — subframe draws carry
their own source rect and stay exempt). Verified live: brick/wall now co-packs
`{512 → 1024²}` cleanly with the resampled normal; the old 320 would have been
refused; the fixture boots with no `REFUSED` in the console.
