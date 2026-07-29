# Completed — ui-select

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-29 · P0 — input remap + the selection model (4/4)

Middle-button pan (button 1 only, preventDefault kills autoscroll; context menu suppressed on
the canvas), left CLICK-ON-UP with a 4 px slop (a longer travel is a drag — the branch the
drag-box pass will claim), right button = the move order. `SelectionModel`
(`game/world/SelectionModel.ts`): Map-backed Set of {pawn|thing|tile} + primary +
replace/add/toggle/clear + change event (D1 — the drag-box calls the same APIs with many).
Hit test (D2): `MoverLayer.pawnAt` (drawn box, painter order) → `Viewport.thingAt` (tight
silhouette box via the new `ShadowGather.tightBoxFor`) → tile. Move order (D3):
`WorldBridge.moveEntity` passthrough to the wasm client's MOVE_TO. **Verified live**
(synthesized pointer events + `__sel`): clicking the resting wolf → `{pawn, 0x30800001}`;
clicking a conifer → `{thing, 657}` (the exact prim); clicking verified-empty ground →
`{tile, (99,48)}`; a "ground" click that landed on pawn 0x30800003's 2-tile box correctly
selected the pawn (not a bug — the box IS there). Right-click with the npc wolf selected:
the order **arrived at the edge and fanned** (npc log 03:27:09: `move intent … tile_x=100
tile_y=52` — my order), the wolf walked it, and the npc's deadline-supersede reclaimed the
wolf at 03:27:18 — the movement-hardening chain-supersession interplay working as designed.
Middle-drag panned (+180 world px), left-drag panned 0 and left the selection untouched.
