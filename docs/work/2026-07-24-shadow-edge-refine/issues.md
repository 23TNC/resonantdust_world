# Issues / caveats — shadow-edge refine

_Open questions + known limits. Resolved ones stay with their resolution noted._

---

## I1 · Cast-shadow SHAPE vs shadow-ON-a-prim (receiver clip) — RESOLVED {#i1}
**2026-07-24 — RESOLVED (user): it's the CASTER's shadow (the cast-shape path).** And the broader intent — ALL
the shadow/lighting clips move to the fine (`TEXTILE_SQUARE`) resolution, so the whole image sharpens, not just
this one edge. Below is the distinction, kept for reference:

Two different "blocky shadow" problems, only ONE of which this stream addresses:
- **Cast-shadow shape (THIS stream):** the shadow projected onto the ground/receiver has a blocky edge because
  the CASTER silhouette was sampled coarse. Fixed by re-testing the caster silhouette at fine res in the bake.
- **Shadow landing ON a billboard (receiver clip):** a shadow that climbs a prim is cut to the RECEIVER's
  silhouette by `receiverCover`, which is ALREADY fine-res (it samples the co-packed surface quadrant per fine
  texel). That path is separate + already sharp; if its edge ever looks coarse it's the coarse shadow VALUE, not
  the clip. Confirm which artifact the user's shot shows before building (README framing assumes cast-shape).

## I2 · Overlapping casters share one sharpened edge {#i2}
The refine sharpens the dominant/shadowing slot's edge ([forks.md#f3](forks.md#f3)). Where two casters' shadows
meet at an edge texel, they resolve to one sharpened boundary rather than each caster's own crisp silhouette.
Fine for a dense forest (the union reads correct); flagged so it isn't mistaken for a bug.

## I3 · Cost is bounded by shadow PERIMETER, not area — but is it bounded ENOUGH? {#i3}
The `(0,1)` gate means only penumbra/edge texels re-walk. With a big emitter (soft penumbra) the "edge" band is
WIDE — more texels fall in `(0,1)`, so the refine touches more area. Measure with the actual emitter radius; if
soft shadows make the band too fat, cap the re-walk radius or fall back to (a). Static scenes are dirty-gated so
this is a one-time bake cost per changed tile, not per frame.

## I4 · `walkShadow` scope move must not perturb the gather {#i4}
P0 moves `walkShadow` into `GATHER_COMMON` so `LIGHT_FRAG` can call it. `GATHER_FRAG` must stay bit-identical
(corridor↔brute `debugReadShadow` identity = 0 mismatches). A pure scope move should be inert — verify it.
