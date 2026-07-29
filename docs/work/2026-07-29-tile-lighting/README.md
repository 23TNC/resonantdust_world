# tile-lighting — tiles join the COLD lighting class; blueprints ride HOT

_Work stream, opened 2026-07-29. Component: `client/webgl` (`shadowGather`,
`coldShadowData`, `WorldBridge`, `blueprintOverlay`), possibly a content lane for wall
height. User's brief: add lighting to TILES via the COLD lighting — "I think we can do
this because tiles update less frequently than most things"; the BLUEPRINT shown while
placing walls is EPHEMERAL and should be lit via the HOT path._

## The user's cost model (ratified)

- **Tiles → COLD class.** A tile changes only on a build/terrain event, so its lighting
  participation (records, caster silhouettes, receiver N·L) bakes into the cold maps and
  re-bakes only when the tile changes — exactly the cold tier's contract. A build event's
  override already dirties the affected cells; lighting dirt rides the same change.
- **The blueprint → HOT class.** The placement preview changes every pointer move and
  lives seconds — it must never touch a cold bake. Its lighting rides the hot lightmap
  (the mover/cursor-light class), dropped with the preview.

## Grounded state (2026-07-29)

- Ground texels are LIT today (ambient + cold/hot pools + shadows cast ONTO them) but are
  not PARTICIPANTS: the lightmap gives ground `ndl = 1` (no N·L — "ground keeps falloff"
  was explicit in pawn-render P1) and tiles never cast.
- The wall tiles (the FIRST textured tiles, build-walls) RESOLVE a def — probed live:
  def with a full-tile 128×128 tight box, atlas maps albedo|layers|normal|surface — but
  **no billboard record is minted** for them. P0 pins where tile prims fall out of the
  caster/receiver pipeline (`buildCasters` walks every standing cold prim, so something
  filters them — found, not assumed).
- The blueprint preview (`BlueprintOverlay`) draws AFTER the blit, raw albedo × alpha —
  fully unlit.
- The receiver machinery (fine/coarse receiver maps, `billboardNormal`, the conservative
  hot classification) and the class matrix (hot-sync P4: hot = pure correction over cold)
  are the rails both halves ride; nothing new is invented, tiles/blueprints join
  EXISTING classes.

## Design decisions (P0 confirms against the audit)

- **D1 · A tile participates as a FLAT receiver + an optional caster.** Receiver: tile
  texels get per-light N·L through the tile's atlas NORMAL (the walls ship one) instead
  of ground's `ndl = 1` — baked cold. Caster: a WALL casts like a standing structure —
  its silhouette/height likely wants a content lane (`&tile.height`-style; 0 = flat, no
  cast) so grass never casts and walls do. The tile card is NOT a leaning billboard —
  the caster geometry decision (flat-lid vs standard card at tile width) is P0's.
- **D2 · Only TEXTURED tiles participate.** Flat-tint ground (`white` stem) keeps
  today's exact path — `ndl = 1`, no records; participation keys off the def's resolved
  maps (a tile without a normal map has nothing to receive WITH). Zero cost where
  nothing changed.
- **D3 · The blueprint registers HOT records while it lives.** Preview tiles join the
  hot class the way movers do (records + hot rects per drag move, the ONE-dirty idiom),
  so the preview is lit by torches and shadowed under trees; release/exit drops the
  records and hot texels re-bake clean. The overlay keeps drawing the albedo — lighting
  comes from the hot lightmap at those texels through the normal blit.
- **D4 · Change cadence**: a built wall's lighting re-bake = the cold rects its cells +
  reaching lights cover, ONCE per build — the user's "tiles update less frequently"
  premise, measured in the drill (cold bakes on build, zero during walks/drags).

## Not in this stream

Walls as OCCUPANCY/pathing, wall-top rendering (n/s face art), ambient occlusion
authoring for tiles, terrain (non-wall) normal mapping beyond what D2 gives free.
