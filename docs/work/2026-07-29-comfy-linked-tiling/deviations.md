# Deviations — comfy-linked-tiling

_Logged AT THE MOMENT of deviating, with why._

_None._

## 2026-07-29 · P3 relief gate: shipped 18.3 % against the item's "≥ 20 %"

Isolated (same-input A/B): ~4 pts of the gap vs the 23.4 % reference is RUN-TO-RUN
pipeline variance (my whole-engine on the IDENTICAL old diffuse reads 19.5 %); ~1.2 pts
is the inpainted source being honestly smoother at the ex-seams (the old bands' hard
seams read as geometry). The 20 % guidance was calibrated against the seam-artifacted
reference, so the number is accepted with the decisive check moved to P4's in-game drill
(bevels/faces must present as walls). Also fixed en route: the whole-atlas engine now
FLOATS the atlas (normals.py's own margin rule) — without it relief collapses to 0 %,
and processing resolution must stay at the model's trained 768 (512 → 0.3 %, 1024 → 0 %).
