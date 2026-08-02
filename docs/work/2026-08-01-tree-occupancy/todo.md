# Todo — 2×3 trees + tile occupancy

_Visual 2×3, physical bottom 2×1, spacing by pure per-tile tournament, occupancy as a
derived worker cache. Stances: [`README`](README.md)._

---

## P0 — author the shape

- [ ] Add footprint lanes to the DSL data corpus: `&thing.footprint.w` / `.h` (tiles,
      default 1×1, anchored at the base row) on `tree` in `content/data/things.rd`;
      parser + loader expose them on the kind def. Acceptance: `bin/sim` loader test (or
      a worker log line) prints tree footprint 2×1.
- [ ] Author the visual: conifer `&thing.size 3` (3×3 drawn box, subject letterboxed
      2×3). Acceptance: the client layout table carries size 3 for the tree kind;
      content publishes + hot-swaps.

## P1 — the art

- [ ] Re-master the conifer variants with the subject at 2:3 proportions inside the
      square canvas (bottom-anchored, feet at the canvas base), through `bin/art`.
      Acceptance: `opaqueBBox` on the new master reads fw ≈ 2/3, fh ≈ 1.0 of the frame;
      the drawn tree on screen is 2 tiles wide × 3 tall with feet on its anchor tile.
- [ ] Verify the record side inherited: def subframe ≈ 2×3 tiles, shadow springs from
      the 2-wide base line, presence registers the 2-tile x-span (multi-tile presence
      is already live). Acceptance: `__zprobe` on both base tiles resolves the tree;
      capture of a grounded 2-wide shadow.

## P2 — biome spacing

- [ ] Give the biome DSL a coordinate-addressed roll (`^rand` at NEIGHBOUR coordinates,
      e.g. `^rand.at x y salt`) so a tile can recompute another tile's candidate roll.
      Acceptance: a VM test — `^rand.at` of the current tile equals its own `^rand`.
- [ ] Implement the tree tournament in the forest/taiga rules: candidate iff roll < p;
      place iff the candidate BEATS every candidate in the footprint-conflict window
      (2 wide × 1: dx ∈ [−1, +1] at the base row — lower roll wins, coordinate
      tiebreak). Acceptance: a regenerated area shows NO two trees with overlapping
      footprints; rule committed with the window derivation in a comment.
- [ ] Retune the tree probability for the 2-wide footprint (start ~0.10, judge by eye
      against the current forest read). Acceptance: a regenerated forest capture beside
      a before-capture; the user's density verdict recorded.
- [ ] Regenerate the fixture area (`cb` bump or fresh zones) and verify cross-zone
      spacing: no overlapping footprints across a zone seam. Acceptance: capture of a
      seam region; a probe listing tree anchors near the seam with pairwise distances.

## P3 — occupancy, the derived cache

- [ ] Build the per-zone occupancy bitset in the worker: derived from the zone's things
      × their kinds' footprints on zone load; exposed as `occupied(zone, tile)`.
      Acceptance: a unit test — a synthetic zone with one 2×1-footprint tree at (x,y)
      answers occupied for exactly (x,y) and (x+1,y).
- [ ] Maintain it on every thing mutation (worldgen create; SET/BUILD paths that add or
      replace things), recomputing the touched tiles from the shard rather than
      patching incrementally where ambiguous. Acceptance: a drill — build a wall /
      create a thing in a live zone, re-probe, the bitset reflects it.
- [ ] Add the debug probe: a worker-side dump (log or query) of a zone's occupancy grid.
      Acceptance: the fixture zone's dump matches its visible trees' base tiles,
      recorded in `completed.md`.

## P4 — the world displays correctly

- [ ] The joint drill: cold load at a regenerated forest, both zooms — 2×3 trees,
      grounded 2-wide shadows, sane spacing, lighting/presence intact, wolf walking
      through (pathfinding not yet — it may clip trunks; note it). Acceptance: captures
      in `completed.md`; **the user's eyes are the exit criterion**; the pathfinding
      successor stream is declared with the occupancy query named as its input.
