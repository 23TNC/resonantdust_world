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

## 2026-08-09 — P2 + P2b: the chrome on rows, and the UI scale

Taken together and slightly out of plan order: P2b's foundation (`--ui-row` + the `:root` block)
landed **before** P2's chrome items, because the dependency runs that way — the chrome is
expressed in the scale, not the reverse. Same items, reversed order, nothing added.

**The layout/scale split**, which the code now leans on everywhere: *layout* (position, size)
comes from the projection; *scale* (fonts, padding, gaps, glyph boxes) comes from `--ui-row`.
The taskbars are grid rows, so they are **projected** (`project({row: 0|32, rows: 1})`), not
styled with `height: var(--ui-row)` — that would be nearly right and wrong by up to a pixel,
since integer edge rounding lets row 32 differ from row 0. Same reasoning gave title bars
`panelGrid.rowHeightAt(nearestRow(outerTop))` rather than the published row height, and added
`nearestRow` to the grid for it.

`PanelTaskbar.HEIGHT` / the `HEIGHT` const are deleted. `index.html` carries the whole derived
scale as `calc()` in one `:root` block: `--ui-font` (3/8 row, `max(9px, …)`), then `-sm/-md/-lg/
-xl` as multiples of the **floored** base — deriving those from the row independently would let
the ladder INVERT once the floor engages (at a 19px row an unfloored "large" is 8.3px, under the
9px base). Every `fontSize` px literal under `client/webgl/src` is gone (0 remain), along with
the fixed paddings, gaps and button widths in `DomPanelStyles` + `PanelTaskbar`.

**Two real defects found by measuring rather than assuming:**

1. **`PANEL_CSS` was `content-box` with a 1px border.** `project()` returns border-box pixels,
   and placement writes them straight to `style.width`/`height` — so every panel would have sat
   2px larger than its cell rect, and each title-bar toggle grew the panel by 2px, compounding
   (observed: body height creeping 253 → 254 → 256 across toggles). Fixed with
   `boxSizing: "border-box"`, which the cell law requires independently of this bug. The world
   viewport's height went 862 → **861**, exactly the safe area (917−28−28).
2. **`heightMode` and F4 are contradictory** — resolved as [F10](forks.md#f10). A panel whose
   height is locked to the field cannot also be free to grow a row for its title bar, so
   height-locked panels are exempt and the title row comes out of their body. Found by testing
   the toggle on the world viewport, the one panel where both rules meet.

**Verified live** (app running, viewport 1862×917, measured through real click paths — entering
UI edit mode via the settings menu and clicking the popup's Title-bar row):

- **Both bars are exactly one row** at 1862×917, 1920×1080, 1366×768, 1024×640, 3840×2160 —
  measured heights 28/33/23/19/65 against independently computed `edgeY` differences, all match,
  and the bottom bar's top edge sits exactly on `edgeY[32]` (889 at the native viewport).
- **Title bar = its own exact row**: the Details panel's bar measured 28px against
  `rowHeightAt(nearestRow(top))` = 28.
- **THE BODY DOES NOT MOVE.** On a free-height panel, six consecutive title-bar toggles left the
  body at `57,1,1861,832` — bit-identical every time — while the outer box alternated
  `56,833` ↔ `29,860`, one row apart in both top and height. On a height-locked panel the mirror
  image held: outer pinned at `28,861` while the body took the row (`29,…,860` ↔ `57,…,832`),
  round-tripping exactly with zero drift.
- **One `--ui-row` write per resize event**, counted by wrapping `setProperty`: exactly 1 at each
  of four viewports. Entries stay inside the bar at every size (14px entry in a 19px bar at
  1024×640).

**Where the acceptance wording was looser than reality.** Two items said the scale "resolves to
12px" / is "pixel-identical to today's" at a 1080-tall viewport. Measured: a 1080-tall viewport
gives a **33px** row (not 32), so the base font is 12.375px and each derived metric is ~3% above
today's. The ratios reproduce today's numbers exactly at a *32px* row, which is what F9 claims;
the criterion was written assuming 1080/33 = 32. At the real 1862×917 viewport the row is 28px
and the base font 10.5px — the chrome is ~13% lighter than before, exactly the consequence
predicted in [I2](issues.md#i2) and still owed a verdict from the user at P5.
