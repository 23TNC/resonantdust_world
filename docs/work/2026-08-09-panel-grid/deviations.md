# Deviations — panel-grid

## 2026-08-09 — the grid is its own module singleton, not a field on `UiEditMode`

**What the plan says.** P1's first three items are written as "`UiEditMode`: `GRID_COLS` /
`GRID_ROWS` + integer edge tables", i.e. the grid stays where the geometry lives today.

**What the code does.** The grid is a new module, `ui/dom/PanelGrid.ts`, exporting a singleton
`panelGrid`. `UiEditMode` keeps only what its own doc-comment claims — the edit-mode flag and the
settings-popup reference — and loses `gridSize` / `getGrid` / `reservedTop` / `reservedBottom`.

**Why (strong).** `UiEditMode` is an **optional injected dependency**, and one panel deliberately
declines it: `SettingsMenu` passes no `uiEditMode` with the comment _"the user is toggling edit
mode from this dropdown; the dropdown itself doesn't need grid-snap / lock buttons on its own
chrome"_. That is a correct decision about *chrome*, but under [F5](forks.md#f5) every panel is
placed by the grid unconditionally — so keeping the grid on that reference would mean any panel
opting out of edit-mode buttons also silently opts out of **being placed at all**. `DomPanel`
already guards `if (!this.uiEditMode) return` in `snapToGrid`, so the failure would be silent:
the settings menu simply wouldn't participate. The grid is not a mode, it is the coordinate
system; it cannot be optional. Two more consequences confirm the split: the taskbars need the row
height and have no reason to know about edit mode, and the UI scale ([F9](forks.md#f9)) is
app-global.

**Status.** Resolved by design — this *is* the "one module" the design doc's grid section calls
for, and `docs/components/client/webgl/design/panel-layout.md` was written module-agnostic, so no
doc contradicts the code. The P1 item text is the only thing that named `UiEditMode`; the intent
(constants + edge tables in one place, no derived duplicates) is met.
