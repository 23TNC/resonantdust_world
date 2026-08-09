# Forks — player-pawns

## F1 — a new module stamped from pawn; ONE shard

`player_pawn` instantiates the SAME shared machinery the pawn module does — `entity_tables!`,
the payload sidecar, the spawn/mint machinery, the needs (and other gameplay) sub-tables — so
"all of the pawn methods" is literal: the ONE eval, the need-write sweep, the crossing
scheduler shape all apply to player-pawn rows without new laws. One shard
(`resonantdust-<env>-player-pawn-0`; ST DB names take no underscores): the population is
one-per-player and none of it is spatially hot. Rejected: rows inside the pawn shard behind a
subtype flag (mixes lifecycle — pawn rows are zone-hot and GC'd by movement laws; player-pawns
are login-lived) and rows in the players auth DB (it is deliberately low-write and carries no
gameplay machinery).

## F2 — exactly one per player, minted idempotently at login; linkage in the auth DB

First login mints the player's player-pawn (the spawn-log dedup already makes minting
idempotent); the players auth DB gains the linkage — a LIST of owned player-pawn references
with ONE active bit, length pinned to 1 for now. "Players might have multiple characters and
select one on login" is the recorded future: the login flow resolves THE ACTIVE player-pawn,
and nothing else in the system may assume the list is length 1. Rejected: minting on first
use (every consumer would need a not-yet-minted path; login is the one funnel that always
runs first).

## F3 — row-carriers first; the world arc stays open

v1 player-pawns are never rendered, never moved, never zone-fanned to viewports — they carry
gameplay rows (needs, conditions, emotions, traits-via-def) and that is all. The copied stamp
keeps position/movement/payload machinery live so the future "play as your character in the
world" arc needs no schema change. Rejected: trimming the copy down to a needs-only table
(saves nothing real, and re-growing it later means a second migration).

## F4 — player gameplay defs are SUBTYPES, superseding npc-host's tag choice

"New gameplay subtypes for player traits and player emotions etc." — the gameplay taxonomy
gains a `player` subtype lane: `wolf_pack`/`bunny_fluffle`/`area_of_influence` traits,
`wolf_count`-style needs, player emotions. Authored TOML like every gameplay def,
registry-numbered, APPEND-ONLY as ever. npc-host F7's "tags classify player traits" is
SUPERSEDED (noted in its folder): subtypes are the real classification; tags remain for
behavior checks (the attack lane). Assignment stays definitional: a player-pawn's
definition_reference carries its kind's binds, exactly as pawn kinds derive theirs.

## F5 — the npc module's player-pawn takes its BRAIN definition

Every npc module plays as a player-pawn (user). The brain def (npc-host F5 — the `brain`
type) IS the natural definition for that module's player-pawn: its constant trait binds
(wolf_pack level N, area_of_influence level M) land on the player-pawn and grant its needs
(wolf_count) — one derivation path, no parallel assignment machinery. Human players get a
default `player` definition with (for now) no binds. The exact taxonomy (is the human default
a `brain` kind or its own kind?) is pinned in the P0 paper.

## F6 — player-pawn rows fan to their OWNER, not to zones

Pawn rows fan per zone to whoever subscribes the zone. A player-pawn's rows fan to the OWNING
session (the way a session already receives its own login-scoped data) — an npc brain reads
its own wolf_count; a browser session reads its own player-pawn. Nobody else needs them in
v1; spectator/panel visibility for OTHER players' player-pawns is a named successor decided
when something wants it.
