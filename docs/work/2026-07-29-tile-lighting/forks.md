# Forks — tile-lighting

_Decisions resolved during execution, with reasoning. Expected: the tile caster geometry
(flat-lid vs standard card, D1), the participation gate (D2), and the wall-height lane's
authoring shape._

## F1 · The tile caster geometry — the STANDARD cold card, gated by an authored height

A wall casts as the existing card (width = the def's box, height = the art's box — one tile
for the wall atlas), through `casterCover` untouched: the wall art's silhouette is a full
square, so the card throws a SOLID quad shadow that reads like a structure's. Rejected for
now: a perpendicular/box model (real complexity; the ns-shadows card exists if a wall ever
wants it) and any un-gated participation (a FLOOR must never cast — the gate is the height
lane, 0/absent = flat). Receiver classification rides the same record: wall texels classify
on-billboard → the wall's atlas NORMAL feeds per-light N·L, and tree shadows CLIMB walls via
the existing on-billboard shadow path — both free once the record exists.

## F2 · The participation gate — `&tile.height` (a data lane), carried as `Primitive.litTile`

A tile participates iff its `:data @define` hook authors `&tile.height` > 0 (wall_smooth = 1).
Plumbed like `tile.build`: loader `tile_heights()` → wasm `tileHeights` → the bridge's def
table → `tilePrimSpec` sets a new `Primitive.litTile` flag → `standingPrims()` includes
`litTile` prims alongside `zIndex >= 1`. Why a lane and not "has a normal map": textured
FLOORS are coming (dirt/planks with normals someday) and must not become casters by accident;
receive-only participation can extend the lane later (e.g. height 0 + normal = receiver-only)
without re-plumbing. `white` ground authors nothing → bit-identical (D2).

## F3 · The blueprint's lighting — the overlay SAMPLES the live lightmaps (no records)

"Lit via the hot path" lands as: `BlueprintOverlay` binds the SAME cold+hot lightmaps +
ambient the display blit uses and lights its own pixels with the blit's formula — torch
pools light the preview, tree shadows darken it, and the hot map's live content (cursor
light included) applies. Zero records, zero bakes, residue-free by construction — stronger
ephemerality than hot records (nothing to drop on release). NOT done: the preview as a hot
RECEIVER/CASTER (its own N·L or cast shadows) — beyond an ephemeral ghost's needs; the hot-
record path stays available if the user wants the preview to self-shade later.
