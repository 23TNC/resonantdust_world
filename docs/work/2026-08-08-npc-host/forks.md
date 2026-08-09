# Forks — npc-host

## F1 — one host process, ONE world model, modules ARE players (user, 2026-08-08)

"The one npc client will handle the client/server communication and world handling etc" + the
revision: "every npc module is a player." So: the HOST is one process owning ONE shared world
model (tiles/things/tic estimate parsed once, read by every brain); each MODULE is a PLAYER —
its own login/identity, whose anchor is its position and whose mints are its pawns. The edge
fans zones per session, so when two modules' areas overlap the same zone rides two sessions on
the wire — accepted at v1 (localhost, bounded by overlap; the shared model dedupes in memory);
edge session-multiplexing is the named successor if it ever measures as a cost. Rejected:
one login for all modules (ownership and traits would need a parallel host-side concept the
PLAYER already provides); threads-per-module (one async loop over modules is deterministic —
I4 watches starvation). "N npc clients per realm" = N host containers.

## F2 — a module's position IS a named client anchor

`Command::SetAnchor` already takes a NAME and the anchor system already models "a position with
nested radii, cost-based release" (client-anchor-zones). Each module opens `npc:<module-name>`
at its position; MOVING the module = re-sending the same named anchor at the new tiles. This is
NOT a per-module subscription stack (user asked, 2026-08-08): the ONE client engine merges all
anchors into a SINGLE zone set — each zone subscribed once, at the tier owed to the nearest
anchor — so overlapping module neighborhoods share their zones and nothing streams twice; the
anchor is only how the one client is told WHERE to look. Rejected:
a parallel module-position concept beside anchors (two notions of "where the npc looks" that
would drift); one shared anchor for all modules (modules must be placeable independently — a
warren and a pack in different corners of the realm).

## F3 — the operational area is a tile radius, clamped in the SENSE/ACT helpers

`area = (center, radius)` per module, config-authored. The clamp lives in the module-scoped
helpers (scan-in-area, wander-in-area, mint-in-area) — NOT in the worker and NOT as a fence:
a pawn mid-walk past the edge is fine; the NEXT command targets inside. Migration after an
anchor move is emergent — exactly the user's "eventually operate in the new area by virtue of
how the brain will issue commands". Rejected: server-side territory (nothing in the sim knows
about modules; the npc is just a player).

## F4 — ownership is PLAYER ownership via spawn attribution (user, 2026-08-08)

"We shouldn't have issues fighting over pawns. Every npc module is a player… as every npc module
spawns its npcs we shouldn't have an issue." Ownership = the pawn's minting player
(spawn-authority already routes every mint through SPAWN_REQUEST and its spawn log — the
attribution exists server-side). A brain commands ONLY pawns its player minted; on restart it
re-attaches to them through that attribution instead of kind-scanning. Two wolf-pack modules in
the SAME area function fine — each herds its own mints. Kind-global adoption (today's bunnies
shape) DIES. Cross-host needs nothing special: a player is a player.

## F5 — brains are CONTENT: a new `brain` object type + player traits (user, 2026-08-08)

REVISED from an env spec at review. A new object TYPE `brain` with its own
type/subtype/kind/variant, authored in TOML (`content/brains.toml`, registry-numbered like
everything else) — "more entry points… a place to stuff constants for our brains." A brain def
carries CONSTANT trait binds (the trait-lights law: TOML-only, zero-storage, derived): its
group trait and its area trait. The host env shrinks to WHICH brain def and WHERE:
`NPC_MODULES="wolf_pack@112,68;bunny_fluffle@120,75"` — name + anchor position only; radius and
group size come from the def's traits. The single-brain envs (`NPC_BRAIN`/`NPC_KIND`/
`NPC_COUNT`) stay as the debug-harness fallback (I5).

## F7 — player traits ride the delivered trait machinery

"Add a new trait type, player traits… this also allows us to give players traits too."
CLASSIFICATION SUPERSEDED by [2026-08-08-player-pawns](../2026-08-08-player-pawns/README.md)
F4 (user, later same day): player traits/emotions/needs are gameplay SUBTYPES (`player` lane),
not tags — tags stay for behavior checks. The numbers remain per-level PARAMETER arrays (the
shape emit_light proved, trait-lights F4):
- `wolf_pack` / `bunny_fluffle` — per-level group size ("the number of wolfs/bunnies the
  player has"); the brain's mint guard reads it.
- `area_of_influence` — per-level radius, "wider or smaller areas"; the operational-area clamp
  (F3) reads it.
An npc module-player derives its traits from its brain def's constant binds. HUMAN players
carrying traits is the door this opens — recorded as intent, not planned here.

## F8 — players get NEEDS; the group's state IS a need (user, 2026-08-08)

"Players can get needs too. This then allows us to assign a wolf_pawn need to our wolf brain
via a wolf_pack trait. That need can then track the number of active wolfs." So the
`wolf_pack` trait GRANTS a `wolf_pawn` need on the brain-player (a trait granting a need is
the new capability; modifiers on needs already exist), and its value counts the player's LIVE
owned pawns — written at mint and at death, the way the inventory need tracks free slots.
"Running out of pawns" then stops being bespoke brain state: the need bands, a condition
activates, and the existing needs machinery (bands → conditions → need-write triggers) drives
the response — the mint guard becomes a banded-need reaction instead of an ad-hoc counter.
Where player need ROWS live is ANSWERED by the preempting
[2026-08-08-player-pawns](../2026-08-08-player-pawns/README.md) stream: on the player's
PLAYER-PAWN (the need there named `wolf_count`) — this stream consumes that machinery.

The full stack follows (user, 2026-08-08): with needs come CONDITIONS, and with conditions
EMOTIONS — a brain-player whose group is dying bands its `wolf_pawn` need, activates a
condition, and shifts emotion, "which could alter how the npc brains evaluate and handle their
pawns" (a scared brain hoards its pack near water; a content one ranges wide). Nothing new to
build — the ONE eval already computes all three for any row-carrier; the brain reading its OWN
emotion as a decision input is the seam stage 3's autonomy scoring and stage 4's per-module
network plug into. Recorded as intent; stage-1 plans only the need.

## F6 — stage-1 AI is ONE shared keep-alive policy, not per-brain copies

Wolves and bunnies each half-implement needs-seeking today. One policy helper — evaluate the
module's pawns' lazy needs; thirst banding → nearest water in area + drink; hunger banding →
nearest diet-gated food in area + eat; else wander in area — parameterized by diet, called by
both modules. The seam stays sense/decide/act so stages 2–5 swap DECIDE only. Rejected:
leaving the two brains' logic diverged (stage 2+ would fork four ways).

## F9 — the I10 spikes, answered (P0, live evidence)

**(a) Player need rows** live on the PLAYER-PAWN in the `player_pawn` shard — delivered whole
by [2026-08-08-player-pawns](../2026-08-08-player-pawns/README.md): both npc brains already
own linked player-pawns carrying `wolf_count` (their completed.md P4 has the probes).

**(b) A module-player's commands validate WITHOUT own-session subscriptions**: a fresh probe
session (`SpikeB`) logged in, anchored NOTHING, subscribed NOTHING, and queued a bare
`SPAWN_REQUEST` for a bunny at (116, 71) — the edge accepted (`queue_ok`) and the worker
MINTED (`spawn request minted … x=116 y=71 def=0x300100d0 tic=43531`). So the host keeps ONE
shared world model and the module-players stay command-only logins. The one caveat: a zone
must have been GENERATED (someone subscribed it once) for worldgen-dependent validation —
the HOST's model subscription covers that by construction (its anchors are the modules').

## F10 — the brain def's login resolution (P0, the paper's law)

The mint funnel picks a player-pawn's def by LOGIN NAME: a `[[brain]]` def whose name equals
the player's name wins (the host logs each module-player in AS its brain — `wolf_pack` logs
in as `wolf_pack`); everything else takes the default `player` thing def. One resolution
point; the future character-select generalizes it. Brain parameters (group_size,
area_radius) ride the STAT lane — per-level `stats` contributions on the player traits, read
through the ONE stat eval (no new parameter machinery). VARIABLES.md §Brains is the paper.

## F11 — the P3 group-module shape (resolved before the build)

ONE `brains/group.rs` replaces the wolves/bunnies pair for HOSTED modules: a `GroupModule`
parameterized by (pawn kind name, diet) whose state is the OWNED set (fed by
`Event::OwnedPawn` — never a kind scan; the legacy brains keep kind-scanning only until the
ambient-wildlife containers retire onto the host, which is when item 2's "adoption deleted"
box ticks). Tick order per module: (1) BOOKKEEPING — wolf_count/bunny_count = |owned ∩ live|,
written via SET_NEED on the module's own player-pawn whenever it drifts (F8: mint/death move
the need); (2) MINT GUARD — while count < group_size (the brain's stat) queue SPAWN_REQUEST
inside the area (the banded `packless` condition is the OBSERVABLE, the guard reads the same
number); (3) KEEP-ALIVE per owned pawn (F6) — evaluate lazy needs off the WORLD model's rows,
banded thirst → nearest in-area water + drink, banded hunger → nearest in-area diet-gated
food + eat, else wander-in-area (the F3 clamp lives in these helpers; area-starved falls
through to wander, I6). The host passes (world &Bot, module &mut HostModule) — sense/decide/
act stays the seam. Positions of owned pawns come from the world model's StateObject events
(the host keeps a per-entity position map — new harness state, noted once, read by all).
