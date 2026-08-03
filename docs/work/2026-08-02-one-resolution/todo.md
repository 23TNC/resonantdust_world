# Todo — one resolution

_The atlas LOD ladder dies; the slot-grid level is renamed; mipmaps do minification.
Stances: [`README`](README.md)._

---

## P0 — pin the two concepts apart, and the packing law

- [x] Inventory every consumer of BOTH "lod"s into `issues.md`: the atlas ladder
      (resolver/LodPool/def-swap/IndexedDB keys) vs the slot-grid level
      (squareMath/SquareCache/lighting `win.lod`). Acceptance: the table names each
      file + which concept it touches; the deepest-zoom BEFORE captures taken.
- [x] AUDIT the live atlas for pow2 conformance (user: a concurrent stream apparently
      broke away from pow2 packing; the pool only WARNS): dump every packed frame's
      dims + grid stems' masters at runtime; name each violator and the stream that
      introduced it. Acceptance: the violator table in `issues.md` (empty is a valid
      finding).
- [x] Settle the packing convention as [F3](forks.md#f3) — pros/cons of pow2 vs
      free-size recorded, ONE law chosen, the user's sign-off noted. Acceptance: F3
      states the law and every P1-P3 item below builds to it.
- [x] Enforce the law at ingest: the pool's pow2-square warn becomes a REJECT (the
      frame does not pack; loud console error), plus a boot-time conformance line in
      the pool counter. Acceptance: a deliberately non-conforming test frame is
      refused; the fixture boots clean.
- [x] Repair whatever the audit found: re-ingest (or re-master) each violator to the
      law. Acceptance: the audit re-run reports zero violators; the affected stems
      render unchanged on screen.

## P1 — one-resolution resolver

- [x] Collapse the resolver to one co-pack per stem at the maximum size: delete
      `LOD_SIZES`/`pickLodForSize`/`targetPx`/`setTargetLod`/`FLOOR_LOD` preview and
      the below/above chooser; `packed` becomes `Map<stem, frame>`. Acceptance:
      typecheck; cold load renders textured at zoom 1.
- [x] Collapse the IndexedDB cache keys to (stem, map) at the one size; stale
      per-size rows purged or ignored-by-miss. Acceptance: a cold reload packs from
      cache (no refetch of unchanged stems — network tab).
- [x] Simplify `LodPool` to the single-size sprite pool (rename with it) and delete
      the per-size page duplication; `lodStats` becomes a plain pool counter.
      Acceptance: resident texture bytes at the fixture ≤ the P0 baseline; counter
      recorded.

## P2 — the records stop swapping

- [x] One def block per stem (rotation-indexed as today), never re-pointed by zoom:
      delete the CPU def-id swap and let the page registry hold still across the whole
      zoom range. Acceptance: `__buildrecords` def count constant across a
      2 → 0.25 sweep; `__lightexact` bit-identical.
- [x] Delete `setTargetLod` plumbing from the Viewport/zoom path (the resolver no
      longer has a target to set). Acceptance: grep-clean; zoom drills unchanged.

## P3 — mipmaps carry minification, on a SPLIT atlas

- [x] Split every pool page into TWIN textures per [F4](forks.md#f4) (user): ONE packer
      rect, GRAPHICS page holds albedo+normal quadrants, DATA page holds
      surface+layers; `resolve()` hands each map its page; frame coords identical.
      Acceptance: typecheck; cold load textured; a surface texel readback is
      byte-identical to the master (unfiddled). → layers moved to GRAPHICS by the
      user's follow-up (linear-in-weights proof in F4); surface alone is DATA.
- [x] Mips + filtered sampling on the GRAPHICS page ONLY (mip chain capped at level 2 —
      the bleed bound; DATA stays NEAREST/no-mips). Acceptance: zoom 0.25 A/B against
      the P0 captures — equal or better; no shimmer on a pan.
- [x] Verify the silhouette/records side reads the DATA page exactly as before:
      `silhouetteHit`, receiver coverage, `__zprobe` at two zooms; shadows unchanged
      on screen. Acceptance: `__lightexact` bit-identical + captures.

## P4 — rename the survivor

- [x] Rename the slot-grid "lod" to PARTITION LEVEL ([F1](forks.md#f1)) across
      squareMath (`lodForZoom` → `partitionForZoom`, `LOD_MAX` → `PARTITION_MAX`),
      SquareCache, and the lighting `win.lod` plumbing. Acceptance: `grep -wi lod`
      over `client/webgl/src` hits only history-referencing comments; typecheck.
      → 7 residual hits: history notes + the server's `/lod/` route literal (wire
      name, out of scope). `lodUrl`→`texUrl`, `lodStats`→`poolStats`,
      `lod.ts`→`urls.ts`, previewCache helpers renamed.
- [x] Update VARIABLES.md (the freed `frame_lod` u2 returns to reserved; the torus
      paragraph says "partition level") and the affected component docs. Acceptance:
      `bin/rd docs-check` green.

## P6 — the user's verdict (2026-08-02): fix performance or roll back

_User, verbatim: "with the changes to lod came a ton of performance issues and bugs…
First it seems you decided to block on texture load. There is a reason texture loading
was asynchronous, and it wasn't because it was fast. Secondly we should keep the 32px
previews. Third we need to solve panning at max and min zoom levels. Fourth we need to
fix z-order. Fifth trees are clipped."_

- [ ] Restore the 32-px PREVIEW tier as the fast-first tier — exactly TWO tiers
      (preview → max), no ladder, no target tracking: resolve serves the preview
      co-pack the moment it lands and the max swaps in async. Acceptance: a cold
      cache-less load paints textured (preview) world in ≤ ~1 s; masters upgrade in
      place; network shows 32 + max only.
- [ ] Verify NOTHING blocks on texture load (the perceived block = geo-until-max):
      boot/critical paths never await packs; mip regeneration coalesces to once per
      frame instead of per-arrival (burst arrivals = one 2048² mipgen each = stutter).
      Acceptance: no long tasks > 50 ms attributable to packing in a boot profile.
- [ ] Reproduce + fix panning at ZOOM_MAX (2) and ZOOM_MIN (0.25): name the defect
      (stale/black slots, bake storms, lighting cost) in `issues.md`, then fix it.
      Acceptance: a full-screen pan at both extremes stays visually intact.
- [ ] Reproduce + fix the z-order defect. Acceptance: the failing case named in
      `issues.md`; a capture before/after; `__zprobe` agrees with the drawn order.
- [ ] Reproduce + fix the tree clipping. Acceptance: the failing case named in
      `issues.md`; conifers render un-clipped at all zooms; captures.
- [ ] The re-verdict: cold load + pan drill at both extremes, `__gather` occupancy
      non-zero, `__lightexact` bit-identical. Acceptance: captures + numbers in
      `completed.md`; the user's eyes close the phase — or the stream ROLLS BACK.

## P5 — the sweep

- [x] Cold loads at zoom 2 / 1 / 0.5 / 0.25: textured world, pools + shadows intact,
      A/B captures beside P0's, memory counter beside P0's, `__framecost` sanity
      (no regression). Acceptance: captures + numbers in `completed.md`; **the user's
      eyes are the exit criterion**.
