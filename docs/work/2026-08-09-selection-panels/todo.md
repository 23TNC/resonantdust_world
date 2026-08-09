# Plan — selection-panels

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** As for panel-grid: no test runner in `client/webgl`, so every item
lands on `npm run build` green + **no new** type errors against the 7-error baseline
(`2026-08-09-panel-grid` I11), plus measured browser evidence via
`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`.

**The per-panel option ritual** (P1 runs it twice — the same eight touch points each time):
`DomPanelOptions` → `PanelStateJSON` → the `_field` + its `defaultedBool`/`String` read →
getter + setter → `storageSet` → the popup row + `rowsByKey` → `serializeState` →
`resetToDefaults`'s SUFFIXES list. Missing the last two is how a setting silently fails to
survive a reset or a corpus export.

## P0 — the paper

- [x] Extend `docs/components/client/webgl/design/panel-layout.md` with the two new per-panel
      options (background values incl. `none`, click-through) and the layout/scale/interaction
      split they join (F1/F2). Acceptance: docs-check green.
- [x] Record the selection-surface split in the same design doc or a sibling: details = identity,
      intentions = queue, conditions = cards (F3–F6). Acceptance: docs-check green; work index row.

## P1 — the two options

- [x] `background` per-panel option over named values (`chrome` / `dim` / `none`), full ritual,
      applied to title bar, body AND footer (F1/I9). Acceptance: set `none` on a panel via the
      popup → all three surfaces transparent; survives reload.
- [x] VOID — there is no `PixiPanel`; it was deleted with the pixijs client and
      nothing overrides a background to transparent. See [I10](issues.md#i10); the stale
      comments that caused this item were rewritten instead.
- [x] `clickThrough` per-panel option — `pointer-events: none` on the root, existing chrome
      `auto` preserved (F2). Acceptance: with it on, a body click reaches the world; the title
      bar still drags.
- [x] Both options in the settings popup as rows, and in `serializeState` + `resetToDefaults`.
      Acceptance: Copy-All emits both; reset restores both.

## P2 — the conditions panel

- [ ] New `ConditionsPanel` owning `ConditionCards`, subscribing to `SelectionModel` directly with
      its own refresh cadence and `sameCards` identity check (F5/I8). Acceptance: selecting a pawn
      fills it; selecting a tile clears it.
- [ ] `ConditionCards` becomes a CHILD: delete the host-append, the rect/focus/minimize/open
      subscriptions, the mirrored z-index and the hand-rolled visibility predicate (F3).
      Acceptance: no reference to another panel's rect remains in the file.
- [ ] Cards set `pointer-events: auto` so they stay live inside a click-through panel (I1).
      Acceptance: with click-through on, a card click still expands and a world click still passes.
- [ ] The tooltip renders outside the panel's clip (I2). Acceptance: hovering a card in a
      one-row-tall panel shows the full tooltip, unclipped.
- [ ] Decide + implement the over-width behaviour, re-reading `CARD_H` / `PAD_BOTTOM` and the
      8-square fit note against a full-width panel (I3). Acceptance: 12+ conditions on one pawn
      stay reachable; capture.

## P3 — the intentions panel

- [ ] New `IntentionsPanel` owning `IntentStrip` unchanged, subscribing to `SelectionModel` +
      `IntentQueues` directly, carrying the `CANCEL_INTENT` sender (F4/F5). Acceptance: the
      selected pawn's queue renders; a circle click still cancels.
- [ ] The progress ring still computes per frame from the tic estimate after the move (F4).
      Acceptance: ring percentage advances live and snaps to truth after a hidden-tab pause.

## P4 — details slims down

- [ ] Remove the strip and the cards from `DetailsPanel`, INCLUDING both reserves — the
      `CARD_H + PAD_BOTTOM * 2` bottom band and the `STRIP_W` left shift (I7). Acceptance: no
      empty margin; details is name + tile + button + wash.
- [ ] Re-read details' min size now it holds two text rows and a button (I7). Acceptance: it
      cannot be resized to hide its own content.
- [ ] Keep the emotion wash working against a configurable background (F6/F1). Acceptance:
      capture of the wash over `chrome` and over `none`.

## P5 — the corpus

- [ ] `defaultCell` + `defaults.json` entries for both new panels (I6). Acceptance: cleared
      storage → both open on-grid at their authored cells.
- [ ] Conditions authored full-width bottom with `background: none`, `clickThrough: true`,
      `titleBarHidden: true` (F3). Acceptance: a cleared profile opens it transparent and
      click-through with no bar.
- [ ] Pick both panels' z-tiers deliberately against bug-sweep F1's table (I5). Acceptance:
      neither covers a panel the user reads; conditions sits over the world.
- [ ] Taskbar entries + icons for both, so a closed panel can be reopened. Acceptance: closing and
      re-opening each from the taskbar restores it in place.

## P6 — the truth

- [ ] Verify the conditions panel can be moved and restored from EDIT MODE while bar-less and
      click-through (I4) — if it can't, it ships with its bar. Acceptance: capture of the move.
- [ ] The full exit on camera: select a pawn, three panels populate, world clicks pass through the
      conditions panel, cards and tooltips live. Acceptance: captures in completed.md.
- [ ] Docs + memory truth pass; **the user's eyes close the stream**. Acceptance: docs-check
      green; the memory line written.
