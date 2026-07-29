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

## 2026-07-29 · P1 — selection feedback (3/3)

`OutlineOverlay` (new, `game/viewport/outlineOverlay.ts`): one world-space quad per selected
item drawn TOPMOST after the blit through the shadow-overlay projection; SPRITE mode samples
the co-packed surface frame's B lane and lights the OUTER edge (5 texel fetches over tiny
quads — D5's silhouette option shipped outright, no fallback demotion; forks note), BOX mode
is a screen-constant 2 px ring (tiles + unresolved frames). The scene rebuilds the outline
set per frame between `moverLayer.tick()` and the draw, so a selected pawn's outline follows
the chase same-frame. Cursor light (D4): a 1 px warm HOT carrier prim, light authored inline
(reach 2 tiles, castShadows off, hot class), moved on pointermove through the REAL placement
path — `buildCasters`' carried-light compare routes the scoped dirty. Title suffix: a new
"selection" `TitleSuffix` + late-registration (`setTitleSuffixResolver`) + value-refresh
(`refreshTitleSuffix`) on DomPanel; the viewport panel defaults to it. **Verified live**:
the conifer outline hugs the drawn silhouette exactly at zooms 1 AND 0.5 (captures); the
cursor sweep left a warm pool with **cold bakes 0** (304 hot) — the hot-light class held;
"Game View - tile 101, 47" appeared in the title bar + taskbar after a tile selection.
ART-BLOCKED (B1): the WOLF's outline is invisible for the same reason the wolf is — its
surface silhouette is empty under art-128's in-flight masters; the tracking mechanism
verified via the live prim reads (positions moving while selected) + the tree case.
