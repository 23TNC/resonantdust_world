# Todo — LOD aftermath

_Fix forward. Each bug: reproduce → name the cause in `issues.md` → fix → verify on
screen. Stances: [`README`](README.md)._

---

## P0 — flora's colour returns (the widest visible break)

- [ ] Coordinate with `render-performance` FIRST: read its todo/completed state; if its
      per-map re-pack fix landed, verify flora directly and skip the next item.
      Acceptance: the finding (landed / in-flight / abandoned) recorded in `issues.md`
      with the commit or its absence.
- [ ] If unlanded: make the co-pack COMPLETE-ON-ARRIVAL — a stem re-packs (or
      re-blits the quadrant) when a late map's bytes land, and a pack missing
      albedo+surface never publishes. Acceptance: flora renders its reconstructed
      colours on a cold cache-less load; the layers-quadrant row probe reads real
      weights on the graphics twin.
- [ ] Regression net: a boot-time (debug) audit that counts packed stems whose LISTED
      maps have empty quadrants, surfaced in the pool counter. Acceptance: fixture
      boots at 0; deleting a map file on disk makes it count 1 (and the world still
      renders).

## P1 — the lighting darkness ("clipped" canopies)

- [ ] Reproduce at zoom 2 and isolate the term: toggle shadows-only vs N·L-only vs
      wrap-floor values on the dark canopy bands (three A/B captures). Acceptance: the
      dominant term named in `issues.md` with captures.
- [ ] Fix the named term (candidates, judged by the isolation: billboard shadow
      attenuation, the normal pitch on canopy texels, the wrap floor at high zoom).
      Acceptance: no pitch-black canopy at any zoom; trees read whole lit AND unlit;
      `__lightexact` bit-identical + `__gather` occupancy nonzero.

## P2 — z-order, re-judged

- [ ] After P0: re-capture the unlit overdraw case. If it vanished with flora's colour,
      close as reduced-to-P0; if not, root-cause the bake paint order vs the zdepth
      painter's key for the failing pair. Acceptance: the verdict + capture in
      `issues.md`; if real, the fix verified by `__zprobe` agreeing with the drawn
      order.

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
