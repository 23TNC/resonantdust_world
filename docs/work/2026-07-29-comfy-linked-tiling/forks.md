# Forks — comfy-linked-tiling

_Decisions resolved in-stream. F1 (frozen canonical edges) / F2 (bin/lib placement) /
F3 (diffuse-only inpaint) pre-resolved in the README. P3's stamping-strength decision
lands here with its numbers._

## F4 · Stamping strength on the fixed source (P3) — KEPT at full donor stamping

Measured align-only (no stamping) on the inpainted diffuse: arms E 2.76° / W 3.88°
(above the ~2° bar), normal seams E|W 5.42/11.28. The source fix cleaned the ALBEDO
seams but whole-atlas Marigold still frame-drifts E/W pieces — stamping remains
load-bearing. Rejected: weakened (blended) stamping — it would re-open seam spread for
no measured character gain; revisit only if the user's eyes flag arm sameness.
