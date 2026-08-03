# Todo — LOD aftermath

_Fix forward. Each bug: reproduce → name the cause in `issues.md` → fix → verify on
screen. Stances: [`README`](README.md)._

---

## P0 — flora's colour returns (the widest visible break)

- [x] Coordinate with `render-performance` FIRST: read its todo/completed state; if its
      per-map re-pack fix landed, verify flora directly and skip the next item.
      Acceptance: the finding (landed / in-flight / abandoned) recorded in `issues.md`
      with the commit or its absence. → [I1](issues.md#i1): I11 landed (`2c908012`),
      complete-on-arrival OPEN + idle ~5 h; custody taken, note written into their
      issues (their I12).
- [x] If unlanded: make the co-pack COMPLETE-ON-ARRIVAL — a stem re-packs (or
      re-blits the quadrant) when a late map's bytes land, and a pack missing
      albedo+surface never publishes. Acceptance: flora renders its reconstructed
      colours on a cold cache-less load; the layers-quadrant row probe reads real
      weights on the graphics twin. → bounded TIME-DRIVEN retry (resolve-side trigger
      parks once bakes drain — learned in the first drill and rebuilt).
- [x] Regression net: a boot-time (debug) audit that counts packed stems whose LISTED
      maps have empty quadrants, surfaced in the pool counter. Acceptance: fixture
      boots at 0; deleting a map file on disk makes it count 1 (and the world still
      renders). → bookkeeping-side (`partial` in poolStats + HUD row); the disk-delete
      form couldn't 404 (the edge serves its derived cache) so the drill used an
      in-page 404 window: partial 1 during, healed to 0 after.

## P1 — the lighting darkness ("clipped" canopies)

- [x] Reproduce at zoom 2 and isolate the term: toggle shadows-only vs N·L-only vs
      wrap-floor values on the dark canopy bands (three A/B captures). Acceptance: the
      dominant term named in `issues.md` with captures. → `__shadows(false)` removed
      EVERY dark band: 100% the occlusion term (zero-contribution shadows). New
      toggles `__shadows`/`__ndotl` stay as drills.
- [x] Fix the named term (candidates, judged by the isolation: billboard shadow
      attenuation, the normal pitch on canopy texels, the wrap floor at high zoom).
      Acceptance: no pitch-black canopy at any zoom; trees read whole lit AND unlit;
      `__lightexact` bit-identical + `__gather` occupancy nonzero. → SHADOW_KEEP 0.35:
      a shadow DIMS its light, never deletes it (tunable constant).

## P2 — z-order + depth layering (the ONE-KEY model, F2)

- [x] After P0: re-capture the unlit overdraw case. If it vanished with flora's colour,
      close as reduced-to-P0; if not, root-cause the bake paint order vs the zdepth
      painter's key for the failing pair. Acceptance: the verdict + capture in
      `issues.md`; if real, the fix verified by `__zprobe` agreeing with the drawn
      order. → CLOSED reduced-to-P0: textured flora renders correctly BEHIND canopies
      (unlit close-up); the black fill had destroyed the depth cues — the paint order
      was right all along.
- [x] ONE base-row derivation ([F2](forks.md#f2)): a shared records/GLSL definition of
      "ground-contact row" consumed by the bake sort, the zdepth lane, the presence
      sort, and the refine — replacing their independent derivations. Acceptance:
      grep shows the four call sites reading the one definition; typecheck; render
      unchanged (`__zprobe` + captures). → `groundContactY/Row` exported from
      SquareCache; recordSync + the zdepth lane consume it; `thingOrder`/slot order
      bound by the contract note (they encode the row at creation, pre-Primitive).
- [x] The shadow-layering compare (user: "shadows in the back are not drawn over
      shadows in the front"): the refine skips a caster whose ground-contact row is
      NORTH of the receiver's — its shadow lands on the receiver's back face, never
      drawn. One subtract-compare on fetched words. Acceptance: a staged
      caster-behind-receiver scene shows no front-face shadow; `__lightexact`
      bit-identical; `__gather` occupancy sane. → landed WITH the P1 fix; the A/B
      captures show rear-caster canopy shadows gone; occupancy 355, exact ✓.
- [x] Presence on EVERY physically-occupied tile (user; extends the x-span work): a
      prim registers in each tile its FOOTPRINT covers, and the receiver/gather
      dilation loops shrink accordingly (they exist to paper over under-registration).
      Acceptance: `__zprobe` finds the prim from any occupied tile; `droppedReceivers`
      stays 0 at the fixture; the gather's per-step tile fetches REDUCE (count logged
      before/after). → full-box registration (1050 presence tiles at the fixture);
      dilation 2→0 in BOTH passes = 15→1 fetches/walk-step, 3→1/receiver-texel;
      walk = brute at dilation 0; dropped 0.

- [x] REOPENED by the user 2026-08-03: trees clip at the edges. Root-cause the
      bbox/atlas path. Acceptance: cause named in `issues.md`; trees render whole and
      VARY by variant on screen. → [I3](issues.md#i3): variants rode the CELL but are
      STEMS — ingest cropped every conifer by variant 0's sapling rect. Fixed by
      stem-routing (`thingTexture` + per-variant-stem subframes + manifest-race
      repaint); verified live — 455 prims across 22 variant stems, 9 per-stem rects,
      whole varied canopies, partial 0.

## P3 — panning at the zoom extremes

- [ ] The FOREGROUND pan drill (user watching, or the tab granted focus): full-screen
      pans at zoom 2 and 0.25, watching for stale/black slots, seams, bake storms,
      lighting lag. Acceptance: each observed artifact named in `issues.md` — or the
      item closed as not-reproducible with the session noted.
- [ ] Fix what the drill names (unplannable until named — this item is the budget for
      it). Acceptance: the re-drill passes clean at both extremes.

## P4 — the verdict

- [ ] Cold load + the full drill set: flora coloured, canopies lit sanely, pan clean at
      extremes, `__framecost` within the render-performance baseline, `__lightexact` +
      `__gather` green. Acceptance: captures + numbers in `completed.md`; **the user's
      eyes close the stream**.
