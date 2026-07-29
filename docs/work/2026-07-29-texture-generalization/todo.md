# Todo — texture-generalization

_Items tick in place; the box is the move. Design D1–D8 + risks: [`README`](README.md).
STAGED oracles: P1 must be bit-identical (pure re-encode); P3 is comparative BY DESIGN
(the model changes). Visual items verify at ≥2 zooms, tab VISIBLE + FOREGROUND._

---

## P0 · Docs + DSL lanes (VARIABLES first)

- [ ] VARIABLES.md: `prim_presence` (renamed from `billboard_presence`) = 4 × u32 slots
      `u4 flags (cast_shadows bit 3, receives_shadows bits 1–2) | u4 set | u16 index`;
      definition R bit 24 = `cast_shadows`; G's reserved lane gains `internal_padding`
      (units) + `receives_shadows` (2 bits); rotation documented as the per-type tile
      MODE (0 plain / 1 autotile / 2 world / 3 reserved). Acceptance: docs-check green
      before code.
- [ ] DSL lanes on tile kinds: linked grid (4×4), `internal_padding` (1 for walls — the
      INTERNAL between-cell padding; the manifest's external pad stays 0 and is a
      DIFFERENT value, R5), `cast_shadow` (false on ALL tiles incl. walls),
      `receives_shadows` (ground tiles = 2/like-ground; walls = 1/like-billboard).
      Acceptance: corpus republishes; the bundle exposes the lanes (console probe).

## P1 · The presence reshape (billboards only — the pure re-encode)

- [ ] Occupancy probe FIRST (R1′): instrument the CURRENT bucket fill and record the
      live high-water casters-per-tile over a forest pan + the wall ring. Acceptance:
      the number in `completed.md`; if > 3, raise the 8×u32 escape hatch as a fork
      BEFORE converting.
- [ ] The reshape: 4 × u32 `flags|set|index` slots — fill (buildCasters), the dense
      early-out re-spec, both walks, `receiverAt`, both receiver bakes, presence build —
      billboards ride `set = billboard_data` with their def's cast/receives flags
      mirrored. Acceptance: brute↔corridor identity AND bit-identical shadow output vs
      pre-reshape on a billboards-only scene (`debugReadShadow` compare).

## P2 · Tiles enter presence

- [ ] Linked defs from CONTENT (one per stem: frame = whole atlas, span = grid, W/H =
      the cell window minus internal_padding, offset = the padding inset, top-left
      anchor, type = biome-tile, rotation = mode, cast/receives from the lanes); the
      per-(cell,lod) tile-def path retires. Acceptance: a wall def probe shows the
      authored geometry; one def per lod for the whole atlas.
- [ ] Slot 0 writer: every window tile's slot 0 = `set definition_data + index` its
      kind's def, flags mirrored (all tiles !cast v1); ground rows + overrides feed it.
      Acceptance: probes over ground/wall/edge read the right def + flags; walks skip
      tile slots at zero fetches (flag early-out — counter probe).

## P3 · The receiver unification (D8) + autotile

- [ ] `receives_shadows` modes replace the ground special case: mode-1 receivers walk
      elevated (the climbing path), mode-2 walk flat (ground), mode-0 early-out at the
      presence flag; receivers draw in order; the on-billboard CUT retires with the
      special case. Acceptance (R3′ — comparative, not identical): a wall-free scene's
      captures read equivalent to pre-stream; tree-shadow-climbs-trunk still works;
      the perf drill numbers recorded (re-work pre-authorized if heavy).
- [ ] Autotile in-shader (rotation 1): the cell from the 4 neighbors' slot-0 defs
      (rotation-1 match = connected — the variant/kind gate is FUTURE per R4), through
      the D1 formula; a tile change dirties the adjacent ring and neighbors self-heal.
      Acceptance: lighting-side cells match the drawn art's cells (one formula, two
      consumers, compared in a probe).

## P4 · Tiles lit + the blueprint + the drill

- [ ] Tiles receive with their def's NORMAL through the unified path (the original
      goal). Acceptance: a torch-lit wall shades directionally at ≥2 zooms; plain
      ground + tree shadows read equivalent to pre-stream; cold bakes only on tile
      change (counter drill, foreground tab).
- [ ] `BlueprintOverlay` lights via the live cold+hot lightmaps + ambient (tile-lighting
      F3). Acceptance: a drag near a torch shows a lit blueprint; release leaves the
      accumulators untouched.
- [ ] End-to-end drill: build beside a torch — lit blueprint, walls lit on the new
      rails, shadows intact everywhere else, reload-stable, fps recorded (a regression
      budget is EXPECTED here — the user pre-authorized re-work). Acceptance: numbers +
      captures in `completed.md`; the user's eyes are the final oracle.
