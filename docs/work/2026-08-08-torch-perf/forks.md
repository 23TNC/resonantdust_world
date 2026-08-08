# Forks — torch perf

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — debug_torch is a MINIMAL pawn: walks + constant emit_light, no needs {#f1}
_2026-08-08 · resolved at plan time_

**Chosen.** `type = "pawn"`, one flat-tint placeholder part, `walks` level 2
(the wolf's brisk 12 tics/tile — more movement per second = more hot-bake
stress per pawn), `{ name = "emit_light", level = 1, constant = true }` (the
REAL warm torch tuple: reach 16, elevation 2.5, flicker), and an EMPTY needs
list — nothing decays, the brain manages nothing, and the measurement isn't
confounded by need-write traffic.

**Why walks 2, not a custom speed**: the test wants the shipped pipeline under
motion; authoring a new walks level for it would touch the append-only level
tables for no measurement gain. **Rejected — reusing the wolf kind**: wolves
drag hunger/thirst/brain habits into the scene; the point of a debug kind is
that it carries nothing else. **Rejected — a softer test light**: measuring a
smaller reach would flatter the numbers; the user asked what the real torch
costs.

## F2 — the npc module is a `torches` group brain, the bunnies' shape {#f2}
_2026-08-08 · resolved at plan time_

**Chosen.** A new `client/npc/src/brains/torches.rs` registered in the main.rs
brain match: mint up to `NPC_TORCHES` via SPAWN_REQUEST (pathable pick as
politeness, the 10 s adopt-first window, the refusal-retry posture — all
spawn-authority I2/I6 law), adopt from the state fan, and WANDER FOREVER —
on every authoritative arrival, roll a new pathable dest within the home
radius and issue `move_to`. Runs as its own container
(`NPC_BRAIN=torches NPC_TORCHES=N NPC_HOME=…`), exactly how rd-bunnies runs.

**Why the bunnies shape**: it is the proven group-mint/adopt/wander loop; the
torches brain is that loop minus needs minus predator logic. **Rejected — a
webgl-side spawner**: the client is the thing being measured; driving the load
from it would perturb the measurement. **Rejected — extending the bunnies
brain with a kind parameter**: entangles a debug harness with live wildlife
behavior; a 60-line brain is cheaper than the coupling.

## F3 — the measurement protocol, fixed before any number is read {#f3}
_2026-08-08 · resolved at plan time — so the three rows are comparable_

**Chosen.**
- **Scene reset**: redeploy the pawn module (wipes the cast), wolves + bunnies
  containers STOPPED — debug torches are the only movers ([I1](issues.md#i1)).
  World things (the 4 placed torches) stay: they are COLD lights, constant
  across all three rows, and stated in the table.
- **Camera**: zoom 1, focused on the wander HOME so every pawn stays in the
  subscribed/rendered window ([I5](issues.md#i5)); the browser tab foreground
  (a hidden tab freezes rAF — human-pawns I10).
- **Per N ∈ {8, 16, 24}**: restart the torches container with `NPC_TORCHES=N`
  (re-minting on the wiped world for row 1; rows 2/3 top up — adopted pawns
  count toward the cap, so the population is exactly N), let the wander settle
  ≥2 minutes, then read `__lightcost` (lights, msPerLightingUpdate min/med/max)
  and `__framecost` several times and record the spread ([I3](issues.md#i3)),
  plus one capture per row.
- **Beside each row**: the worker's composed-events health at that load
  ([I6](issues.md#i6)) — so a client number degraded by server backpressure is
  visible as such.

**Rejected — one long run ramping 8→16→24 without resets**: cheaper, but each
row inherits the previous row's stragglers and heat; the user asked for a
reset. **Rejected — FPS eyeballing as the metric**: the debug hooks exist and
give ms; FPS saturates at vsync and hides the curve.

## F4 — the human's drill bind REVERTS in this stream's P0 {#f4}
_2026-08-08 · the user's call, adopted_

The trait-lights P4 drill left `emit_light` constant on `human_male` (marked
in TOML for exactly this decision). It comes off first — before any
measurement, so no stray mover light rides the scene — and the golden
re-blesses with it. Zero storage was the feature: the revert is a pure corpus
edit; the existing human simply stops glowing on the next hot-swap.
