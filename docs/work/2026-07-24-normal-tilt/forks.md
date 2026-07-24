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

## F2 — ground vs thing orientation, the angle update, and bake-commit (2026-07-24, user)
Three decisions from the lighting-pipeline design (see [`lightmap-resolution`](../2026-07-24-lightmap-resolution/forks.md#f7)):

- **Two orientations, one atlas path.** Standing objects pitch **toward horizontal** (this stream's F1);
  **ground tiles** pitch the OTHER way — a ground tile's card lies IN the ground plane, so its flat normal
  is **world-up `ẑ`** (perpendicular to the ground, not the screen). Both bake through the same atlas; a
  **per-def DSL `orientation`** (`lies-in-ground` vs `stands-perpendicular`) picks which rotation `bin/art`
  applies. **Ground tiles become real prims** carrying a **generic white texture** (white albedo, layer-1
  material, up-normal, full surface) — no geo special-case; forward-compatible with authored ground textures.
- **The angle is now 55°, not 65°.** `WORLD_TILT_DEG` moved 65→55 (Fusion360 model + the honest 3D
  falloff). So the standing-object pitch is **`90 − 55 = 35°`** (F1's "25°" was at 65°). Key the bake to the
  current `WORLD_TILT_DEG`, not a frozen literal, and re-check the numbers at 55°.
- **Baking the pitch bake-COMMITS the world angle — globally.** Since the normal is baked in the world frame
  at the ingest angle and the light **direction** is computed from the data-map tilt, `N·L` is only correct
  if both share one angle. So the data-map tilt is locked to the ingest angle; `__tilt` live desyncs; changing
  the angle = **re-ingest the corpus**. Accepted (fixed-angle camera, settled art direction). This resolves
  the earlier "pitch in-shader so normals follow `__tilt`" lean — the only normal consumer is `N·L` (wants
  world-frame), so baking wins.
