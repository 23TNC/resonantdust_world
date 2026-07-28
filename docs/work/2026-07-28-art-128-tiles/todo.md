# Art pipeline on 128 px tiles — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering rationale in [`README.md`](README.md): **verify what already exists**, then make the tile
edge a value, then teach the pipeline the footprint, then fix the atlas guard, then migrate.

## P0 — Verify the model before building on it

- [x] Grep the corpus for every `&thing.span set` and record the value per def. Acceptance: a table in `issues.md` of def → span → implied square at 128, incl. conifer = 2 → 256.
- [x] Confirm `frame_span` is written and read as pow2 tiles end to end. Acceptance: name the writer and the reader by file:line, or record that one side is missing.
- [x] Check whether any leaf `meta.json` already carries a footprint or span today. Acceptance: a yes/no with the field name, so P2 extends rather than duplicates.
- [x] Read back what `tex_manifest.rs` serves for one linked leaf and one thing leaf. Acceptance: the two JSON entries, showing which already carry `grid`/`pad` and which carry nothing.
- [x] Confirm `GRID_COLS`/`GRID_ROWS` = 4 is the only place the linked 4×4 is fixed. Acceptance: every site that assumes 4 is listed, or the assumption is found duplicated elsewhere.

## P1 — Make the tile edge a value, not a literal

- [x] Verify whether `bin/art` has tile-size arithmetic to parameterise, and site `TILE_PX` where it is first needed. Acceptance: [I9](issues.md#i9) records the finding; a constant is only added beside a real consumer.
- [x] Change `generate_tile.py` defaults to `--tile 128 --grid 8 --pad 0`. Acceptance: a default run reports `8x8 tiles of 128px -> 1024px sheet`.
- [x] Make `generate_tile.py` take the tile edge from `RD_TILE_PX` when set. Acceptance: `RD_TILE_PX=64 --grid 16` reports a 64px-tile 1024 sheet, i.e. the tile edge is a value not a literal.
- [x] Regenerate the grass sheet at 8×8×128 toroidal and confirm the wrap survives the larger cell. Acceptance: `seam_energy` ≤ 1.1 on the 1024 sheet, matching the 64 px result.

## P2 — Carry the tile footprint in the leaf metadata

- [x] Decide and record where the footprint lives and who authors it. Acceptance: [F1](forks.md#f1) names the field, the file, and the rejected alternatives.
- [x] Write `span` + the derived square into each leaf `meta.json` during remaster, per [I6](issues.md#i6). Acceptance: conifer variant 0 shows span 2 and square 256.
- [x] Read the footprint from the corpus rather than re-authoring it, falling back when absent. Acceptance: deleting a def's `span` makes remaster warn and fall back to `_pow2_box`, not fail.
- [x] Size the emitted square from `span` instead of blob extent in `_emit_crops`. Acceptance: conifer emits 256² because span says so — verified by shrinking the blob and getting 256² still.
- [x] Surface `tiles`/`span` through `tex_manifest.rs` beside `grid`/`pad`. Acceptance: the manifest entry for conifer carries the footprint.
- [x] Emit the manifest's required `cols`/`rows`/`padU`/`padV` from `generate_tile.py`, per [I8](issues.md#i8). Acceptance: `read_atlas_meta`'s logic accepts a freshly generated ground sheet, which it rejects today.
- [x] Write `tiles` into `atlas.json` for ground and linked sheets. Acceptance: ground reads `[8, 8]`, a linked form reads `[4, 4]`.

## P3 — Stop `--pad` corrupting atlases; use the guard that already exists

_Revised at P0.5 — see [F3](forks.md#f3). The per-cell guard is `GRID_INSET_FRAC`, already shipping._

- [x] Reproduce the bug: run `--pad 1` on an 8×8 atlas and measure an interior cell boundary. Acceptance: `issues.md` records that interior boundaries are unguarded while the canvas edge is.
- [x] Make `pad_maps.py` skip any leaf carrying `atlas.json`. Acceptance: a `--pad 1` run over a ground leaf leaves every map byte-identical and says why it skipped.
- [x] Leave non-atlas leaves on canvas-edge padding. Acceptance: a conifer leaf still pads exactly as it does today, byte-identical to a pre-change run.
- [x] Emit `padU`/`padV` from `generate_tile.py` off `GRID_INSET_FRAC`'s formula. Acceptance: a generated sheet's `atlas.json` carries `padU = f/cols`, matching what `bin/art:1683` computes.

## P4 — Migrate what is already generated

- [ ] List every existing leaf with its current square and its span-implied square at 128. Acceptance: a table naming which are already right, which are undersized, and which have no span.
- [ ] Re-master the undersized leaves. Acceptance: each one's square equals `span · 128`, and the count of mismatches is zero.
- [ ] Regenerate the ground sheets on the 8×8 grid. Acceptance: `atlas.json` reads `grid [8,8]`, `tile 128`, and the sheet is toroidal.
- [ ] Decide what happens to the old `blueprint/wall/1..16/` per-cell folders. Acceptance: [F4](forks.md#f4) records whether this stream folds them to held-whole or explicitly leaves them.

## P5 — Reconcile the docs

- [ ] Update the texture-layout design with the footprint field and the derived-square rule. Acceptance: the doc states `next_pow2(max(w,h) × TILE_PX)` and names where the footprint is authored.
- [ ] Fix the stale `squareMath.ts` comment claiming `SQUARE (128)` while it reads 64. Acceptance: the comment matches the constant, whichever value it holds when this lands.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, `completed.md` records the before/after squares per kind.
