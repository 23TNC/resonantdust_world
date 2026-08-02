# Todo — one resolution

_The atlas LOD ladder dies; the slot-grid level is renamed; mipmaps do minification.
Stances: [`README`](README.md)._

---

## P0 — pin the two concepts apart

- [ ] Inventory every consumer of BOTH "lod"s into `issues.md`: the atlas ladder
      (resolver/LodPool/def-swap/IndexedDB keys) vs the slot-grid level
      (squareMath/SquareCache/lighting `win.lod`). Acceptance: the table names each
      file + which concept it touches; the deepest-zoom BEFORE captures taken.

## P1 — one-resolution resolver

- [ ] Collapse the resolver to one co-pack per stem at the maximum size: delete
      `LOD_SIZES`/`pickLodForSize`/`targetPx`/`setTargetLod`/`FLOOR_LOD` preview and
      the below/above chooser; `packed` becomes `Map<stem, frame>`. Acceptance:
      typecheck; cold load renders textured at zoom 1.
- [ ] Collapse the IndexedDB cache keys to (stem, map) at the one size; stale
      per-size rows purged or ignored-by-miss. Acceptance: a cold reload packs from
      cache (no refetch of unchanged stems — network tab).
- [ ] Simplify `LodPool` to the single-size sprite pool (rename with it) and delete
      the per-size page duplication; `lodStats` becomes a plain pool counter.
      Acceptance: resident texture bytes at the fixture ≤ the P0 baseline; counter
      recorded.

## P2 — the records stop swapping

- [ ] One def block per stem (rotation-indexed as today), never re-pointed by zoom:
      delete the CPU def-id swap and let the page registry hold still across the whole
      zoom range. Acceptance: `__buildrecords` def count constant across a
      2 → 0.25 sweep; `__lightexact` bit-identical.
- [ ] Delete `setTargetLod` plumbing from the Viewport/zoom path (the resolver no
      longer has a target to set). Acceptance: grep-clean; zoom drills unchanged.

## P3 — mipmaps carry minification

- [ ] Give the sprite pool's pages mip chains + trilinear sampling for the BAKE path
      (the composites/display keep their settled filtering). Acceptance: zoom 0.25
      A/B against the P0 captures — equal or better; no shimmer on a pan.
- [ ] Verify the silhouette/records side is resolution-independent: `texelFetch`
      consumers (silhouetteHit, receiver coverage) read the one size via the SEED
      ppu lane exactly as before. Acceptance: `__lightexact` + a `__zprobe` at two
      zooms; shadows unchanged on screen.

## P4 — rename the survivor

- [ ] Rename the slot-grid "lod" to PARTITION LEVEL ([F1](forks.md#f1)) across
      squareMath (`lodForZoom` → `partitionForZoom`, `LOD_MAX` → `PARTITION_MAX`),
      SquareCache, and the lighting `win.lod` plumbing. Acceptance: `grep -wi lod`
      over `client/webgl/src` hits only history-referencing comments; typecheck.
- [ ] Update VARIABLES.md (the freed `frame_lod` u2 returns to reserved; the torus
      paragraph says "partition level") and the affected component docs. Acceptance:
      `bin/rd docs-check` green.

## P5 — the sweep

- [ ] Cold loads at zoom 2 / 1 / 0.5 / 0.25: textured world, pools + shadows intact,
      A/B captures beside P0's, memory counter beside P0's, `__framecost` sanity
      (no regression). Acceptance: captures + numbers in `completed.md`; **the user's
      eyes are the exit criterion**.
