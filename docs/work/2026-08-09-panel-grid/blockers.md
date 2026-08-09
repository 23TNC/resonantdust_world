# Blockers — panel-grid

## 2026-08-09 — the final on-camera pass needs a REAL window resize (open)

**What blocks.** The last open item is the exit criterion: the layout looked at across three
viewport sizes, through a resize round-trip, a title-bar toggle and a fresh profile. Everything
except the *real resize* is done and recorded in `completed.md`.

**Why it needs you.** This environment cannot resize the browser window — `window.outerWidth`
reads 0 under the remote Chrome bridge, and `resize_window` has no effect. Every non-native
viewport in this stream was produced by overriding `window.innerWidth` / `innerHeight` and
dispatching `resize`. That genuinely exercises the grid (it reads exactly those globals, and
`--ui-row`, the bar heights and the projections all track), but it does **not** drive CSS's own
resolution of `position: fixed` — a bottom-anchored panel's `bottom: 28px` still resolves against
the real viewport. So any measurement of a CSS-resolved position at a simulated size is unreliable
by construction, and a genuine visual check at 1920×1080 / 1366×768 / 1024×640 is not something
this session can produce.

**What IS verified**, so you know what you're checking rather than re-deriving it:

- the projection math exhaustively off-browser (848,656 round-trips × 5 viewports; 72,864 clamp
  cases; zero failures),
- every JS-computed value at five simulated viewports (bar heights, `--ui-row`, the scale),
- the full DOM exactly, at the native 1862×917: bars one row, title bars matching their own row,
  panels on grid and clear of both bars, cell rects byte-identical through a resize round-trip,
  the body bit-identical through six title-bar toggles, drag and resize landing on grid with the
  anchor pinned, zero pixel keys in storage.

**Suggested path.** Open `:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`, drag the window edge
between a few sizes, and look for: a panel crossing a taskbar, a seam or overlap between abutting
panels, and whether the chrome reads too light (it is ~13% smaller than before — see
[I2](issues.md#i2)). Two knobs if it does: the `0.375` ratio in `index.html`'s `:root`, and
`GRID_ROWS` in `PanelGrid.ts` — fewer rows means a taller row and a larger everything.

**Also worth your call while you're looking:** `build`, `chatPanel` and `details` all carry
`snap: "bottom-left"`, so they stack in one corner and overlap by default. Pre-existing and
unrelated to the grid, so it was left alone — but it is the most visible thing about the default
layout.
