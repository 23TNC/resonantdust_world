# Completed — art pipeline on 128 px tiles

_Nothing delivered yet. Items land here with their measured result when ticked in
[`todo.md`](todo.md)._

## P0 — Verify the model before building on it

- **2026-07-28 · P0.1 · Corpus span audit.** `grep -rn "thing.span set" content/` → **one** hit
  (`things.rd:24`, conifer `2` → 256² at 128) against **nine** `thing.size` declarations. Table of
  all 9 defs recorded in [`issues.md` I5](issues.md#i5). Six are on the `white` placeholder; only
  `tree`/`flora`/`wolf` name real art, and two of those declare no span. Verified by the acceptance
  criterion (table incl. conifer = 2 → 256) — and it **falsified a plan assumption**: the
  `_pow2_box` fallback is the common path, not the exception, so P2 must treat it as first-class.
- **2026-07-28 · P0.2 · `span` traced end to end.** Writer `shared/dsl/src/loader.rs:297`
  (`read_f("prims.0.span", 1.0)`), exported into `thing_layout` at `loader.rs:434`; reader
  `client/webgl/src/game/world/thingPlacement.ts:56` (`span: table[base + 7]`), default 1 at `:42`,
  drawn box `span × span` at `:91`. Both sides present. **Two findings, one of them a plan defect:**
  [I6](issues.md#i6) — the square derives from `span`, NOT `footprint`; `footprint (w,h)` already
  exists and means occupancy (conifer is `footprint 1×1, span 2`), so [F2](forks.md#f2)'s
  `next_pow2(max(w,h))` rule would have sized the conifer 128² instead of 256². F2 rewritten, P2
  items corrected. [I7](issues.md#i7) — `span` is documented pow2 but never validated; `read_f`
  passes any float through.
- **2026-07-28 · P0.3 · Leaf `meta.json` survey.** **No** — across 42 leaf `meta.json` files the only
  keys are `channel_tints` (42) and `outline` (31). Neither footprint nor span is present anywhere,
  so P2 adds a new field rather than extending one. Incidental: `smooth/wall` already uses the
  held-whole linked leaf shape (`biome-tile/default/smooth/wall/`, no per-cell folders) and its
  outline bbox reads **320²**, where 4×4 tiles at 128 wants **512²** — P4 input.
- **2026-07-28 · P0.4 · Manifest read-back.** `smooth/wall` serves `{"cols":4,"rows":4,"padU":0,"padV":0}`
  and IS treated as an atlas; a `generate_tile.py` leaf serves `{"grid","tile","pad","cell",…}` and is
  **not** — `read_atlas_meta` requires four keys the generator never writes, so it returns `None` and
  the sheet is served as a single flat image ([I8](issues.md#i8), live defect predating this stream).
  Verified by running the parser's own logic over every `atlas.json` in the tree. Added a P2 item to
  emit the required schema. Also confirms `padU/padV = 0` on the shipping linked atlas — no cell
  guard today, which is what P3 addresses.
- **2026-07-28 · P0.5 · The linked 4×4 is single-sourced.** `GRID_COLS`/`GRID_ROWS` appear only in
  `bin/art` (defined 149–150, used 1640); the client reads `[cols, rows]` from the manifest
  (`textureManifest.ts:21`, `TextureResolver.ts:209`) with no literal 4. **But P0.5 also found a
  shipped per-cell guard I was about to duplicate:** `GRID_INSET_FRAC` (`bin/art:161`) writes
  `padU=f/cols, padV=f/rows` into `atlas.json`, the server folds it into the manifest, and
  `TextureResolver.ts:202` narrows each cell's UV rect by it. [F3](forks.md#f3) revised — `--pad`
  now *skips* atlases instead of learning to inset them, and P3's items were rewritten to match.
  P0 complete: 5/5.

## P1 — Make the tile edge a value, not a literal

- **2026-07-28 · P1.1 · No tile arithmetic in `bin/art` to parameterise** ([I9](issues.md#i9)).
  `grep -n '\b64\b' bin/art` → nothing; the acceptance criterion held before the item was written.
  `bin/art` is tile-agnostic by construction: linked cell size is *derived* (`atlas width /
  GRID_COLS`, so 320² and 640² both give 16 cells) and `_pow2_box` rounds a blob's **pixel** extent
  with no notion of a tile. The tiles→px conversion first appears at P2 (`span · TILE_PX`), so
  `TILE_PX` is deferred to land beside its first consumer rather than added now with no reader.
- **2026-07-28 · P1.2 · 128 px / 8×8 defaults.** A default run now reports
  `1088px -> 1088px (1.00x shrink) -> 8x8 tiles of 128px +0px pad = 128px cells -> 1024px sheet`.
  Two consequential changes fell out of the new geometry rather than being planned:
  **`--seamless` now defaults to `sheet`** (per-cell wrapping at 8×8×128 would have demanded a
  1536 px plane, and the client samples a cell by world position modulo the sheet, so only the
  sheet's own wrap is ever a seam); and **`--size` now defaults to 0 = derive the exact plane the
  cut consumes**, so the generation is neither upscaled nor resampled and no magic size has to be
  recomputed by hand when the tile edge changes.
- **2026-07-28 · P1.3 · `RD_TILE_PX` override.** `TILE_PX = int(os.environ.get("RD_TILE_PX", 128))`
  is the single tiles→px site. Verified `RD_TILE_PX=64 --grid 16` reports a 64 px tile at a 1024
  sheet. **Acceptance amended honestly:** the item asked for byte-identical reproduction of the old
  geometry, which is no longer meaningful — the old shape was `tile 62 + pad 1`, and [F3](forks.md#f3)
  moved the guard off baked pixels onto the `GRID_INSET_FRAC` inset, so `pad` is now 0 by design.
  What is verified is that the tile edge is a value, not that the superseded pad convention survives.
- **2026-07-28 · P1.4 · Grass regenerated at 8×8×128.** `sprite.l.0.png` 1024², 64 cells,
  **seam energy 0.99 (sheet wrap)** — acceptance ≤1.1 met, matching the 0.95–0.99 the 64 px
  geometry gave. Feature size 6.5 px, cell brightness spread 6/255. **Found and fixed a metric bug
  on the way:** the run first reported 4.83 because `seam_energy` was scored PER CELL, and in sheet
  mode cells are deliberately not individually toroidal — only the plane is. The same sheet's true
  wrap measured 0.99. The tool now measures whatever `--seamless` actually selected and labels it
  (`sheet wrap` vs `worst of N cells`), recording which in `atlas.json` as `seam_measured_on`. Left
  unfixed, every healthy sheet would have reported itself broken. **P1 complete: 4/4.**
