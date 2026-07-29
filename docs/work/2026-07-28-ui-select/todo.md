# Todo — ui-select

_Items tick in place; the box is the move. Design: [`README`](README.md). Visual items
verify at ≥2 zooms, tab VISIBLE._

---

## P0 · Input remap + the selection model

- [ ] Middle-click pan: `WorldScene` pointer handlers pan on button 1 (drag), leaving
      button 0 free; wheel zoom unchanged; context menu suppressed on the canvas.
      Acceptance: middle-drag pans, left-drag does NOT, wheel still zooms.
- [ ] `SelectionModel` (new, `game/world/`): Set of refs + primary + tile selection,
      replace/add/toggle/clear + change event — the API the future drag-box will call
      with many refs (D1). Acceptance: unit-shaped console drill; consumers below
      react to the change event only.
- [ ] Left-click hit test: `screenToWorld` → topmost warm mover by painter order, else
      cold thing by tight box, else the TILE (D2). Click selects (replace).
      Acceptance: clicking a wolf selects it; clicking a tree selects it; clicking
      ground selects the tile — probed via the model's state.
- [ ] Right-click move order: selected PAWN + right click → `WasmClient.moveEntity`
      (pawn ref, clicked tile) (D3). Acceptance: select the wolf, right-click a tile —
      the wolf walks there (edge MOVE_TO round trip live).

## P1 · Selection feedback

- [ ] Selection outline: viewport overlay pass outlining each selected prim (D5 —
      silhouette preferred, tight box fallback; log the fork). Acceptance: selected
      wolf/tree visibly outlined at zooms 1 + 0.5; outline tracks a walking wolf.
- [ ] Cursor light: a small-radius hot light carried by a cursor prim through the
      placement path (D4). Acceptance: a light pool follows the cursor; cold bakes
      stay 0 while it moves (hot-class isolation — counter probe).
- [ ] Title bar shows the selected TILE's world coordinates (clears when a non-tile
      selection replaces it). Acceptance: click ground → coords appear; click the
      wolf → coords replaced/cleared.

## P2 · Panels

- [ ] Details panel (`game/panels/details/`): a `DomPanel` rendering the selected
      object's details (kind/name, entity ref, tile, facing, speed, zone/macro) from
      the SelectionModel + content tables; empty state when nothing is selected.
      Acceptance: selecting the wolf populates it live; tile selection shows tile info.
- [ ] Remain-on-top: per-panel persisted boolean + toggle row in `PanelSettingsPopup`
      that promotes the panel's z band (D6). Acceptance: flag chat on-top → it renders
      above the viewport panel after reload (persistence proven); unflag → back.

## P3 · Verify

- [ ] Interaction drill: select wolf → right-click walk → outline tracks → details
      live-update → middle-pan + wheel-zoom during the walk; cursor light in tow;
      120 fps guard at zoom 1. Acceptance: numbers + captures in `completed.md`;
      the user's hands on the live tab are the final oracle.
