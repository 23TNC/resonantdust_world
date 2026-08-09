# Forks — panel-grid

## F1 — the grid spans the FULL viewport (33 rows *including* both bars)

2026-08-09. The user gave two constraints that only reconcile one way: "we have 58x33 grids"
and "the top and bottom task bars will be 1 grid tall". If the grid covered only the safe area
between the bars, a row would be `(innerHeight − 64) / 33` = 30.8px at 1080p while the bars stay
32px — the bars would be 1.04 grids tall, and every future "N grids" statement about vertical
space would carry that lie. So the grid is the **whole viewport**: 33 rows, row 0 = the top
taskbar, row 32 = the bottom taskbar, rows **1..31** = the panel field (31 rows).

Sanity: 1920/58 = 33.10, 1080/33 = 32.73 — a ~33px cell, which is where the number pair
plainly came from, and within 1.2% of square on any 16:9 viewport (58:33 = 1.758 vs 1.778).

Rejected: 58×33 over the safe area (bars can't be 1 row — above); 58×35 with the bars as rows
0 and 34 to keep 33 *usable* rows (invents a number the user didn't say, and "58x33" is the
thing they'll check).

## F2 — integer edge TABLES, not a float step

2026-08-09. `getGrid()` returns `stepX = viewW / cols` today and callers do
`round((px − origin) / step) * step`. Two panels meant to abut compute their shared boundary
from different sides of that float and land 0.4px apart — which is exactly the residue visible
in the checked-in corpus (`31.8977px`, `56.3295px`, `462.727px`). Instead the grid publishes
two tables, `edgeX[0..58]` and `edgeY[0..33]`, each entry `Math.round(i * extent / n)`. A cell
rect is a *difference of table entries*, so:

- every boundary is one integer, shared by both neighbours — no seams, no overlaps;
- cells vary by at most 1px and tile the viewport exactly (`edgeX[58] === innerWidth`);
- `nearestCell(px)` is a search/divide on the table, and round-trips exactly.

Rejected: floor-to-integer cell size with the remainder dumped on the last row/column (a visibly
fat edge cell); float steps with px rounded only at write time (that *is* today, and it drifts
because the rounding happens per-gesture, not per-edge).

## F3 — the CELL is the persisted unit; pixels are a projection

2026-08-09. This is the fork the user's "this way when the screen re-sizes we can re-size to the
grid" actually asks for, and the only new machinery in the stream. A panel persists
`col / row / cols / rows` (ints) instead of six CSS strings. Placement is one function,
`project(cellRect) → {left, top, width, height}` read off the edge tables, and a viewport resize
is *just* re-running it: the cell rect never changes, so a resize cannot drift, cannot round, and
cannot lose a panel off-screen.

Consequences taken deliberately: panels **scale** with the viewport rather than holding a pixel
size (that's the point — and where text will first look wrong, [I2](issues.md#i2)); the old
`left/top/right/bottom/width/height` storage keys are superseded and migrated once
([I3](issues.md#i3)); `defaults.json` and the "Copy all panels" exporter both move to cells in
the same commit ([I4](issues.md#i4)).

Rejected: cells for position but pixels for size (half a coordinate system; the resize case
that motivated the stream is a *size* case); storing both and treating px as a cache (two
sources of truth for one rect is how the current drift got in).

## F4 — the persisted rect is the BODY's; the title bar is the row above it

2026-08-09. The user: "Title bars will be 1 grid tall. When title bars are hidden the height
will be reduced by 1 so that the body does not move." Two readings — the body keeps its *size*,
or the body keeps its *screen rect*. We take the strict one, because it's the one that makes the
toggle free: **the stored cell rect describes the body**, and a visible title bar is chrome
occupying the row immediately above it. Outer box = `rows + (titleBarVisible ? 1 : 0)`, top edge
= `row − (titleBarVisible ? 1 : 0)`.

Then toggling the bar mutates *nothing* persisted, and the body's `getBoundingClientRect()` is
bit-identical before and after — which is a stronger, checkable acceptance criterion than "looks
about the same". Falls out: a titled panel's body must start at row ≥ 2 (its bar can't occupy
row 0, the taskbar), an untitled one may sit at row 1, and "hide the title bar to reclaim the
top row" becomes a real, sensible move.

Rejected: stored rect = the outer box, with hide doing `row += 1; rows −= 1` (same pixels, but a
chrome toggle now rewrites persisted geometry, and the rect's meaning flips depending on a
boolean); stored rect = outer box with the top pinned (the current behaviour — the body moves
*and* resizes, which is what the user is asking us to stop).

## F5 — grid snap becomes the LAW; the toggle survives exactly one stream

2026-08-09. `_gridSnap` defaults `true`, `contentDefaults.gridSnap ?? true`, and drag/resize
quantize unconditionally — `activeSnapGrid()` returning `null` stops being reachable. The user
said "default grid snap to true, and later remove the option from the menu", so the popup row
**stays** here: this stream changes every rect in the app, and keeping one escape hatch while
that lands is worth one row of UI. Its removal is a named successor, not a phase.

Note the vocabulary collision this leaves behind: after the toggle goes, "snap" in this codebase
means only `SnapMode`'s corner presets (F7). Grid snap has no name because it is no longer a
mode.

Rejected: deleting the row now (the user sequenced it "later", and a mid-migration escape hatch
has real value); leaving the default `false` and flipping it per panel in `defaults.json` (four
panels already do that, and it's how we got a half-obeyed law).

## F6 — the taskbar height is DERIVED from the grid, and live

2026-08-09. `const HEIGHT = 32` and `PanelTaskbar.HEIGHT` are deleted. Each bar takes the grid
and sizes to one row — top bar `edgeY[1] − edgeY[0]`, bottom bar `edgeY[33] − edgeY[32]` — and
re-applies on every grid change. `UiEditMode.reservedTop/reservedBottom` stop being constructor
parameters and become derived getters (one row each), so `main.ts` stops wiring a constant it
had to keep in sync by hand.

The bar's *contents* size off the row too: entries are `height: 100%` minus the existing 4px
vertical breathing room rather than a hard 24px, so they stay proportionate when a row is 23px
(1366×768) or 44px (2560×1440).

Rejected: keeping `HEIGHT = 32` and asserting the row happens to be ~32 (the assertion fails on
every non-1080p viewport); making the bars 2 rows for touch comfort (the user said 1).

## F7 — `anchor` becomes resize-pivot-only; `snap` survives, re-expressed in cells

2026-08-09. `AnchorMode`'s doc-comment already says it "only affects resize behaviour", but
`applyAnchor()` writes `left/top/right/bottom` CSS and is re-run on every resize, snap and
minimize — because pinning a corner was the only way a panel survived a viewport change. F3
takes that job. So `anchor` keeps exactly one meaning: which corner stays put during a resize
gesture (and therefore which corner carries the grab handle). Placement always writes the
projected `left/top/width/height` with `right/bottom: auto`, in one pass, from one function
([I5](issues.md#i5) — today two passes fight).

`SnapMode` (glue to a corner of the safe area, dragging disabled) is untouched in behaviour and
re-expressed against the field: `top-left` = `col 0, row 1`, `bottom-right` = the far corner of
rows 1..31. Retiring it is out of scope — it's a placement *preset*, orthogonal to the grid.

Rejected: deleting `anchor` outright (the resize pivot is a real, used affordance); folding
`snap` into the grid as "cell presets" (scope creep in a stream that already touches every rect).

## F8 — fixed 58×33 on every aspect: cells stretch, they do not stay square

2026-08-09. The cell count is constant regardless of viewport aspect, so on a 21:9 ultrawide a
cell is 59.3 × 43.6px and panels stretch horizontally. The alternative — hold cells square and
vary the column count — would make a resize *reflow* the layout instead of re-project it, and
every panel's stored `cols` would mean a different fraction of the screen at each width. The
whole value of F3 is that a resize is arithmetic on unchanged integers.

`GRID_COLS = 58` / `GRID_ROWS = 33` live as named constants in one module so the pair is
retunable by editing one line, but they are not a per-viewport function.

Rejected: aspect-adaptive column counts (kills the invariant); a minimum cell pixel size with
overflow scrolling (introduces a second layout mode for small windows — see
[I2](issues.md#i2) for the successor we'd rather have).
