# Issues — panel-grid (anticipated inventory)

Written 2026-08-09 at planning, before any code. Each is a thing we expect to hit; resolutions
land here as we hit them.

## I1 — cells go non-square off 16:9

58:33 = 1.758; 16:9 = 1.778. Square within 1.2% on a normal display, and progressively wider on
anything else — 3440×1440 gives 59.3 × 43.6px cells, so a "square" 4×4 panel is 237 × 174px.
Accepted by [F8](forks.md#f8) (a fixed count is what makes resize a pure re-projection), but it
means **no UI element may assume square cells** — no icon sized `1 cell × 1 cell` expecting a
square. Size icons off the row height (the smaller axis in practice) and let width breathe.

## I2 — cells scale, text does not — small viewports will look wrong first

This is the loudest consequence of [F3](forks.md#f3) and the most likely thing to be called a
bug. A cell is 33px wide at 1920 and 23.6px at 1366; the settings popup at 8 cols goes 265px →
188px while its labels stay 12px. At 1024×640 a row is 19px — under the ~20px a 12px title bar
needs. No floor is added: a minimum cell size would mean the viewport no longer divides into
58×33, i.e. the law would stop holding exactly where a user is most likely to notice.

The named successor if it does look wrong: derive a **UI scale** (font size, padding, icon size)
from the row height, so text scales with the grid instead of fighting it. Explicitly not built
here — record what breaks, with the viewport size, rather than patching a floor in.

## I3 — the localStorage migration, and the stale `gridSnap: "0"`

Every returning profile has `<storageKey>.{left,top,right,bottom,width,height}` in px, and some
have `gridSnap = "0"` — which under [F5](forks.md#f5) would strand exactly those panels
unsnapped forever, since `defaultedBool` prefers the stored value over the new default. Needs a
one-time versioned migration: a `panelLayout.v` stamp, and on a missing/old stamp convert each
panel's px rect to the nearest cell rect (using the *current* edge tables), write the cell keys,
and delete the superseded px + `gridSnap` keys. Silent-drop is fine — losing a hand-dragged
pixel offset is the point of the stream.

## I4 — `defaults.json` is machine-authored; exporter and reader must move together

The corpus at `client/webgl/src/content/panels/defaults.json` is produced by the settings
popup's "Copy All JSON" button and pasted back by hand. If the exporter still emits px while the
reader expects cells (or vice versa), the next copy-paste silently reverts the whole layout to
the old law and it will look like the grid "stopped working". Both sides in one commit, and the
acceptance is a **round-trip**: arrange → copy → paste into the file → reset → identical layout.

## I5 — the two-pass placement fight (`snapToGrid` then `applyAnchor`)

`snapToGrid()` writes `left/top/width/height` with `right/bottom: auto`, then immediately calls
`applyAnchor()`, which re-reads the resulting rect and *re-pins* the opposite edges — its own
comment says without that pass "the resize-corner math would fight the CSS and resize would
visually go the wrong way". Under [F7](forks.md#f7) placement must be **one** pass that writes
all four properties from the projection. Expect the resize-corner geometry
(`currentResizeCorner`/`applyResizeHandleCss`/`resizeClampSize`) to need re-deriving against the
cell rect rather than against a `getBoundingClientRect()` read mid-gesture.

## I6 — `rectChange` must fire on every projection, or Pixi content desyncs

`PixiPanel.syncRect` and the details-panel layout mirror the body rect via `onRectChange`;
nothing else notifies them. Today the panel's own `resize` listener fires it. After [F3](forks.md#f3)
the *grid* drives placement, so every re-projection path — grid change, migration, title-bar
toggle, drag end, resize end, snap, `resetToDefaults` — must fire it. `worldViewport` is the loud
failure case (it's `heightMode: full`, `titleBarHidden`, and holds the game canvas): if it
desyncs, the world renders at the wrong size and it's obvious. Good canary, watch it first.

## I7 — `heightMode: auto` rounds to rows and may oscillate

`applyHeight()` in `"auto"` mode sets height from `titlebar.offsetHeight + _contentNaturalHeight`
— a content-driven pixel number. Rounding it **up** to whole rows can feed back: the panel grows
a row, the content reflows into the extra width/height, reports a new natural height, and the
row count drops again. Round up, and only re-apply when the *row count* changes, not when the
pixel number does — a hysteresis of one row, not a re-layout per pixel.

## I8 — minimize under the body-rect law

Minimize currently collapses the panel to its title bar. With [F4](forks.md#f4) the stored rect
is the body, so a minimized panel is the title **row** only — and a minimized panel whose title
bar is hidden is *nothing*. Decide at implementation: either minimize force-shows the title bar
(consistent — a minimized panel must present a way back), or `hideMinimizeBtn` is implied by
`titleBarHidden`. Note that UI edit mode already force-shows title bars, so the invariant partly
exists.

## I9 — the login `FormOverlay` is a `DomPanel` at `left/top: 50%`

`scenes/login/FormOverlay.ts` uses percentage centring, which no other panel does and which the
cell projection doesn't express. It must be re-authored as a cell rect. 58 is even, so a panel
with an odd `cols` cannot be exactly centred — pick even widths for centred panels, or accept a
half-cell bias. Also check nothing else in the login scene assumes the overlay is centred by CSS
rather than by placement.

## I10 — the settings popup and settings menu are themselves panels

Both quantize like everything else, and both are narrow, glyph-dense, and label-driven: the
popup is 260px (8 cols) with a two-column row layout, the menu 200px (6 cols). They're the first
place I2 bites, and the popup is *how the user fixes a broken layout* — if it clips or overflows,
the recovery path is gone. Verify these two at the smallest viewport we care about before
declaring P3 done; `DomPanel.resetAllToDefaults()` is the backstop if they do wedge.
