# Art pipeline on 128 px tiles — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering rationale in [`README.md`](README.md): **verify what already exists**, then make the tile
edge a value, then teach the pipeline the footprint, then fix the atlas guard, then migrate.

## P0 — Verify the model before building on it

- [ ] Grep the corpus for every `&thing.span set` and record the value per def. Acceptance: a table in `issues.md` of def → span → implied square at 128, incl. conifer = 2 → 256.
- [ ] Confirm `frame_span` is written and read as pow2 tiles end to end. Acceptance: name the writer and the reader by file:line, or record that one side is missing.
- [ ] Check whether any leaf `meta.json` already carries a footprint or span today. Acceptance: a yes/no with the field name, so P2 extends rather than duplicates.
- [ ] Read back what `tex_manifest.rs` serves for one linked leaf and one thing leaf. Acceptance: the two JSON entries, showing which already carry `grid`/`pad` and which carry nothing.
- [ ] Confirm `GRID_COLS`/`GRID_ROWS` = 4 is the only place the linked 4×4 is fixed. Acceptance: every site that assumes 4 is listed, or the assumption is found duplicated elsewhere.

## P1 — Make the tile edge a value, not a literal

- [ ] Add a single `TILE_PX` constant to `bin/art` defaulting to 128, replacing the hardcoded 64 arithmetic. Acceptance: `grep -n '\b64\b' bin/art` returns no tile-size uses.
- [ ] Change `generate_tile.py` defaults to `--tile 128 --grid 8 --pad 0`. Acceptance: a default run reports `8x8 tiles of 128px -> 1024px sheet`.
- [ ] Make `generate_tile.py` take the tile edge from `RD_TILE_PX` when set. Acceptance: `RD_TILE_PX=64` reproduces the old 16×8 geometry byte-for-byte against a saved fixture.
- [ ] Regenerate the grass sheet at 8×8×128 toroidal and confirm the wrap survives the larger cell. Acceptance: `seam_energy` ≤ 1.1 on the 1024 sheet, matching the 64 px result.

## P2 — Carry the tile footprint in the leaf metadata

- [ ] Decide and record where the footprint lives and who authors it. Acceptance: [F1](forks.md#f1) names the field, the file, and the rejected alternatives.
- [ ] Write `tiles: [w, h]` and derived `span` into each leaf `meta.json` during remaster. Acceptance: conifer variant 0 shows `[1, 2]` and span 2.
- [ ] Read the footprint from the corpus rather than re-authoring it, falling back when absent. Acceptance: deleting a def's `span` makes remaster warn and fall back to `_pow2_box`, not fail.
- [ ] Size the emitted square from the footprint instead of blob extent in `_emit_crops`. Acceptance: conifer emits 256² because span says so — verified by shrinking the blob and getting 256² still.
- [ ] Surface `tiles`/`span` through `tex_manifest.rs` beside `grid`/`pad`. Acceptance: the manifest entry for conifer carries the footprint.
- [ ] Write `tiles` into `atlas.json` for ground and linked sheets. Acceptance: ground reads `[8, 8]`, a linked form reads `[4, 4]`.

## P3 — Make the guard ring per-cell on an atlas

- [ ] Reproduce the bug: run `--pad 1` on an 8×8 atlas and measure an interior cell boundary. Acceptance: `issues.md` records that interior boundaries are unguarded while the canvas edge is.
- [ ] Teach `pad_maps.py` to read `atlas.json` and inset every cell, not the canvas. Acceptance: on an 8×8 sheet all 64 cells have a replicated ring and the sheet is still 1024².
- [ ] Leave non-atlas leaves on canvas-edge padding. Acceptance: a conifer leaf still pads exactly as it does today, byte-identical to a pre-change run.
- [ ] Confirm the emitted `pad` matches what the client already trims. Acceptance: `atlas.json`'s `pad` is the normalized per-cell inset `tex_manifest.rs` expects, checked against its parser.

## P4 — Migrate what is already generated

- [ ] List every existing leaf with its current square and its span-implied square at 128. Acceptance: a table naming which are already right, which are undersized, and which have no span.
- [ ] Re-master the undersized leaves. Acceptance: each one's square equals `span · 128`, and the count of mismatches is zero.
- [ ] Regenerate the ground sheets on the 8×8 grid. Acceptance: `atlas.json` reads `grid [8,8]`, `tile 128`, and the sheet is toroidal.
- [ ] Decide what happens to the old `blueprint/wall/1..16/` per-cell folders. Acceptance: [F4](forks.md#f4) records whether this stream folds them to held-whole or explicitly leaves them.

## P5 — Reconcile the docs

- [ ] Update the texture-layout design with the footprint field and the derived-square rule. Acceptance: the doc states `next_pow2(max(w,h) × TILE_PX)` and names where the footprint is authored.
- [ ] Fix the stale `squareMath.ts` comment claiming `SQUARE (128)` while it reads 64. Acceptance: the comment matches the constant, whichever value it holds when this lands.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, `completed.md` records the before/after squares per kind.
