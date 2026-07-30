# Issues — analytic-wall-normals

_Defects found during execution. Known inputs: the smooth diffuse is the seam-fixed one
(comfy-linked-tiling P2) — the P0 fit measures THAT art; the oblique art convention means
the front face is SOUTH-biased (profile sweep must respect the n/s asymmetry — N arms show
the wall's back/top, S arms the front face; E/W are side-symmetric); the corpus normal
encoding is view-space +Y-up (normals.py's convention, verified against Laigter)._

## I1 · Angled internal/external corners (user-reported post-delivery; FIXED 2026-07-29)

The v1 apron had square ends and butt joints. The art's corner crops (T + lone cells)
taught the true model, which INVERTED my first miter guess: the front face SPLAYS —
widening at 45° over the flanking side bands from each body corner (the prism's south
face in oblique; the side bands' visible width shrinks with depth until the face meets
the outline) — while at CONCAVE (T) corners the perpendicular arm's side face claims the
45° wedge nearer the arm. Implemented as splay-GAIN over the bands (per-column body-end
lookup at 45°) + the concave nearest-surface rule; the failed convex-CUT attempt is in
history. Shipped: relief 35.4 %, flat 0.318°, seams 0.00 unchanged; the lone/cross
side-by-sides (`corners-final.png`) match the art's trapezoids in every slant direction,
and the in-game torch drill shows the mitered face ends catching light correctly.

**I1 refinement (user red-outline annotation):** the concave wedge was CAPPED at the
side-face width, so only its bottom sliver showed — the annotation showed the 45°
diagonal running the FULL face height from the junction corner. The cap is removed (the
wedge widens to the apron's foot; its side-face normal holds to full depth instead of
fading at `sd`). Gate unchanged (flat 0.318°, seams 0.00, relief 36.0 %); the cross
side-by-side now matches every red line, and the in-game torch drill shows full-height
mitered face ends. `corners-final.png` updated.
