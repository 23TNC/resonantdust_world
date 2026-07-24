# Forks — normal-tilt

## F1 — target normal orientation for a standing sprite (RESOLVED 2026-07-24, user)
**Resolution: a standing billboard is conceptually PERPENDICULAR to the world ground, so its normal lies
IN the ground plane → pitch the eyeball-facing normal 25° DOWN (`= 90° − WORLD_TILT`).**

Flat `(0,0,1)` → `(0, −sin25, cos25) = (0, −0.423, 0.906)`. Rigid rotation about the E–W (screen-x) axis
(`ny' = ny·cos25 − nz·sin25`, `nz' = ny·sin25 + nz·cos25`), NOT the current additive bias.

Dead ends explored (both were my mis-models, corrected by the user):
- *"65° rotation → normal = camera view axis"* — over-pitched; the constraint is "normal in the ground
  plane" (⊥-billboard), which is the **complement** 25°, not 65°.
- *"⊥ ground → normal points straight up → no tilt"* — inverted surface-vs-normal. The **surface** is ⊥
  ground; the **normal** is parallel to the ground (pitched 25° down). Definitely a tilt, not identity.

Current bake (~84°) over-pitches — nearly flat in the ground plane, past the correct 25°.

_Open detail:_ per-facing sign (E/W/S/N) — S faces the viewer (pitch toward viewer), N faces away; E/W
pitch sideways. Confirm the sign table before the full corpus re-bake.
