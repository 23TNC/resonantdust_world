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

## Design deltas (user, 2026-07-29 — second pass, answering R1–R5)

- **D7 · The presence table reshapes to 4 × u32 slots**: `u4 flags | u4 set | u16 index`
  per slot (same uvec4 texel footprint as today's 8 × u16 — zero storage change). The
  `set` field makes a slot SELF-DESCRIBING: it can point at `definition_data` directly
  (the tile case — no per-tile `prim_data`, because prim_data is a CONTAINER of prims and
  a bare tile needs no container) or at `billboard_data` directly — the common
  biome-tile/biome-thing cases skip indirections entirely. Flags: **bit 3 =
  `cast_shadows`** (skip every fetch in the common non-caster case — all tiles v1),
  **bits 1–2 = `receives_shadows`** (mirrored from the def for the same early-out),
  bit 0 spare. 4 slots suffice BECAUSE prim_data contains prims; the fill/walk contract
  (slots from 0, early-out) re-specs against the new shape. (The table gets renamed from
  `billboard_presence` to `prim_presence` — it no longer holds only billboards.)
- **D8 · The ground-shadow special case is REMOVED — everything is a receiver.**
  `definition_data` gains a 2-bit **`receives_shadows`**: 0 = doesn't receive, 1 =
  receives LIKE A BILLBOARD (the elevated/climbing walk), 2 = receives LIKE GROUND (the
  flat walk), 3 reserved. Receivers draw IN ORDER; the on-billboard cut logic retires
  with the special case. Ground tiles author mode 2 — which is exactly what buys tile
  NORMAL MAPS for free, since tiles then walk the same draw paths billboards do.
  Performance impact is UNKNOWN and suspected heavy (user) — re-work is expected;
  the flags early-out is the first lever.
  Bit placement note: def R has ONE free bit (24 → `cast_shadows`); `receives_shadows`
  (2 bits) lives in G's reserved lane alongside `internal_padding` (VARIABLES first).
- **R4 resolved for now**: cross-material junctions (smooth↔brick) are accepted; the
  FUTURE brings up-to-16 VARIANTS per kind (used everywhere), where connection gates by
  a variant/kind check (a wall must not connect to a fence or rock) — deferred, needs
  its own planning.
- **R5 resolved**: two DIFFERENT paddings. The manifest's `pad` is the atlas-EXTERNAL
  padding (correctly 0); `internal_padding` is BETWEEN cells inside the linked atlas —
  and because it exists, no external pad is needed to stop neighbor bleed.

## Known risks (planned, not discovered later; R1–R3 restated under D7/D8)

- **R1′ · The 4-slot caster capacity.** 8 (≤7 effective) caster slots per tile become 3–4
  under D7 (the tile takes one). Dense clusters (forest tiles where several tight-bbox +
  corridor rows overlap) may exceed it — dropped casters = silently missing shadows.
  MEASURE FIRST: a P1 occupancy probe records the live high-water mark per tile before
  the reshape ships; the escape hatch is 8 × u32 (two texels/tile — presence doubles).
- **R2′ · One table, self-describing slots.** The `set` field replaces the id-namespace
  special case — but every reader (corridor/brute walks, `receiverAt`, receiver bakes,
  the bucket fill + dense early-out) converts to the new slot encoding in ONE cut.
  Bit-identical shadows on a billboards-only scene is the conversion's oracle (the
  re-encode changes no geometry).
- **R3′ · The receiver unification has NO bit-identity oracle.** Removing the ground
  special case + the cut changes every shadow edge BY DESIGN. Acceptance is comparative
  captures (before/after on a wall-free scene must read equivalent, not identical) +
  the perf drill the user pre-authorized re-work for.
- **R6 · The DRAW side stays CPU-selected for now.** The albedo bake picks the drawn cell
  via the CPU D1 helper (build-walls P1); lighting picks it in-shader. Two selectors, ONE
  shared formula, same inputs — acceptable v1, with def-driven drawing as the documented
  destination.

## Carried over from tile-lighting

The BLUEPRINT placement preview lights via the hot path — resolved there as F3 (the
overlay samples the live cold+hot lightmaps + ambient with the blit's formula; ephemeral
by construction). Lands here as its own phase.
