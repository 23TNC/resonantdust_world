# Forks — npc-host

## F1 — one host process, ONE client engine, N modules

The user's shape verbatim: "the one npc client will handle the client/server communication and
world handling etc, and each module will act as a brain for a group." So the host owns ONE `Bot`
(one login, one event pump, one world model — tiles/things/tic anchor stay harness state);
modules register on the host and share the view read-only, acting only on pawns they own.
Rejected: N logins in one process (defeats the point — the world model would be duplicated per
module, and the edge sees N players); threads-per-module (the tick loop is cheap; one async loop
fanning events in registration order is simpler and deterministic — I4 watches starvation).
"N npc clients per realm" = N host CONTAINERS, each its own automated player.

## F2 — a module's position IS a named client anchor

`Command::SetAnchor` already takes a NAME and the anchor system already models "a position with
nested radii, cost-based release" (client-anchor-zones). Each module opens `npc:<module-name>`
at its position; MOVING the module = re-sending the same named anchor at the new tiles. Rejected:
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

## F4 — pawn ownership = minted-by + in-area adoption, per module

Each module tracks the pawns it minted (the bunnies re-mint guard generalizes); adoption of
pre-existing pawns of its kind is claimed ONLY inside its area, first-module-wins within a host.
Cross-host arbitration is out of scope (I8): v1 runs hosts with disjoint areas. Rejected:
kind-global adoption (today's shape — two same-kind modules would fight).

## F5 — the config is an env spec string parsed by the host

`NPC_MODULES="wolves@112,68,r8;bunnies@120,75,r10"` — `<module>@<x>,<y>,r<radius>` semicolon-
separated, position/radius optional with per-module defaults. The existing single-brain envs
(`NPC_BRAIN`/`NPC_KIND`/`NPC_COUNT`) stay as the one-module fallback so THE debug measurement
harness (mover-perf) keeps working unchanged. Rejected: a config TOML file (another authored
surface + serving question for three knobs; revisit when hosts multiply); CLI args (the
container runner passes env).

## F6 — stage-1 AI is ONE shared keep-alive policy, not per-brain copies

Wolves and bunnies each half-implement needs-seeking today. One policy helper — evaluate the
module's pawns' lazy needs; thirst banding → nearest water in area + drink; hunger banding →
nearest diet-gated food in area + eat; else wander in area — parameterized by diet, called by
both modules. The seam stays sense/decide/act so stages 2–5 swap DECIDE only. Rejected:
leaving the two brains' logic diverged (stage 2+ would fork four ways).
