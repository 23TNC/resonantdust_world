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

## P2 — Carry the tile footprint in the leaf metadata

- **2026-07-28 · P2.1 · Footprint ownership decided.** [F1](forks.md#f1): the corpus authors
  (`thing.span` in `content/visual/things.rd`), the leaf caches. Three alternatives rejected with
  reasons. [F2](forks.md#f2) was rewritten at P0.2 once `footprint` turned out to already exist and
  mean occupancy — the square derives from `span` alone ([I6](issues.md#i6)).
- **2026-07-28 · P2.3 · Corpus span reader** (`bin/lib/def_span.py`). Maps a texture stem to its
  authored `thing.span` by pairing `&thing.texture set` with `&thing.span set` inside each def.
  Verified: `biome-thing/default/conifer` → 2 (square 256 at 128 px, 128 at `RD_TILE_PX=64`);
  `pawn/animal/wolf`, `biome-thing/default/flora` and any unknown stem → **exit 1**, so the caller
  falls back rather than silently assuming span 1. Skips the `white` placeholder (6 defs use it and
  it names no leaf). Rounds up to pow2 since the loader never validates ([I7](issues.md#i7)).
  Re-confirms [I5](issues.md#i5) live: 1 of 3 real-art defs authors a span.
- **2026-07-28 · P2.2 · Span stamped into every leaf** (`bin/lib/leaf_span.py`, `art leaf-span`,
  and a remaster stage). Acceptance met: `conifer/0` → `span 2, square 256, span_from corpus`.
  Resolution is three-tier — corpus, then the leaf's own `atlas.json` (an atlas *states* its tile
  count, so `smooth/wall` resolves `span 4, square 512` as fact rather than measurement), then
  inference from the art marked `span_inferred`. Whole tree: **80 leaves — 9 corpus, 3 atlas,
  68 inferred**, which is [I5](issues.md#i5) quantified. Two bugs caught in test: the source tally
  ran outside its `if` guard, and `span_inferred` persisted stale because `meta.update` merges and
  cannot delete a key — the flag is now always written, so a leaf that stops being inferred loses
  the marker.
- **2026-07-28 · P2.4 · The square now comes from `span`, not blob extent** — [I1](issues.md#i1)
  closed. `_span_side <kind>` returns `span · TILE_PX` (conifer 256, empty for the wolf), and both
  emit paths use it as `EMIT_SIDE` when the corpus authors one, keeping the measured pow2 box
  otherwise. **Acceptance verified with a real counterfactual:** the conifer sheet was shrunk to
  627² so its blob max fell to 160 px — `nearest_pow2(160) = 128`, so extent-driven sizing would
  have emitted **128²** — and it still emitted **256²**, logging `canvas 256□ from the declared span
  (blob max 160px)`. Art restored from backup afterwards (1254² sheet, 256² leaf, all 9 maps).
  Note both paths needed it: the leaf path (`_remaster_leaf_sprites`) and the kind-level sheet path
  (`_split_variant_sheet`), and conifer actually travels the latter.
- **2026-07-28 · P2.5 · `atlas.json` now speaks the manifest's schema** — [I8](issues.md#i8) closed.
  `generate_tile.py` emits `cols`/`rows`/`padU`/`padV` (the four `read_atlas_meta` requires) beside
  its provenance keys, which the parser ignores. Added `--inset F` mirroring `GRID_INSET_FRAC`'s
  contract (`padU = f/cols`). Also found the sidecar was written at KIND level beside the sprite
  source while the manifest walks LEAVES, so it would never have been read — remaster now carries
  it into every leaf the sheet produces.

## P3 — Stop `--pad` corrupting atlases; use the guard that already exists

- **2026-07-28 · P3.1 · Bug reproduced and measured.** `--pad 1` on the 8×8 grass sheet: canvas edge
  guarded (`row0 == row1`), **interior cell boundary at x=511|512 unguarded** (mean |Δ| 2.75, not
  equal), and content 1022 px across 8 cells = **127.75 px/cell**. Exactly what [F3](forks.md#f3)
  predicted.
- **2026-07-28 · P3.2 · `pad_maps.py` skips atlas leaves.** A leaf carrying `atlas.json` is left
  untouched and the reason is printed. Re-remaster: `0 map(s) shrunk … skipped 2 atlas leaf/leaves`.
- **2026-07-28 · P3.3 · Non-atlas leaves unchanged.** Conifer still pads — 72 maps, guard ring
  present (`row0 == row1` on a 256² albedo).
- **2026-07-28 · P3.4 · `padU`/`padV` emitted** on `f/cols`, matching `bin/art`'s awk formula.
- **2026-07-28 · [I10](issues.md#i10) · Found a LARGER bug behind P3.1.** With `--pad` fixed the
  sheet was still wrong: `_split_variant_sheet` blob-detects every kind-level sheet and refits it to
  a pow2 box, so `SPLIT_PADDING=4` resized the 1024 atlas to 1016 inside a flat 4 px frame —
  **127 px/cell, and resampled** (`max |diff| sprite vs diffuse = 230`, `row0` a single colour).
  Held-whole atlases now pass to leaf 0 verbatim, as `_split_variant_leaf` already did. Verified:
  **max |diff| 0, cell pitch 1024/8 = 128 px.**
- **2026-07-28 · P2.6 · `span`/`square` surface through the manifest.** `Entry` carries both, read
  from the leaf's `meta.json` by `read_span_meta`, emitted beside `grid`/`pad`. Absent keys stay
  absent rather than defaulting — an unstamped leaf must not claim span 0 or 1. Also folded
  `meta.json` into the leaf hash (only `atlas.json` was), or a re-stamped span would never reach a
  client holding the stem cached. Verified by a new test asserting: no `span` key before stamping,
  `"span":2` and `"square":256` after, and a changed leaf hash. **Found three dead tests on the way**
  ([I11](issues.md#i11)) — the helper wrote a superseded leaf shape so the manifest was empty and
  every assertion was vacuous. Repaired it; suite went **12 passed/3 failed → 14 passed/2 failed**,
  the two remaining being pre-existing `textures.rs` failures confirmed by stashing and re-running.
- **2026-07-28 · P2.7 · `tiles` in both `atlas.json` writers.** Ground verified live —
  `textures/biome-tile/default/grass/atlas.json` reads `tiles [8,8]`. Linked verified by exercising
  the writer's `printf` directly (`tiles [4,4]`) because the path is currently **unreachable**:
  `GRID_CATS` is still `"linked"` while the linked forms live under `biome-tile` per the design's
  decision 5, so `_grid_slice_id` never fires for them ([I12](issues.md#i12)). Moving `GRID_CATS`
  is [F4](forks.md#f4)'s call. **P2 complete: 7/7.**

## P4 — Migrate what is already generated

- **2026-07-28 · P4.1 · Migration audit.** 78 stamped leaves at 128 px tiles: **61 already right,
  17 undersized, 0 oversized.** Span source: 68 art / 9 corpus / 1 atlas. The 17 split cleanly into
  two unrelated problems — 16 are `blueprint/wall/1..16` (the superseded per-cell split, 160→256)
  and 1 is `smooth/wall` (the real linked atlas, 320→512). Raised as [B1](blockers.md#b1): neither
  should simply be re-mastered.
- **2026-07-28 · P4.3 · Ground sheets on the 8×8 grid.** `biome-tile/default/grass` reads
  `grid [8,8]`, `tile 128`, `tiles [8,8]`, 1024 px, **wrap seam 0.99**. It is the only ground sheet
  in the tree (the stale 64 px `stone` leaves were removed). Acceptance met.

## P5 — Reconcile the docs

- **2026-07-28 · P5.1 · Derived-square rule documented** in
  [`texture-layout`](../../components/dev/textures/design/texture-layout/README.md). States
  `square_px = span_tiles × TILE_PX`, the atlas variant (`tiles` in `atlas.json`), a table
  separating `footprint` / `span` / `size` with the conifer and wolf as the worked cases, where the
  value is authored vs cached, and that the guard ring differs by shape (baked for a sprite, a
  per-cell sampling inset for an atlas). **The item's own acceptance was wrong** — it asked for
  `next_pow2(max(w,h) × TILE_PX)`, the footprint rule [I6](issues.md#i6) disproved — so the
  corrected rule was written and the item amended.
- **2026-07-28 · P5.2 · `squareMath.ts` reconciled.** The contradiction I3 recorded is gone, fixed
  by [`square-128`](../2026-07-28-square-128/README.md) landing its P1+P2 (`bee4e55`, `acaa1ad`)
  while this stream ran: `SQUARE` now genuinely reads **128** with `TEXTILE_LIGHT = 64` split off.
  **Not my work** — but it left one residual: a comment still called `SQUARE` "the only dial", which
  is precisely the identity that split removed. Rewritten to say raising `SQUARE` now sharpens the
  art maps alone. Convenient consequence: this stream's `TILE_PX = 128` and the renderer's `SQUARE`
  now agree, which was the assumption the whole plan was parameterised against.
