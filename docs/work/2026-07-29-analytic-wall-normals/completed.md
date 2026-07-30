# Completed — analytic-wall-normals

_Dated entries as items land: what landed + how it was verified._

## 2026-07-29 · P0 + P1 — profile fitted, generator gated (4/4)

**P0**: the cross-section measured from the seam-fixed diffuse — outline 13 px, top body,
2 px ramp, front face 33 px, side faces 16 px (512-px atlas) — written to `geometry.json`
with view-space pitch params (front 65° / side 55° / north 40°, drill-tunable). The
register check (`atlas_geometry.py --check`) reads every band boundary off the art's
luminance steps: ALL offsets ≤ 1 px after the refit (capture: `profile-cell6-bands.png`).

**P1**: the first per-side band model FAILED its own gate and the art corrected it — the
measured truth (junction cells' east edges are IDENTICAL to run edges; T-cells carry the
front face through their flanks) is a FOOTPRINT model: the wall body is a BAND (E/W arms
occupy the northern rows — the south face draws BELOW the body in oblique screen space;
N/S arms the middle columns; hub = their block), faces attach to the footprint's exposed
boundaries via directional distance fields, outline rings the silhouette inheriting the
nearest face. **Gate (checker v2 — `flat_cluster_mean` now SEEDS AT +Z**, because the old
masked-mean seed anchored on the 65° front-face cluster of crisp fields; v2 also re-reads
soft learned fields honestly): flat 0.318° uniform (the u8 quantisation floor — exactly
+Z by construction), **normal seams 0.00/0.00 on all 128 pairs**, N/S arms 0.00; E/W arms
4.7 and hub 9.5 are REAL connectivity-corridor content (a T's south arm corridor differs
from a run — in-world correct; the piece metric's identical-arms assumption only holds
away from the hub corridor). Relief 34.0 % = the true face share, registered by
construction. Side-by-side captures (`cross-art-vs-analytic.png`, `hrun-art-vs-analytic`):
faces land exactly where the art draws them; known v1 simplifications — square face ends
vs the art's mitered trapezoids, butt joints at face/face corners.

## 2026-07-29 · P2 — wired, shipped, drilled (2/2)

`bin/art normal --analytic <kind>` dispatches `.l.` leaves carrying a `geometry.json` to
`atlas_geometry.py`, stamps `normal_engine=analytic` (the recorded-engine honour list
gains `analytic`), errors helpfully when no fitted leaf exists; marigold/laigter paths
untouched. **Shipped** through the command (stamp verified `analytic`; gate re-confirmed
on the master: relief 34.0 %, flat 0.318°, seams 0.00). **In-game at zoom 2 + zoom 1**:
the shading is now GEOMETRIC — the wall run north of the torch lights its south-facing
front band exactly when the light is south of it, side faces catch flanking light, tops
stay correctly dim under horizontal torch light; runs continuous, no seams. The face
pitch params (65/55/40°) are drill-tunable in `geometry.json` if the user wants stronger
or softer face response.
