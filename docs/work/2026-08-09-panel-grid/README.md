# panel-grid — the 58×33 cell grid as THE UI coordinate system

_User (2026-08-09): "We are going to modify panel placement. We will dynamically change the
grid so that we have 58x33 grids. Grid snapping will snap to those grids. This way when the
screen re-sizes we can re-size to the grid. All panels will grid snap, so we will default grid
snap to true, and later remove the option from the menu. Title bars will be 1 grid tall. When
title bars are hidden the height will be reduced by 1 so that the body does not move as the
title bars are 1 grid. The top and bottom task bars will be 1 grid tall. I think this
generalizes our UI placement."_

The last sentence is the whole stream. Today the UI has no coordinate system — it has pixels,
plus five separate mechanisms that each try to keep pixels honest across a resize. The grid
replaces all five with one number pair per edge.

## What this dissolves (surveyed against the delivered state)

- **The grid is DERIVED from the taskbar today, and the dependency is backwards.**
  [`UiEditMode`](../../../client/webgl/src/ui/dom/UiEditMode.ts) takes a *target* cell side
  (`gridSize`, default 32 — chosen "matching the taskbar height for a consistent visual
  rhythm"), insets the viewport by `reservedTop`/`reservedBottom` (both wired in
  [`main.ts`](../../../client/webgl/src/main.ts) from `PanelTaskbar.HEIGHT = 32`), then rounds
  to whole cells. The bar's magic constant defines the grid. Inverted (F1/F6), the grid defines
  the bar — and "1 grid tall" is true by construction instead of by coincidence.
- **Panel rects persist as CSS pixel strings, and the drift is already checked in.**
  `persistSize`/`persistPosition` write `panel.style.left` etc. verbatim; the shipped
  [`defaults.json`](../../../client/webgl/src/content/panels/defaults.json) carries
  `"top": "56.3295px"`, `"bottom": "31.8977px"`, `"height": "462.727px"`. Those numbers are the
  residue of float snap math (`round(px / step) * step`) against a step that is itself a float.
  Cells are integers (F2/F3); there is nothing left to drift.
- **`anchor` exists because there was no layout space.** A right-anchored panel sticks to the
  right edge on resize; that is a per-panel workaround for the missing coordinate system. With a
  cell rect the re-projection *is* the reflow, and `anchor` collapses to what its own doc-comment
  already claims it is: the resize pivot (F7).
- **Grid snap is opt-in and the shipped corpus half-uses it.** `_gridSnap` defaults `false`;
  4 of the 8 panels in `defaults.json` snap, 4 don't. A law that half the corpus obeys is not a
  law. Default true now, option deleted next (F5).
- **The title bar's height is an accident of padding.** `TITLEBAR_CSS` is `padding: 8px 12px`
  around a 12px font — ~29px, near the 32px bar but not equal to it, and unrelated to any cell.
  So a panel snapped to the grid still has an off-grid seam inside it. One row, explicitly (F4).
- **Hiding the title bar moves the body.** `toggleTitleBarHidden` sets `display: none` and lets
  the `flex: 1 1 auto` body grow into the freed slot — the body both moves and resizes on a
  chrome toggle. The user's requirement kills this: the persisted rect becomes the **body's**,
  the title bar is chrome in the row above it, and the toggle touches the body's rect not at all
  (F4).

## The stance

- **The grid is 58 × 33 over the FULL viewport** (F1) — not the safe area. Row 0 is the top
  taskbar, row 32 the bottom taskbar, rows 1..31 are the panel field. This is forced by the
  user's own two sentences: if the bars are 1 grid tall, the bars must be *in* the grid. The
  numbers check out — 58:33 = 1.758 against 16:9 = 1.778, so cells are square within 1.2% on any
  16:9 viewport, and at 1920×1080 a cell is 33.1 × 32.7 px: today's rhythm, now exact.
- **Cell edges are integers, from an edge TABLE** (F2): `edgeX[i] = round(i · innerWidth / 58)`,
  `edgeY[j]` likewise. Cell width is `edgeX[i+1] − edgeX[i]`, so cells vary by ≤1px, tile the
  viewport exactly, and two panels sharing a boundary share the same integer. No `stepX` float.
- **The CELL is the persisted unit; pixels are a projection** (F3). A panel stores
  `col / row / cols / rows` — four small ints. A viewport resize recomputes the edge tables and
  re-places every panel from its unchanged cell rect. That is the user's "when the screen
  re-sizes we can re-size to the grid", and it is the only part of this stream that is genuinely
  new machinery rather than a re-expression of something that exists.
- **The persisted rect is the BODY's; the title bar is the row above it** (F4). Showing the bar
  grows the outer box *upward* by one row; hiding it shrinks the outer box by one row. The body's
  rect is bit-identical across the toggle — "the body does not move", literally. A titled panel's
  body therefore starts at row ≥ 2; an untitled one may sit at row 1.
- **Grid snap is the law, not a mode** (F5): `_gridSnap` defaults `true` and every drag/resize
  quantizes. The popup toggle *stays this stream* — the user said "later remove the option" and
  removing it is a separate, trivial commit once the law has been lived with.
- **The taskbar height is derived and live** (F6): `PanelTaskbar.HEIGHT` is deleted; each bar
  measures one row and re-heights on every grid change, entries sizing off the row.

## Watch

Cells scale with the viewport and **text does not** ([I2](issues.md#i2)) — a 260px settings
popup becomes 8 cols, which is 265px at 1920 wide and 188px at 1366. That is the intended
consequence of the law, but it is where it will first look wrong; the named successor is a UI
scale derived from the row height, explicitly not built here. Off 16:9, cells stop being square
([I1](issues.md#i1)) — accepted, because a fixed cell count is exactly what makes a resize a
pure re-projection. And `defaults.json` is machine-authored by the popup's "Copy all panels"
button ([I4](issues.md#i4)): the exporter and the reader move in the same commit or the next
copy-paste silently reverts the corpus to pixels.

## Not in this stream (named, not built)

Removing the grid-snap row from the settings popup (the user's "later" — F5); retiring
`SnapMode`'s corner presets, which survive re-expressed in cells (F7); a font/UI scale derived
from the row (I2); any change to `pin` (which taskbar an entry lives in — unrelated to
placement).

## Exit

Every panel's position and size is four integers; resizing the browser from 1920×1080 to 1280×720
and back leaves every panel on the same cells with no drift and no panel under a bar; both
taskbars and every title bar measure exactly one row at three viewport sizes; toggling a title
bar leaves the body's bounding rect unchanged; a cleared profile opens the full layout on-grid.
The user's eyes close the stream.
