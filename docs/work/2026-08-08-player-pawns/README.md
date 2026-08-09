# player-pawns — players play AS a pawn; the shard that carries their rows

_User (2026-08-08): a new shard `player-pawn` — a COPY of pawns. Players play "as" their
player-pawn. A player may own multiple player-pawns but only one is ACTIVE at a time; for now
exactly ONE per player. In theory players later have multiple "characters" and select one on
login. By doing this we have ALL the pawn methods available. As per npc-host: new gameplay
SUBTYPES for player traits, player emotions etc., assignable to player-pawns. Every npc module
plays as a player-pawn — so brains get needs (wolf_count etc.), emotions, conditions._

**This stream PREEMPTS [2026-08-08-npc-host](../2026-08-08-npc-host/README.md)** — it answers
npc-host's open I10(a) (where do player need rows live? → on the player-pawn, in this shard)
and supersedes its F7 tag-classification with real gameplay SUBTYPES.

## What exists (surveyed 2026-08-08)

`server/spacetime/server/modules/pawn` is the hot mover shard: the shared
`entity_tables!` stamp (clock / entity_state_log / entity_state + init/bump/claim/write/gc),
the payload sidecar (slaved opcode stream), the spawn machinery (server-minted ids +
`spawn_log` dedup), and the gameplay sub-tables (needs, inventory) the ONE eval reads.
`modules/players` is the AUTH database — accounts, login, identity↔player_id routing — with
no gameplay rows at all. Pawn gameplay state is packed 16+16 rows + lazy need rows; every
consumer already evaluates them through the one shared eval. Nothing anywhere gives a PLAYER
a row-carrier.

## The stance

- **A new module `player_pawn`, stamped from pawn** (F1): same `entity_tables!` core, payload
  sidecar, spawn/mint machinery, and gameplay sub-tables — a copy, so every pawn method
  (needs, conditions, emotions, inventory, the eval) works unchanged on a player-pawn. ONE
  shard (`…-player-pawn-0`): player-pawns are few and not spatially hot.
- **Exactly one per player, minted at login** (F2): idempotent mint on first login; the
  players auth DB carries the linkage (player → player-pawn reference + ACTIVE). "Multiple
  characters, chosen on login" is the recorded future — the linkage is a list with one active
  bit from day one, even while its length is pinned to 1.
- **Row-carriers first** (F3): v1 player-pawns hold gameplay rows and are NOT rendered or
  moved in the world. The copied machinery keeps position/movement available for the future
  character/avatar arc; nothing uses it yet.
- **Player gameplay SUBTYPES** (F4): traits, emotions, needs etc. authored under a `player`
  subtype in the gameplay taxonomy — `wolf_pack` traits, `wolf_count` needs, player emotions —
  assignable to player-pawns via their definition, exactly as pawn kinds derive theirs.
- **Every npc module plays as a player-pawn** (F5): the brain's login resolves its
  player-pawn; the brain def's binds land on it; wolf_count/emotions/conditions ride the
  standard machinery. npc-host builds directly on this.

## Exit

A fresh login owns exactly one player-pawn; a `player`-subtyped need (`wolf_count`) and trait
assigned through its definition evaluate on it (bands, conditions, emotion argmax) via the
same eval as any pawn; a SET_NEED-style write targeting the player-pawn round-trips through
the event system; npc-host's I10(a) is answered in its folder. The user's eyes close the
stream.
