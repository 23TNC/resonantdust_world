# Bug sweep — panel z-order, geo flashes, solid walls, honest outlines, the vanishing wolf

**What** (user, 2026-08-08): five bugs. (1) REMOVE "On Top" from panels — panels get a
Z-ORDER instead, retained properly; ties draw the LAST ACTIVE panel on top; the game
view sits at z-order 32, details at 40, chat at 48, build at 48, settings and debug
at 56. (2) Drawing: objects often FLASH A GEO BOX when changing direction — and these
flashes are NOT texture loads. (3) Wall tiles become IMPASSABLE. (4) Outline drawing
for MULTI-PRIM objects: no outline where another prim OCCLUDES it — the head should
occlude the body's outline (the user's screenshot: the selection ring crosses the
neck). (5) The wolf VISUALLY DISAPPEARS during movement under some conditions — at
least IDENTIFY the conditions.

## The stance

- **Numeric z-order replaces the band + On Top machinery** ([F1](forks.md#f1)):
  `DomPanel` already banded z-indices (gameview/inventory/overlay/dom/ontop) with a
  per-band recency counter — the shape survives, the NAMES go. A panel declares its
  z-order (the user's table); `bringToFront` advances a per-TIER counter so ties
  resolve to last-active WITHIN the tier and a lower tier can never climb over a
  higher one. The settings popup's On Top row and `_onTop` storage are DELETED
  (delete-don't-deprecate; stale stored keys are ignored — [I1](issues.md#i1)).
- **The geo flash is a swap artifact, not a load** ([F2](forks.md#f2) — the user's
  observation): a facing change re-bakes the warm prim, and the bake falls back to
  the geo silhouette even when the new facing's texture is RESIDENT (e/w is the SAME
  texture mirrored — a flip should never drop to geo). Investigate the re-bake path
  first; the fix direction is "keep drawing the OLD texture until the new one is
  bindable, and never geo between two resident textures". The legit first-load geo
  silhouette stays ([I3](issues.md#i3)).
- **Walls block routes through the ONE derived-pathability law** ([F3](forks.md#f3)):
  `pathable = false` on the wall tile def — the same corpus flag water carries
  (pathfinding F1); every consumer (worker A*, wasm glide, npc estimate) follows
  with zero new machinery. Built walls become real barriers: routes go around,
  enclosed destinations refuse (F5), and anything walled in is genuinely trapped
  ([I4](issues.md#i4)).
- **The outline is the OBJECT's union silhouette** ([F4](forks.md#f4)): today the
  overlay outlines the CARRIER prim's alpha alone and draws it topmost — the body's
  ring crosses the head. The outline pass stamps EVERY prim of the selected object
  into one mask (same subframe crops as the draw — [I5](issues.md#i5)) and
  edge-detects the UNION, so an edge interior to the object (the neck) never draws.
- **The vanishing wolf gets an evidence file before a fix** ([F5](forks.md#f5)):
  reproduce while recording, then bisect the render path with probes. The suspect
  list (ordered): zone-close dropping the mover mid-walk, a facing subframe with no
  registered crop (skip-draw), the eps/skip gate eating re-bakes, warm-cache
  eviction, the chase-snap, and the subtile-position sprite anchor. Deliverable:
  the CONDITIONS recorded in issues.md with captures — a fix lands only if the
  cause proves cheap ([I6](issues.md#i6)).

Authoritative docs touched: VARIABLES.md (the panel z-order table; the wall tile's
pathable flag rides the existing law).
