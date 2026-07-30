# Forks — analytic-wall-normals

_Decisions resolved in-stream. F1 (shape from connectivity, registered to art) /
F2 (placement) / F3 (height field only-if-free) pre-resolved in the README. P3's
GO/NO-GO for productionising ControlNet lands here._

## GO/NO-GO · ControlNet seeding for irregular wall kinds — **GO, with tuning**

The recipe (recorded: `cn-brick-recipe.json`) is PROVEN mechanically: base image = the
smooth ANALYTIC normal atlas in latent space, conditioning = the target kind's diffuse
through Canny into `controlnet-union-promax`, low-to-mid denoise. Measured on brick:
v1 (dn 0.45 / cn 0.55) inherits the macro almost exactly — flat 0.64°, seams 0.59/0.88 —
but under-details; v2 (dn 0.62 / cn 0.85) grows visible brick-course relief in the faces
at the cost of some drift (flat 2.59°, seams 1.54/4.25). BOTH are ~20–100× more
consistent than brick's current marigold-era normal (flat 1.88° but seams 34.98/90.17 —
rainbow chaos). The productionising follow-on's shape: tune the dn/strength trade per
kind AND compose the marigold stream's align/symmetrize post passes ON the CN output to
claw back seam drift — the two pipelines stack. Brick's masters were NOT touched
(candidates live in the session scratchpad).
