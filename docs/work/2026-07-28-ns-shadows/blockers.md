# Blockers — ns-shadows

_Rows: what blocks, why it needs the user, options, recommendation._

## B1 · The wolf-eyes live drill (P2) — awaiting the user's eyes (wolf art FIXED 2026-07-29)

The invisible wolf's ROOT CAUSE is found and fixed: the art-128 re-export of
`pawn/animal/wolf/1` (2026-07-28 20:42) wrote ALL-ZERO `surface.*.png` maps (B = coverage,
the one silhouette source of truth — zero coverage = nothing drawn; the "thin dark sliver"
was the shadow path's remnant). The diffuse masters were intact, so `bin/art surface
pawn/animal/wolf` regenerated them (coverage now exactly matches diffuse alpha) and the wolf
renders again — verified live on a cold-cache load. REMAINS: the drill itself — the user's
eyes on a walking n/s wolf (shadow sweeping e/w from the center line, flipping sides across
the light's column).
