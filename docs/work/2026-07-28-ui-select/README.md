# ui-select — mouse interaction, selection, and panel layering

_Work stream, opened 2026-07-28. Component: `client/webgl` (`WorldScene` input,
`ui/dom` panel system, `game/panels`, viewport overlay), `client/core` (nothing new —
the move path exists). User's brief: left click select, middle click pan; right click
with a pawn selected sends a move order (webgl → core → edge) from the pawn's location
to the clicked tile; re-implement a details panel for the selected object; panels gain
a REMAIN-ON-TOP setting (set in the panel settings dialogue) so chat/options/debug can
sit above viewport-like panels; the cursor carries a small-radius light; selected
objects are outlined; a selected tile shows its world coordinates in the title bar.
Drag-selection (multi-select) comes in a LATER pass — build the selection model
knowing that._

## Wiring facts (grounded, 2026-07-28)

- **The move order path already exists end-to-end**: `WasmClient.moveEntity(entity, x, y)`
  → core `web.rs move_entity` → edge `MOVE_TO` (allowlisted client verb). Right-click is
  input wiring, not protocol work.
- **Panel z-order is already banded**: `DomPanel` has per-panel CATEGORY z-index bands +
  `bringToFront` within a band, and per-panel persisted settings (localStorage) with
  toggle-row precedent (Pin) in `PanelSettingsPopup`. Remain-on-top = a persisted flag
  that promotes the panel's band.
- Input today: `WorldScene` owns pointerdown/move/up + wheel on the viewport canvas.
  `Viewport.screenToWorld` exists.

## Design decisions

- **D1 · Selection is a MODEL, not a click handler**: `SelectionModel` holds a `Set` of
  selected refs + a primary, with replace/add/toggle/clear APIs and a change event.
  Single-click uses replace; the future drag-box calls the same APIs with many refs.
  Consumers (outline, details panel, title bar, right-click) read the model only.
- **D2 · Hit test order**: screen → world via the canonical `screenToWorld`; candidates =
  warm movers first (painter order — topmost wins), then cold things, by TIGHT box
  containment; a miss selects the TILE under the cursor (tile selection and object
  selection are both selections — one model).
- **D3 · Right-click = move order for a selected PAWN only** (owned-pawn checks are a
  later concern — no ownership model yet). No selection → right-click does nothing.
  Context menu suppressed on the canvas.
- **D4 · The cursor light is a HOT light** carried through the real placement path (a
  cursor carrier prim, `markLightDirty` on move — the front door, no bespoke array).
  Small radius (~2 tiles), modest intensity. NOTE: this is the first LIVE hot light —
  it exercises the hot-light matrix cells (hot light × cold/hot prims) that pawn-render
  verified only structurally.
- **D5 · Outline v1** = a viewport overlay pass around each selected prim; prefer the
  sprite's silhouette (surface alpha edge) and fall back to the tight box if the
  silhouette read is disproportionate — decide in-stream, log the fork.
- **D6 · Remain-on-top** = a per-panel persisted boolean in the settings popup that
  moves the panel to a higher z band; default off. The viewport panel stays in the low
  band, so chat/options/debug/details float above it when flagged.

## Not in this stream

Drag-box selection (the model is shaped for it), ownership checks on move orders,
touch input, path preview lines, group move formations.
