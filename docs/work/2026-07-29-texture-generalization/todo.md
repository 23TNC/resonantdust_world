# Todo — texture-generalization

_Items tick in place; the box is the move. Design + risks R1–R6: [`README`](README.md).
Visual items verify at ≥2 zooms, tab VISIBLE + FOREGROUND (background tabs freeze rAF and
void every measurement — tile-lighting's recorded lesson)._

---

## P0 · Docs + DSL lanes (VARIABLES first)

- [ ] VARIABLES.md: definition R bit 24 → `cast_shadows`; rotation documented as a
      PER-TYPE MODE (tiles: 0 plain / 1 autotile / 2 world / 3 reserved); G's reserved
      lane gains `internal_padding` (units); `prim_presence` slot 0 documented as the
      tile's DEFINITION id (1-based; 0 = no tile; casters occupy 1..7). Acceptance:
      docs-check green before code.
- [ ] DSL: linked lanes on tile kinds — atlas tiles w/h (4×4 for walls), internal_padding,
      cast_shadow (false on ALL tiles incl. walls); confirm every base tile is authored.
      Acceptance: corpus republishes; the bundle exposes the lanes (console probe).
- [ ] Verify internal_padding against the ACTUAL wall master (R5 — the manifest says pad
      [0,0] today); author the true value. Acceptance: the sampled window is
      demonstrably art-correct in the P4 drill (no cross-cell bleed / no cropped edges).

## P1 · Defs from the DSL

- [ ] Linked defs mint from CONTENT, one per stem (not per cell): frame = the WHOLE atlas
      at its lod, span = grid, W/H = the tile window minus padding, offset = padding,
      type = biome-tile, rotation = the mode, cast_shadows from the lane. The
      per-(cell,lod) tile-def path retires. Acceptance: a wall def probe shows the
      authored geometry; def count for walls = 1 per lod.

## P2 · Presence slot 0

- [ ] The writer: every window tile's slot 0 = its kind's definition id (from the dense
      rows + overrides); slots 1..7 = casters; the dense-bucket early-out flips to
      slot 1 (R1). Acceptance: a probe reads slot 0 = the right def over ground, walls,
      and empty edge (0).
- [ ] The readers: corridor + brute walks, `receiverAt`, and the receiver bakes
      special-case slot 0 (a DEF id — R2): consult `cast_shadows` (skip non-casters —
      every tile today), never dereference prim_data. Acceptance: brute↔corridor
      identity holds; ground shadows bit-identical to pre-stream on a wall-free scene.

## P3 · Autotile in-shader (rotation 1)

- [ ] The lighting-side cell pick: a rotation-1 def samples its cell from the 4
      neighbors' slot-0 defs (rotation-1 match = connected — R4 fork on first
      two-material junction), through the D1 formula; a tile change dirties the
      adjacent ring and neighbors self-heal cold-side. Acceptance: the wall ring's
      lighting-side sampling (normals/silhouette when casting arrives) uses the same
      cells the drawn art shows (R6 — one formula, two consumers, compared in a probe).

## P4 · Tiles lit (the original goal, on the new rails)

- [ ] Tile receivers: rotation-authored tiles get N·L from their def's normal at their
      texels WITHOUT flipping the ground-shadow cut (R3 — the cut keys on standing
      receivers only). Acceptance: a torch-lit wall shades directionally at ≥2 zooms;
      plain ground + tree shadows BIT-match pre-stream captures; cold bakes only on
      tile change (counter drill, foreground tab).

## P5 · The blueprint (carried from tile-lighting F3)

- [ ] `BlueprintOverlay` samples the live cold+hot lightmaps + ambient with the blit's
      formula — the preview lights/darkens with the world, zero records, zero residue.
      Acceptance: dragging near a torch shows a lit blueprint; release leaves hot+cold
      untouched (counter probe).

## P6 · The drill

- [ ] End-to-end: build beside a torch — lit blueprint while dragging, walls lit on the
      new rails after, ground shadows intact everywhere else, reload-stable, 120 fps,
      cold quiet after the build burst. Acceptance: numbers + captures in
      `completed.md`; the user's eyes are the final oracle.
