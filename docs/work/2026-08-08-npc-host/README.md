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

## The stance (revised at review, 2026-08-08 — the user's model)

- **Modules ARE players** (F1/F4): each npc module is a player — its own identity, its anchor
  as its position, its mints as its pawns (ownership = spawn attribution; the spawn log already
  records the minter). Two same-kind modules co-locate fine — each commands only its own mints;
  kind-global adoption dies. The HOST is one process with ONE shared world model that all
  brains read; N hosts per realm = N containers.
- **A module's position IS its player's anchor** (F2): players already have anchors — the
  module-player anchors at its position; moving the module = re-sending the anchor. No parallel
  position machinery.
- **Brains are CONTENT** (F5): a new object type `brain` (own type/subtype/kind/variant,
  TOML-authored, registry-numbered) — the entry point holding a brain's constants as CONSTANT
  trait binds. Host env shrinks to which-brain-where.
- **Players carry traits** (F7): `wolf_pack` / `bunny_fluffle` (per-level group size) and
  `area_of_influence` (per-level radius — wider or smaller areas by level) as `player`-tagged
  traits with per-level parameters, the delivered trait machinery. Human players carrying
  traits is the same door.
- **Players carry needs; the group's state is a need** (F8): the `wolf_pack` trait GRANTS a
  `wolf_pawn` need tracking the player's live owned pawns — "ran out of pawns" bands like any
  need and the existing bands→conditions→triggers machinery reacts; the mint guard becomes a
  banded-need response, not an ad-hoc counter.
- **The operational area** (F3): the radius from `area_of_influence`. Scans, wanders, and mint
  sites clamp to it; the group MIGRATES after an anchor move because every next command targets
  the new area — no teleporting, no herding verb.
- **Stage-1 AI = the keep-alive policy, shared** (F6): one needs-policy helper (drink when
  thirst bands, eat when hunger bands, else wander-in-area) that wolves and bunnies modules both
  call. Survival's drains now KILL, so the standing acceptance is population stability (I7).

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

Adjacent intent (user, 2026-08-08): brain-players eventually carry the FULL gameplay stack —
with needs (F8) come conditions and EMOTIONS, altering how a brain evaluates and handles its
pawns (a scared brain plays differently from a content one). The ONE eval already computes all
three for any row-carrier; the brain reading its own emotion is a stage-3/4 decision input.

## Exit

One host process runs the wolves module AND the bunnies module concurrently over one login, each
group alive and operating inside its own area over a soak; an anchor move on camera migrates its
group; the debug measurement harness still runs. The user's eyes close the stream.
