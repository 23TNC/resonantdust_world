# build-walls — the build panel, wall placement, and linked-tile rendering

_Work stream, opened 2026-07-28. Components: `client/webgl` (build panel, placement mode,
blueprint preview, linked-tile cells), `shared/codec` + `server/worker` + `server/edge`
(the `BUILD_WALL` event), `content` (smooth-wall DSL + visuals). User's brief: a new BUILD
panel with category sub-tabs (first: wall), icons per wall type (first: smooth wall, icon =
cell (0,1) of the `biome-tile/default/smooth/wall` linked atlas); left-click an icon →
WALL PLACEMENT MODE (right-click exits); left-drag forms a rectangle previewed with the
`biome-tile/default/blueprint/wall` textures CLIENT-SIDE ONLY; release → a `build_wall`
event through core to the event shard carrying start tile, end tile, and the u32 object
id; the worker builds IMMEDIATELY (perimeter tiles become walls). Linked-texture cell
selection by neighbors ships in the same pass._

## FUTURE INTENT (build toward, don't preclude)

Immediate building is the FIRST implementation. The documented destination: placement
creates a BLUEPRINT other players can see, and walls must then be BUILT (by pawns, over
time). The `BUILD_WALL` event's shape (start, end, object id) must not bake in
"immediate" — the worker's immediate path is one consumer of the same order.

## Grounded state (2026-07-28)

- **The linked-atlas pathway half-exists and names this work as its next phase**:
  `WorldBridge.thingTexture` already routes linked-category kinds to `<stem>/l` with
  `cell: 0` and the comment "for a linked kind it's the neighbour-context cell (still
  Phase 2 — canonical cell 0)". `TextureResolver.resolve(stem, map, cell)` already narrows
  linked-atlas cells (`cellFrames` cache; manifest `grid` + `pad` fields exist).
  `content/visual/things.rd` notes `linked/wall.smooth` was the one mastered linked kind.
- **Both masters are on disk**: `textures/biome-tile/default/smooth/wall` and
  `textures/biome-tile/default/blueprint/wall`.
- **Next action id**: 9 (`MOVE_STEP` = 8). `rd redeploy` hashes `shared/codec` into module
  inputs (movement-hardening fix), so a new verb publishes cleanly.
- ui-select delivered the input seam (mode-aware click handling slots beside the
  selection path), the panel system (tabs, persisted settings, remain-on-top), and the
  overlay idiom (OutlineOverlay's world-space quad pass — the blueprint preview's shape).

## D1 · The neighbor→cell formula (derived from the user's 16-row table)

With N/E/S/W = 1 when the same wall kind stands in that cardinal neighbor tile:

```
atlas x = N + 2·E          (0..3)
atlas y = 3 − (S + 2·W)    (0..3, row 3 = bottom)
```

Every row of the user's table satisfies it — (0,3) lone, (0,2) S, (0,1) W, (0,0) SW,
(1,3) N, (1,2) NS, (1,1) NW, (1,0) NWS, (2,3) E, (2,2) SE, (2,1) WE, (2,0) EWS,
(3,3) EN, (3,2) ENS, (3,1) ENW, (3,0) NESW. A unit-shaped table test pins all 16.

## Design decisions

- **D2 · Variant selection is CLIENT-side, type is SERVER-side.** A wall tile row stores
  the KIND (smooth wall) only; the renderer picks the cell from same-kind neighbors at
  expansion, and a changed tile re-evaluates its 4-neighbor ring (across row and zone
  boundaries — the seam case is the acceptance test).
- **D3 · The DSL owns the menu.** Smooth wall is a content-authored tile kind tagged as a
  WALL BUILD (`&tile.build.wall`-style tag — exact lane per DSL conventions at execution),
  with the linked visual + the blueprint visual referenced from content. The build panel
  populates categories/icons by scanning the corpus for build-tagged kinds — adding a
  brick wall in content alone must light up a second icon.
- **D4 · Placement is a MODE, selection-adjacent.** Entering build mode suspends the
  ui-select click semantics (left = start/drag rect, right = exit mode, middle pan still
  works); exiting restores them. The mode owns a preview overlay drawing the blueprint
  linked cells over the drag rect's PERIMETER, variant-aware against the preview shape
  itself (the same D1 formula, exercised CPU-side before any server round trip).
- **D5 · The event**: `BUILD_WALL` (action 9, client-issuable — edge allowlist), args
  start tile + end tile (packed positions per ACTIONS.md conventions) + u32 object id
  (the wall kind's def). VARIABLES/ACTIONS docs land BEFORE code. The worker expands the
  rect's perimeter and writes the tile shard (SET events — the proven P4 path); state
  fans; every client re-renders with correct variants.
- **D6 · Icons come from the resolver.** The panel's icon for a wall kind = the linked
  atlas's cell (0,1) (per the user), drawn from the same resolver frames the world uses —
  no separate icon assets.

## Not in this stream

The blueprint-as-entity phase (visible to others, built over time), non-wall categories
(the tab machinery ships, one category populated), wall demolition, occupancy/collision,
diagonal-aware autotiling (the 4×4 atlas is 4-neighbor by design).
