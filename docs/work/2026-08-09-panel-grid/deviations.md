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

## 2026-08-09 — the migration DROPS old pixel rects instead of converting them

**What the plan says.** P3's migration item: "px → nearest cell, superseded keys deleted",
i.e. quantize each stored pixel rect against the current grid and keep the user's arrangement.

**What the code does.** `migratePanelLayoutStorage()` deletes the six CSS-string keys (and a
stale `gridSnap`) and writes the schema stamp. It does not convert. Each panel then re-derives
its cell rect from its content default on first open.

**Why (strong).** The stored pixel values were authored against *whatever viewport the user last
had* — and nothing records which. Quantizing them against the current grid is only correct when
those two viewports match; otherwise every panel lands somewhere plausible-but-wrong, which reads
as "the new grid misplaced my panels" rather than "the layout reset". A wrong-looking arrangement
is worse than a clean one, because the user cannot tell it apart from a bug. Dropping re-derives
from a rect someone chose deliberately. The cost is one lost hand-arrangement, once, on a dev
build whose panel layout is re-authored by this stream's P4 anyway.

**Status.** Resolved by design; the reasoning is in the code at `migratePanelLayoutStorage`.

## 2026-08-09 — grid snap stays GATED by its flag rather than quantizing unconditionally

**What the plan says.** P3: "Drag + resize quantize to cells unconditionally; `activeSnapGrid`'s
null path retires." [F5](forks.md#f5) says the same, while *also* keeping the popup's Grid-snap
row for this stream as an escape hatch.

**What the code does.** `activeSnapEdges()` still returns `null` when `_gridSnap` is false; the
flag now defaults `true` (and `contentDefaults.gridSnap ?? true`).

**Why (strong).** F5 asks for two things that cannot both hold: quantize unconditionally, AND
keep a working escape hatch on the popup. A toggle that is visible but inert is worse than no
toggle — it tells the user a lie about what the app will do. Since F5's stated reason for keeping
the row is that "this stream changes every rect in the app, and keeping one escape hatch while
that lands is worth one row of UI", the escape hatch has to actually work. So the flag keeps its
gate and defaults on. When the row is deleted (the named successor), the gate goes with it and
the unconditional form arrives for free.

**Status.** Open until the popup row is removed in the follow-up stream, at which point
`activeSnapEdges` loses its null path.
