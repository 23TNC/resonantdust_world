# Completed — panel-grid

## 2026-08-09 — P0: the paper

`docs/components/client/webgl/design/panel-layout.md` written and linked from the component
README's `design/` bullet (which also picked up the previously-unlisted `input-model.md`).

The doc is the durable form of F1–F9: the grid (58×33 over the FULL viewport, row 0/32 = the
bars, field = rows 1..31, count fixed on every aspect, `GRID_COLS`/`GRID_ROWS` as the
compensation knob); the integer edge tables and why the float step is gone (the pre-grid corpus
drift `"56.3295px"` / `"31.8977px"` is the evidence); the cell as the persisted unit, with
`anchor` demoted to the resize pivot and grid snap as the law; the body-rect law with the
`outer.row/rows` arithmetic spelled out; derived chrome; the UI scale table with every property's
value at a 1080-tall viewport; and a closing list of **7 invariants a change must not break** —
written so a future session can check the code against them without re-deriving the design.

Verified: `bin/rd docs-check` clean (699 files, the 5 standing warnings — 4 stale `current/`
stamps predating this stream, 1 repo-wide item-length count).
