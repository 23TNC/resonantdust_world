# Completed — selection-panels

## 2026-08-09 — P0: the paper

`design/panel-layout.md` grew two sections and three invariants.

**"Appearance and interaction are per-panel options"** names a third category beside the layout /
scale split that stream already established: layout comes from the projection, scale from
`--ui-row`, appearance and interaction from per-panel options. It records that `background` is a
named string (`chrome` / `dim` / `none`) reaching all three chrome surfaces, that `clickThrough`
is `pointer-events: none` on the root with children needing to opt back in, that the two are
independent and must stay so, and the consequence that matters operationally — a bar-less
click-through panel has no grab handle and depends on edit mode forcing bars visible.

**"The three selection surfaces"** is the table: details = identity, intentions = the queue,
conditions = the cards, each subscribing to the selection model itself. It records *why* the split
exists rather than just asserting it — the strip used to live outside its panel, positioned
against another panel's rect, because `overflow: hidden` clipped anything wider than its parent —
and notes the one thing that still escapes its panel: the condition tooltip, since the card
carries no text.

Invariants 8–10 added: `background` reaches all three surfaces together; a click-through panel's
interactive children set `pointer-events: auto` explicitly or go silently dead; no selection panel
feeds another.

Verified: `bin/rd docs-check` clean (706 files, the 5 standing warnings).

## 2026-08-09 — P1: the two options

`BackgroundMode` (`chrome` / `dim` / `none`) and `clickThrough` land as per-panel options through
the full ritual: option → JSON field → field + read → getter/setter → storage → popup row +
`rowsByKey` → `serializeState` → `resetToDefaults`. `PanelSettingKey` gained both keys, which is
where the compiler caught the last step. One resolver, `backgroundCss`, is the only place a mode
becomes a colour — so a future `#rrggbb` value is an additive change there and nowhere else.

`applyBackground` writes to the **title bar, body and footer together** (I9). Painting only the
body would leave an opaque bar over a transparent panel, which reads as a rendering fault rather
than a setting.

**Verified live**, edit mode, on a real panel through the popup's own controls:

- Cycling Background `Chrome → Dim → None` turned the bound panel's title bar **and** body to
  `rgba(0, 0, 0, 0)` together, left every other panel at `rgba(20, 22, 30, 0.96)`, and persisted
  as `worldViewport.background = "none"`.
- Click through set the root to `pointer-events: none`, kept the title bar at `auto`, and a
  synthetic hit test at the body's centre resolved to `#app` — the click genuinely leaves the
  panel.
- Both survived a reload, and on a panel with a **visible** bar (chat) the decisive pair held
  together: `BODY_PASSES_THROUGH: true` **and** `TITLE_BAR_STILL_GRABBABLE: true`. That is the
  property the whole design leans on — a click-through panel is not stranded.

Two earlier hit tests read as failures and were **invalid measurements**, not defects: the world
viewport and details both carry `titleBarHidden`, so their bars have a zero-size rect and the
probe point landed on the taskbar. Worth recording because it is the second time in two streams
that a `[0,0,0,0]` rect has impersonated a bug.

### The plan error (I10)

"`PixiPanel`'s private `transparent` body override retires in favour of the option" is **void**:
there is no `PixiPanel`. It died with the pixijs client on 2026-07-28, leaving **39 stale
comment references** across four files describing a class that has not existed for weeks — and
those comments were the evidence this stream's README cited for "click-through already exists,
undeclared". The pattern they describe is real and is exactly what F2 generalizes; only the actor
is fictional. `ViewportPanel` shows the world by hosting the viewport's own `<canvas>` inside its
body, so nothing overrides a background anywhere (`grep 'background = "transparent"'` → nothing).

The ten references in `DomPanelStyles.ts` were rewritten to describe the **option**, because they
are the standing justification for the `pointer-events: auto` this stream depends on and leaving
them is how the next session repeats the mistake. Comment text only. The remaining references in
`DomPanel.ts`, `PanelManager.ts` and `PanelSettingsPopup.ts` are flagged for a separate sweep.
