# Todo — build-walls

_Items tick in place; the box is the move. Design: [`README`](README.md) (D1 formula, D2–D6,
the FUTURE-INTENT note). Visual items verify at ≥2 zooms, tab VISIBLE._

---

## P0 · Content + the linked-cell formula

- [ ] DSL: a smooth-wall TILE kind in content — build-wall tag (lane per DSL conventions),
      linked visual → `biome-tile/default/smooth/wall`, blueprint visual →
      `biome-tile/default/blueprint/wall`; corpus republishes + hot-swaps. Acceptance: the
      client content bundle exposes the kind + both stems; manifest carries the 4×4 grid.
- [ ] The neighbor→cell helper (shared client util): mask N|E|S|W → atlas (x, y) via D1,
      packed to the resolver's cell index. Acceptance: a 16-row table test reproduces the
      user's mapping exactly.
- [ ] Build-menu query: a content-bundle scan returning build-tagged kinds grouped by
      category (wall first). Acceptance: console probe lists the smooth wall from content
      alone; a second tagged kind (temp fixture) appears without client code changes.

## P1 · Linked-tile rendering (the pathway's documented Phase 2)

- [ ] `WorldBridge` tile expansion: a linked tile's `cell` comes from same-kind cardinal
      neighbors (D1 helper), not canonical 0; a tile change re-expands its 4-neighbor ring,
      across row AND zone boundaries. Acceptance: a hand-painted L of wall tiles renders
      ends/corners/tees correctly, including across a zone seam; zoom-stable at 2 zooms.

## P2 · The build panel + placement mode

- [ ] `BuildPanel` (`game/panels/build/`): DomPanel with category sub-tabs (wall), icons
      per build-tagged kind drawn from the resolver (linked cell (0,1) — D6), populated by
      the P0 query. Acceptance: the panel shows the smooth-wall icon from content alone;
      panel settings (incl. remain-on-top) work on it.
- [ ] Placement mode: icon click enters; right-click exits; left-drag forms the rect
      (middle-pan stays live); ui-select's click semantics suspended while active and
      restored on exit. Acceptance: mode in/out drill via `__sel` (no selections while
      active) + cursor behavior.
- [ ] Blueprint preview: an overlay drawing the drag rect's PERIMETER with the blueprint
      linked atlas, variant-aware against the preview shape (D1 CPU-side), client-side
      only. Acceptance: dragging shows corners/edges/tees correctly; release or exit
      clears it; nothing hits the server until release.

## P3 · The BUILD_WALL event end-to-end

- [ ] Docs first: `ACTIONS.md` + `VARIABLES.md` rows for action 9 `BUILD_WALL` (start
      tile, end tile packed per conventions, u32 object id) with the FUTURE-INTENT note
      (the same order later creates a blueprint instead of building). Acceptance:
      docs-check green before code.
- [ ] codec + edge + core: action 9 in `shared/codec`, edge CLIENT_VERBS allowlist,
      wasm-client `buildWall(startX, startY, endX, endY, objectId)` + `WorldBridge`
      passthrough. Acceptance: a console-issued order reaches the event shard (edge log).
- [ ] Worker: handle BUILD_WALL — expand the rect perimeter, write the wall kind into the
      tile shard (SET path), immediate build. Acceptance: tiles land in the shard and fan;
      module redeploy publishes cleanly (shared/codec hashed).

## P4 · The drill

- [ ] End-to-end: enter mode → drag a rect → release → walls appear with correct variants
      (corners, tees at overlaps with existing walls), persist across reload, and the
      perimeter's NEIGHBOR ring re-variants; 120 fps guard at zoom 1. Acceptance: numbers +
      captures in `completed.md`; the user's hands are the final oracle.
