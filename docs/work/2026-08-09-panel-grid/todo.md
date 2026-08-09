# Plan — panel-grid

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** No test runner exists in `client/webgl` (scripts are `dev` /
`build` / `typecheck` only), so every item lands on `npm run typecheck` + `npm run build` green
plus *measured* browser evidence — `getBoundingClientRect()` readings and captures at the three
reference viewports **1920×1080, 1366×768, 1024×640** ("three sizes" below), via
`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`.

## P0 — the paper

- [x] Write `docs/components/client/webgl/design/panel-layout.md`: 58×33 over the full viewport,
      row 0/32 = the bars, field = rows 1..31, integer edge tables, cell-as-unit, body-rect law
      (F1–F4). Acceptance: docs-check green.
- [x] Link the new design doc from the `client/webgl` component README's `design/` bullet.
      Acceptance: docs-check link integrity green.

## P1 — the grid

- [x] `UiEditMode`: `GRID_COLS = 58` / `GRID_ROWS = 33` + integer `edgeX[0..58]` / `edgeY[0..33]`
      tables rebuilt from the live viewport (F2). Acceptance: `edgeX[58] === innerWidth` and
      `edgeY[33] === innerHeight` at three sizes.
- [x] Add `project(cellRect) → px` and `nearestCell(px) → cellRect`. Acceptance:
      `nearestCell(project(r))` equals `r` for every cell rect in the field, at three sizes.
- [x] Derive `rowHeight` + `reservedTop` / `reservedBottom` (one row each) as getters; DELETE
      `gridSize` and the float-step `getGrid` (F6). Acceptance: typecheck green, zero `gridSize`
      references remain.
- [x] ONE resize broadcaster on the grid; `DomPanel` drops its per-panel `resize` listener.
      Acceptance: with N panels open, one recompute per resize event (counter logged).

## P2 — the chrome on rows

- [x] `PanelTaskbar` height = one row, re-applied on grid change; `HEIGHT` const + static DELETED
      and `main.ts` stops wiring reserves (F6). Acceptance: measured heights equal `edgeY[1]` and
      `innerHeight − edgeY[32]` at three sizes.
- [x] Taskbar entries size off the row instead of a hard 24px. Acceptance: entries stay inside
      the bar with no clipping at 1024×640.
- [x] Title bar = one row: `TITLEBAR_CSS` height grid-driven, horizontal padding only, text
      vertically centred (F4). Acceptance: `titlebar.offsetHeight === rowHeight` for every open
      panel at three sizes.
- [x] Title-bar toggle made body-invariant: outer box = body rows + 1 when shown, top edge moves
      with it, no persisted geometry written (F4). Acceptance: body rect bit-identical across
      hide→show→hide; capture.

## P2b — the UI scale (F9)

- [x] The grid writes `--ui-row` on `document.documentElement` on every recompute, and nothing
      else (F9). Acceptance: `getComputedStyle(root).getPropertyValue("--ui-row")` equals the
      measured row height at three sizes.
- [x] Author `--ui-font` as `max(9px, calc(var(--ui-row) * 0.375))` in one `:root` block in
      `index.html` (F9). Acceptance: it resolves to 12px at a 1080-tall viewport and 9px at 640.
- [x] Author the rest of the scale beside it as `calc()` off `--ui-row`: `--ui-font-lg`,
      `--ui-font-xl`, `--ui-pad`, `--ui-pad-sm`, `--ui-gap`, `--ui-btn` (F9). Acceptance: each
      resolves to today's px value at a 1080-tall viewport.
- [x] Sweep the ~22 hardcoded `fontSize` sites across the 8 files onto `var(--ui-*)`.
      Acceptance: zero `fontSize: "<n>px"` literals remain under `client/webgl/src`.
- [x] Sweep the fixed paddings, gaps and button widths in `DomPanelStyles` + `PanelTaskbar` onto
      the scale; 1px borders stay literal (F9). Acceptance: chrome at a 1080-tall viewport is
      pixel-identical to today's; capture the before/after pair.
- [x] Confirm the scale costs ONE write per resize, not a traversal. Acceptance: resizing with N
      panels open performs a single `setProperty` call (counter logged) and no per-element
      restyle pass exists in the resize path.

## P3 — panels on cells

- [x] Replace the six CSS-string storage keys with `col` / `row` / `cols` / `rows` integers (F3).
      Acceptance: every new write is an integer; no `px` string under any panel storage key.
- [x] Versioned one-time migration: px → nearest cell, superseded keys deleted including a stale
      `gridSnap` (I3). Acceptance: a profile seeded with today's px keys lands on sane cells with
      no panel off-field.
- [ ] Placement becomes ONE pass from the cell rect (`left`/`top`/`width`/`height`,
      `right`/`bottom` auto), retiring the `snapToGrid` → `applyAnchor` two-pass fight (F7/I5).
      Acceptance: resize from each corner still pivots on the anchor.
- [x] Fire `rectChange` on every placement path — grid change, migration, title toggle, drag end,
      resize end, snap, reset (I6). Acceptance: the world canvas stays correctly sized through a
      1920→1280→1920 round-trip.
- [x] Re-project every panel on grid change. Acceptance: 1920×1080 → 1280×720 → back leaves every
      panel's cell rect identical (before/after logged) with no drift.
- [ ] Drag + resize quantize to cells unconditionally; `activeSnapGrid`'s null path retires.
      Acceptance: after a drag and a resize, storage holds integers only.
- [x] `_gridSnap` defaults TRUE (`contentDefaults.gridSnap ?? true`); the popup row stays this
      stream (F5). Acceptance: a fresh panel snaps without being told to; the toggle still
      round-trips.
- [x] Clamp to the field: rows 1..31, a titled body starts at row ≥ 2 (F4). Acceptance: a panel
      dragged at either bar stops at the field edge; nothing overlaps a bar at three sizes.
- [x] Minimums in cells (min body 6 × 3); `heightMode` full/half/quarter become row counts over
      the field. Acceptance: resize refuses under 6×3; a `full` panel spans rows 1..31 exactly.
- [ ] `heightMode: auto` rounds UP to whole rows, re-applying only when the row count changes
      (I7). Acceptance: an `auto` panel doesn't oscillate under content churn.
- [ ] Decide minimize under the body-rect law — force-show the title bar, or imply the button
      hidden (I8). Acceptance: a minimized panel always presents a way back, at three sizes.

## P4 — the corpus

- [ ] "Copy All JSON" exporter emits cell rects (I4). Acceptance: exported JSON carries
      `col`/`row`/`cols`/`rows` and no px strings.
- [ ] `defaults.json` re-authored in cells for all 8 panels, in the same commit as the exporter
      (I4). Acceptance: arrange → copy → paste into the file → reset → identical layout.
- [ ] Every `defaultRect` call site in cells: chat, settings menu, settings popup, video, debug;
      `PanelManager`'s cascade steps too. Acceptance: cleared storage → each opens on-grid inside
      the field.
- [ ] Login `FormOverlay` re-authored in cells, percentage centring retired (I9). Acceptance: the
      form is centred (or off by at most half a cell) at three sizes.

## P5 — the truth

- [ ] Verify the settings popup + settings menu stay usable at 1024×640 (I10). Acceptance: no
      clipped label, no unreachable row; capture.
- [ ] Eyeball the scale's two known consequences (I2): the ~13% lighter chrome on a maximized
      1080p browser, and the 9px floor engaging below ~1366×768. Acceptance: captures at both,
      plus the ratio constant's value if the user wants today's weight back.
- [ ] The full exit on camera: three sizes, a resize round-trip, a title-bar toggle, a fresh
      profile. Acceptance: captures + measurements in completed.md.
- [ ] Docs + memory truth pass; **the user's eyes close the stream**. Acceptance: docs-check
      green; the memory line written.
