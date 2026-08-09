# npc-host — one npc client, N module brains, anchored groups

_User (2026-08-08): ensure we have AN NPC CLIENT — N of these per realm. Each hosts a number of
npc MODULES (we have bunny, wolf, and debug today — uncertain if set up properly, uncertain if
more exist). The one client handles client/server communication and world handling; each module
is a "brain" for a GROUP of npcs. Each module has a LOCATION in the world, similar to a viewport
— its position — and an OPERATIONAL AREA from that point, under which it manages its group. The
pack of wolves / group of bunnies operates near its module's anchor. The anchor may MOVE, and
the group eventually operates in the new area by virtue of how the brain issues commands. We
implement basic AI in stages to keep bunnies/wolves alive._

## What exists (surveyed 2026-08-08)

`client/npc` is a lib (`Bot` + `Brain` trait + `run_brain`) + a thin binary. **One process = one
brain** (`NPC_BRAIN` ∈ wolves | bunnies | debug — the full catalogue; `torches` was absorbed
into `debug` by mover-perf F2). Each brain owns its own login, opens ONE hard-coded anchor in
`on_start`, and manages its group (bunnies/debug already run many pawns; wolves runs one).
Wolves and bunnies already carry embryonic needs-based behavior (lazy `needs_eval` over the
corpus bundle, nearest-water/nearest-food scans, eat/drink interactions) — the raw material for
stage 1, currently copy-adjacent per brain. There is NO multi-module host, NO per-module
position/area concept, and NO moving anchor.

## The stance

- **One host, one engine** (F1): the host owns ONE `Bot` (one login, one event pump, one world
  model); modules are registered on it and receive the shared view. N hosts per realm = N
  containers, each a distinct automated player (NPC_NAME), disjoint areas in v1 (I8).
- **A module's position IS a named client anchor** (F2): the anchor system already does
  "viewport-like position with radii" — each module opens its OWN named anchor at its position;
  moving the module = re-sending its anchor. No parallel position machinery.
- **The operational area** (F3): a tile radius around the module's position. Scans, wanders, and
  mint sites clamp to it; the group MIGRATES after an anchor move because every next command
  targets the new area — no teleporting, no herding verb.
- **Modules own their pawns** (F4): mint-tracking + adoption scoped to the module's area and
  kind, so two modules of the same kind (or two hosts) don't fight over pawns (I2).
- **Stage-1 AI = the keep-alive policy, shared** (F5): one needs-policy helper (drink when
  thirst bands, eat when hunger bands, else wander-in-area) that wolves and bunnies modules both
  call — consolidating what the two brains half-share today. Survival's drains now KILL, so the
  standing acceptance is population stability (I7).

## The AI roadmap (future intent — successor streams, recorded here so it survives)

1. **Stage 1 (THIS stream)**: rudimentary needs-based AI — keep the groups alive.
2. **Stage 2**: world-state-aware AI — threat/resource awareness beyond own needs (flee
   predators, remember depleted patches, prefer safe water).
3. **Stage 3**: sims4/rimworld-like autonomy — scored candidate actions (advertisements ×
   need weights), interruption, schedules.
4. **Stage 4**: each npc MODULE gets a small neural network with weights modulating the stage-3
   scoring — each brain operates somewhat differently.
5. **Stage 5**: each individual NPC gets a rudimentary network augmenting the commands its brain
   issues — groups differ by what they have gone through (weights as experience).

Stages 2–5 are NOT planned here; they constrain shape: the module API keeps a clean seam between
"sense" (bot view, scoped to area), "decide" (the policy — swappable per stage), and "act"
(commands on owned pawns), so later stages replace DECIDE without touching the host.

## Exit

One host process runs the wolves module AND the bunnies module concurrently over one login, each
group alive and operating inside its own area over a soak; an anchor move on camera migrates its
group; the debug measurement harness still runs. The user's eyes close the stream.
