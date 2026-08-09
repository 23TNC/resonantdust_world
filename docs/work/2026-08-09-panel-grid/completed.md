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

## 2026-08-09 — P1: the grid

**`client/webgl/src/ui/dom/PanelGrid.ts`** (new) — the coordinate system. `GRID_COLS = 58` /
`GRID_ROWS = 33` with no derived duplicate anywhere; integer edge tables
(`edges[i] = round(i * extent / count)`); `projectCell` / `nearestCell` / `clampCell` as pure
exports free of `window` so they can be exercised directly; a thin stateful `PanelGrid` class
owning the live tables, the app's single resize listener, and derived `rowHeight` /
`reservedTop` / `reservedBottom`. Exported as a **singleton** — see `deviations.md`: the plan put
this on `UiEditMode`, which is an optional dependency that `SettingsMenu` deliberately declines,
and a panel cannot opt out of being placed.

`UiEditMode` loses `gridSize` / `getGrid` / `SnapGrid` / the reserves and is now only what its
doc-comment always claimed: the edit-mode flag plus the settings-popup reference. `main.ts` stops
wiring `PanelTaskbar.HEIGHT` into it. `pointerInteractions` moves from a float `SnapGrid`
(`step` + `origin`) to `SnapEdges` (the two tables); the resize helper now snaps the **moving
edge** against the anchored one and captures all four start edges to do it — snapping a *size*
directly would leave the moving edge between grid lines. `DomPanel` drops its per-panel `resize`
listener for a `panelGrid.on(...)` subscription, repoints five reserve reads, and `snapToGrid`
becomes quantize-then-project in one pass.

**Verified live** in the running client (vite :5174, viewport 1862×917 — the maximized-1080p case)
by importing the real module in the page:

- Tables tile exactly: `edgeX[58] === innerWidth` and `edgeY[33] === innerHeight` at 1862×917,
  1920×1080, 1366×768, 1024×640 and 3840×2160 — all true.
- Round-trip `nearestCell(project(r)) === r`: **11,466** cell rects live with 0 mismatches, and
  **848,656** rects × 5 viewports with 0 mismatches running the same module under `tsx` offline.
- One rebuild per resize event with 10 fixed-position elements open (`panelGrid.rebuildCount`
  delta = 1 per event); two same-size resize events cost **0** rebuilds.
- Derived reserves are one row: 28px at 1862×917, 33 at 1920×1080, 23 at 1366×768, 19 at
  1024×640, 65 at 3840×2160 — and the F9 scale those imply (12 / 9-floored / 9-floored / 24px)
  matches the numbers predicted at planning.
- `gridSize` and `getGrid` gone from the codebase bar two prose mentions; `PanelTaskbar.HEIGHT`
  already at zero references.

**Two honest limits on this evidence.** (1) The environment cannot resize the browser window
(`outerWidth` reads 0 under remote Chrome), so the non-native viewports were produced by
overriding `window.innerWidth` / `innerHeight` — the exact globals the grid reads — and
dispatching `resize`. That genuinely exercises the code path but is not a native reflow; P5's
on-camera pass still owes a real one. (2) `npm run typecheck` is **not** green and never was —
see [I11](issues.md#i11): 7 pre-existing `Uint32Array`/`Float64Array` errors in `MoverLayer` /
`WorldScene` from the trait-rows-u32 transport change. Measured before and after by stashing this
stream's diff: identical 7, same files. This stream's "typecheck green" criteria are therefore
read as "no new errors against that baseline". `npm run build` is green.
