# texture-generalization — linked textures ride definition_data; tiles enter via presence

_Work stream, opened 2026-07-29, authored by the USER (design ratified verbatim below);
supersedes tile-lighting's per-prim approach (that stream is paused with its audit + the
manifest-race fix landed). Components: `client/webgl` (coldShadowData, shadowGather,
WorldBridge), `shared/dsl` (+ wasm bundle), `content`, `docs/VARIABLES.md`._

## The user's design (2026-07-29)

- **Linked textures use the SAME `definition_data` billboards use.** The full linked atlas
  is the texture frame: `frame_x/y` locate it in the lod atlas; `span` is the atlas's size
  in tiles (4 for the walls' 4×4); the frame anchor is TOP-LEFT for linked. All the other
  maps (normal/surface/layers) come with the frame exactly like every other texture.
- Walls are type **biome-tile** — they ride the same dense table as ground (shared
  type+subtype), so a `u16 kind_reference` fully names them.
- **The DSL carries the linked geometry**: the atlas's tile count in width/height (4×4),
  a new **`internal_padding`** (per-tile inset INTERNAL to the atlas, in the same UNITS as
  `frame_width/height`), and **`cast_shadow`** (bool). With padding 1, a wall tile's window
  is `frame_width/height = 14×14` units and `offset_x/y = 1 unit` (8 px) — the padding
  inset. Every base tile (grass/dirt/sand/water/stone) is in the DSL already; all author
  `cast_shadow false` — walls too for now (wall shadows are a later stream).
- **The def's `u4 type` = the OBJECT type** (biome-tile here) — logic keys on it, one path
  for all biome-tiles.
- **`rotation` is a per-type MODE for tiles** (they don't rotate): `0` = plain (use the
  tile as-is); `1` = AUTOTILE — pick one of 16 cells from surrounding tiles; `2` = WORLD —
  index by world tile x/y (large ground atlases like dirt; implemented later); `3`
  reserved. Walls are rotation 1.
- **`prim_presence`'s FIRST index is reserved for the TILE**, populated with the tile's
  `definition_data` id DIRECTLY — no `prim_data` per tile (frugal: one def per KIND, the
  presence bucket's own tile coordinate IS the position). Def ids are 1-based, so `0`
  keeps meaning "no tile". Lighting walks presence already, so tiles join it
  automatically; readers special-case slot 0 (def id, not a billboard id — check the
  def's `cast_shadows`, never dereference prim_data).
- **`definition_data` R's `u1 reserved` (bit 24) becomes `cast_shadows`** — the system
  reads it to skip non-casters during shadow generation.
- **Adjacency is resolved AT READ TIME from presence**: a rotation-1 tile looks up its 4
  neighbors' slot-0 defs; a neighbor whose rotation is also 1 is wall-like → connected →
  the D1 cell formula picks the frame. Lookups are fine because tile visuals change
  RARELY and bake COLD; a tile change just marks the adjacent ring dirty and the
  neighbors figure themselves out in-shader — no CPU-side derivation. If lookup cost ever
  bites: hold all 16 orientations as their own defs for a direct frame (the escape hatch;
  "I'm just being frugal").

## Why this is right (assessment, 2026-07-29)

- **It fills fields the format explicitly reserved for this.** The def R comment reads
  "layer/rotation/inherit_rotation/type are AUTHORED fields the resolver has no source for
  yet — 0 until the DSL supplies them". This stream IS the DSL supplying them.
- **It deletes the failure class we just debugged.** The CPU cell-mirror (`tileKindAt`,
  re-cell queues) and the manifest race exist because cell selection lived client-CPU-side
  off async manifest data. Under this design the linked geometry is CONTENT (boot-
  available, authoritative) and adjacency is derived from the data texture the GPU already
  owns — self-healing by dirty-marking, nothing to desync.
- **It dissolves the def conflicts found in tile-lighting**: per-(cell,lod) defs wanted the
  offset lanes for both placement AND sampling, and broke the normal-quadrant stride
  (2^lod = cell side vs co-pack stride = atlas side). With the frame = the whole atlas and
  the cell chosen at read time, both conflicts disappear — and 16× fewer defs, no def
  swaps when neighbors change.
- **It sets up the future free**: rotation 2 (world-hashed ground variety), def-level
  `cast_shadows` (wall shadows later = author one bool), and cross-type reuse of the mode
  pattern.

## Known risks (planned, not discovered later)

- **R1 · The presence slot-0 contract change.** The dense-bucket EARLY-OUT currently
  assumes slots fill from 0 ("empty slot 0 ⇒ empty tile") — it must flip to slot 1, and
  caster capacity per tile drops 8 → 7. EVERY presence reader special-cases slot 0: the
  corridor/brute walks, `receiverAt`, the receiver bakes, presence build. Enumerating and
  converting those sites is the bulk of the work.
- **R2 · Two id namespaces in one table**: slot 0 = a DEFINITION id; slots 1..7 =
  `billboard_data` ids. The special-case must be airtight or a def id dereferences as a
  billboard.
- **R3 · Receiver semantics must not flip the ground-shadow cut.** If every tile texel
  classifies "on-billboard", the existing cut (`rbillboardN != 0 → ground shadow = 0`)
  erases ALL ground shadows. Tile receivers are "ground WITH a normal": N·L from the
  tile's def, ground shadows KEPT; the cut keys on standing receivers (slots ≥ 1) only.
- **R4 · The connect rule.** Rotation-1 matching connects ACROSS kinds (a smooth wall
  joins a brick wall — structurally right, art mismatches at the junction). The user's
  stated rule is rotation-match; def-id match (same kind only) is the alternative if
  junctions read wrong. Fork to confirm at the first two-material drill.
- **R5 · `internal_padding` vs the current masters**: the served manifest reports
  `pad [0,0]` for today's wall masters. The DSL value must match the actual art — author
  what the master truly has (verify at execution; re-master if padding is wanted).
- **R6 · The DRAW side stays CPU-selected for now.** The albedo bake picks the drawn cell
  via the CPU D1 helper (build-walls P1); lighting picks it in-shader. Two selectors, ONE
  shared formula, same inputs — acceptable v1, with def-driven drawing as the documented
  destination.

## Carried over from tile-lighting

The BLUEPRINT placement preview lights via the hot path — resolved there as F3 (the
overlay samples the live cold+hot lightmaps + ambient with the blit's formula; ephemeral
by construction). Lands here as its own phase.
