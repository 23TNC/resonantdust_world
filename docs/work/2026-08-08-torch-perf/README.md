# Torch perf — N moving hot lights, measured — 2026-08-08

**What** (user, 2026-08-08): now that a trait emits light, a SMALL PERFORMANCE
TEST. Author a `debug_torch` PAWN in TOML — it emits light and moves, so it
carries those two traits. REVERT the human's drill light bind. Add a new npc
module that spawns a number of debug_torch pawns and moves them around. Reset
the scene and check performance for **8, 16, and 24** debug torch pawns moving
around at **zoom level 1**.

**Why**: a mover's light is HOT — re-baked every frame at a moving anchor
(trait-lights I8 named the cost; F8 measured one pawn at 0.01 ms). This stream
buys the CURVE: how the per-frame lighting cost scales with the count of
moving emitters, on the real pipeline, before anyone designs mitigations
against guesses.

**Design stance**:
- `debug_torch` is a minimal pawn ([F1](forks.md#f1)): `walks` (movement — the
  `can_move_ground` gate reads the derived ground_speed) + a constant
  `emit_light` level 1 (the REAL warm torch tuple — the test stresses the
  shipped light, not a softened one), a flat tint placeholder part, NO needs
  (nothing decays; the brain manages nothing but trips).
- The npc module is a `torches` GROUP brain ([F2](forks.md#f2)) — the bunnies'
  shape verbatim: SPAWN_REQUEST mints with the pathable politeness + the
  refusal-retry posture, adopt from the fan, then perpetual wander (a new
  `move_to` trip on every arrival). Count rides `NPC_TORCHES`, home
  `NPC_HOME`.
- The measurement protocol is FIXED before running ([F3](forks.md#f3)): scene
  reset = pawn-module wipe with wolves+bunnies STOPPED (torches are the only
  movers), camera at the wander home at zoom 1, per N ∈ {8, 16, 24} a ≥2-min
  wandering soak, metrics = `__lightcost` (ms/lighting-update min/med/max +
  light count) and `__framecost`, plus a capture — the deliverable is the
  TABLE in completed.md, with worker compose health beside it
  ([I6](issues.md#i6)).

**Exit**: the human reverted, the three-row table with numbers and captures,
findings recorded (a cliff is a finding, not a failure), wolves+bunnies
restored, docs+memory truth pass; the user's eyes close the stream.
