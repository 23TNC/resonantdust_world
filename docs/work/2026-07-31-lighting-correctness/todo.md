# Todo — lighting correctness

_Pin before fixing (the shadow-polish discipline); every phase ends renderable and LOOKED AT;
no MRT anywhere; every cap counts its drops; perf claims only from the corrected harness.
Stances + fixtures: [`README`](README.md)._

---

## P0 — pin the named defects

- [ ] Capture the lit fixture (zoom 1 + 2) and, for EACH of bbox / silhouette / z-order,
      record a concrete on-screen reproduction with a probe naming wrong-vs-expected in
      `issues.md`. Acceptance: each named defect has a pinned cause (or a reasoned
      not-reproducible note).
- [ ] Build the def→bbox truth table: dump `definition_data.subframe` vs the art's actual
      opaque extent for a conifer, the wolf (e + n), the human body + head (scaled), a west
      (flipped) draw, and a linked wall cell. Acceptance: the (stored, actual) table in
      `issues.md` with every mismatch named.

## P1 — reach in the data

- [ ] Split `prim_data.B`'s `u10 intensity` into `u4 reach` (bias +1 → 1..16 tiles) +
      `u6 intensity`; update both record writers, the lane asserts, and `VARIABLES.md`.
      Acceptance: asserts pass; VARIABLES shows the new B layout; a probe reads back an
      authored reach.
- [ ] Plumb authored reach from content (`&thing.light.reach`, torch = 8) through the DSL
      bundle into the emit prim's reach lane. Acceptance: the torch registers on exactly
      the tiles its authored reach covers (occupancy overlay), independent of intensity.
- [ ] Retire `reachFromIntensity` — the tile light-set build and the walk bound read the
      STORED reach; delete `lightReach.ts`. Acceptance: grep-clean; the 16-light fixture
      renders identically when authored reach matches the previously derived values.

## P2 — the bbox

- [ ] Fix P0's mismatches per cause: measure each rotation frame's `subframe` from surface
      presence at ingest (POST pre-atlas scale), mirror for flipped draws, assert
      `subframe ⊆ frame` at write. Acceptance: the P0 truth table re-probed with 0
      mismatches.
- [ ] Verify live: a debug overlay draws each record's bbox over its sprite for the
      conifer, wolf (e + n), human body + head. Acceptance: captures show the box tight on
      the drawn art within 1 unit, all cases.

## P3 — silhouettes

- [ ] Prove the refine's crossing→caster-frame mapping against a brute-force per-pixel
      reference on one caster (the conifer by the torch). Acceptance: 0 differing pixels
      refine-vs-reference at the lighting resolution.
- [ ] Implement `cast_type 2` — the n/s perpendicular caster card via `base + rotation`
      def addressing (no special case). Acceptance: an n/s-facing wolf and human cast the
      SIDE silhouette with the head end tracking the facing (shadow-polish D3), on screen.
- [ ] Implement shadows ONTO billboards: receivers past ground, resolved from the stored
      `(caster, receiver)` pairs + the presence walk. Acceptance: the climbing shadow —
      a tree's shadow lands on a wall/billboard behind it, reproduced at the fixture.
- [ ] A/B the silhouette edge against the strip's before-images at both zooms.
      Acceptance: captures beside `before/04-shadow-edge-zoom2.jpg` in `completed.md`;
      edges silhouette-exact, no wedges.

## P4 — z-ordering

- [ ] Write the ONE z contract: prim `layer` derivation = the draw's zIndex derivation;
      `presence` sorts by it; eviction keeps topmost; assert at write; document in
      `VARIABLES.md`. Acceptance: the contract text + a writer assert that a violating
      layer throws in dev.
- [ ] Build the lit-surface probe: a debug view colouring each pixel by its RESOLVED
      receiver, diffed against the drawn surface on a stacked scene (tile / tree / human
      body+head). Acceptance: the probe exists and reports a mismatch count.
- [ ] Fix what the probe shows (resolution order, eviction, layer values) per cause.
      Acceptance: 0 mismatching pixels at zoom 1 + 2 with the wolf walking through the
      stack.

## P5 — normals + ambient

- [ ] Per-light N·L in the slot pass: sample the RESOLVED receiver's normal quadrant,
      direction from the light prim to the texel (the FINE-lightmap model — the summed map
      inherits it, display stays one fetch). Acceptance: a side-lit conifer/wall shades
      directionally; A/B captures against flat.
- [ ] Restore ambient × AO from `surface.G` in the display composite. Acceptance: crevice
      darkening on the conifer matches the strip's before-image character; unlit areas
      unchanged.
- [ ] Movers under N·L: the normal frame follows facing (def swap), and slot updates on
      movement stay differential-exact. Acceptance: the fixture human's shading changes
      across facings; add/remove returns the summed map bit-identically with N·L on.

## P6 — the verdict

- [ ] Re-measure the full chain — MOVING lights, reach 16, zoom 1, corrected harness —
      at N = 1/4/8/16 with everything above ON. Acceptance: the table in `completed.md`;
      inside 8 ms, or the regression named with its cost attributed.
- [ ] Write the determination: does the new method outperform the old, and WHY — the
      structural argument (stored identity, differential slots, flat refs, fixed-grid
      scaling) against the honest costs (+12 MiB, incommensurable old absolutes per
      rework I10). Acceptance: a verdict section in `completed.md` answerable to the user.
- [ ] Re-score the capability checklist (rework I6) and update `VARIABLES.md` to the
      shipped layouts. Acceptance: items 1–6 + 10 all "yes" or honestly excepted; 7–9
      recorded as awaiting the user's call; docs-check green.
