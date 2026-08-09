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
