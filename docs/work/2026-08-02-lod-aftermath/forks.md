# Forks — LOD aftermath

## F2 — ONE depth key: the ground-contact row (screen.z ≡ unit.y) {#f2}

Settled with the user over two diagrams (2026-08-02). With an absolute common
reference, a base point's depth is `screen.z = tan(world_angle) · unit.y` — a positive
constant times `unit.y` — so **screen.z ordering and unit.y ordering are the same
ordering**: the base-row painter has been the screen.z sort all along. The height term
`tan(world_angle) · unit.z` is NOT a sort key: a billboard is a rigid vertical card,
every pixel inherits its BASE's depth; that term is the depth gap between a sprite
pixel and the ground visible at the same screen pixel — which the zdepth lane's
baseRow compare already encodes discretely.

**The key**: a surface's depth is the `unit.y` of its GROUND CONTACT — a billboard's
base row, a tile's own row. No new dimension, no zdepth re-quantization.

**The discipline**: that key now has FOUR consumers — bake paint order, the zdepth
lane, the presence sort, and (new, P2) the shadow-layering compare. All four must read
ONE derivation of "base row" (a shared records/GLSL definition, like the falloff and
occlusion blocks) or the drawn and lit orders drift — the subframe lesson, again.

## F1 — forward, not rollback {#f1}

The user, after the P6 diagnosis: "moving forward with fixes and mitigations instead
of rolling back". Evidence basis: the pre-stream in-place A/B FROZE the renderer under
tick storms HEAD survives (the invalidateAll-per-arrival stampede, fixed by
render-performance I11); rollback also resurrects duplicate atlas tiers and discards
the restored preview kick. Every new-world cost is a named, bounded bug; the old-world
cost was structural. Recorded so a future bad week does not re-litigate it without new
evidence.
