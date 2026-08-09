# Completed — player-pawns

## 2026-08-08 — P2: mint-at-login

**The linkage** — `players.player_pawns` (PK player_pawn_reference, btree player_id, active)
+ `link_player_pawn`: idempotent, first link becomes ACTIVE inside the reducer — the ONE
enforcement funnel (I4). Published as an in-place update (additive table, accounts kept —
though the earlier P2 publish DID reset the DB: ids restarted at 1024).

**The funnel** — the edge's `ensure_player_pawn`, spawned DETACHED after LoginOk (I6: a
failing shard can never block or fail a login): linkage-cache fast path → own player_pawn
connection → `spawn` with ledger key `(player_id, 0)` (the login funnel's dedup, def/needs
empty until P3) → minted ref off `spawn_log` → `link_player_pawn_then` awaited on a
connection THE TASK OWNS. Two real defects found and fixed by the drill: (1) a bare reducer
submit is only a QUEUE — a login-and-quit session tore the connection down before the flush
and the link silently vanished; (2) even the `_then` form dies when it rides the SESSION's
players conn, because session teardown calls `disconnect()` explicitly — the fix is a
dedicated connection whose lifecycle ends after the outcome arrives. Verified LIVE: five
players (npc Wolves/Bunnies auto-minted on reconnect — every npc module IS a player-pawn
owner now — plus three drill users) each exactly one linkage row, active=true; a re-login
linked the SAME deduped ref (0x40800004) healing its earlier deferral; quit-fast logins land;
login_ok always returned even when the link deferred (the degrade path, observed twice).

## 2026-08-08 — P1: the shard + the routing lane

**The module** — `server/spacetime/server/modules/player_pawn`: pawn's stamp re-instantiated
(entity_tables! + payload sidecar + needs/inventory sub-tables + spawn machinery + the relay
reducers), differing ONLY in the minted server byte (`TYPE_PLAYER` → `0x40`) and prose;
`remove` kept for stamp parity but nothing calls it (I8). The redeploy family map gained
`player_pawn → player-pawn` (the event-shard hyphen convention). Published as
`resonantdust-dev-player-pawn-0`; verified: all six tables live + empty via HTTP SQL.

**The routing lane (I1)** — st-bindings generated (via the daemon container's CLI into the
mounted tree, moved into `server/st-bindings/src/player_pawn`); orchestrator: `TYPE_PLAYER` →
hot shard key 4, uplink + claim arm; worker: `Shard::PlayerPawn` in `shard_of`, a subbed
uplink (pawn's subscription shape minus inventory), SET_NEED/GRANT_CONDITION relays dispatch
by the target's type nibble, the ghost-row live guard mirrored for player-pawn targets, the
WRITE arm, and `base_row` routed. Self-heal rides the shared uplink machinery (the worker log
shows `player_pawn connected`). Verified LIVE: hand-minted `0x40800000` via the spawn
reducer (spawn_log `[278003712, 4242, 0, 0x40800000]`), then a WS-queued `SET_NEED`
(`[10, 0x40800000, 30000<<16|0x123]`) traveled edge → event shard → orchestrator claim →
worker: entity_state_log gained the COMPOSED row (tic 24348, worker 0x62) beside the spawn
row, and the needs row upserted to exactly the queued packed value at the same tic.

## 2026-08-08 — P0: the paper

**VARIABLES.md** gained the Player-pawns section: the `TYPE_PLAYER` reference lane (server
byte `0x40 | server_id` — the type nibble IS the routing, surveyed against the codec: the
constant already exists, unused), the linkage + ACTIVE law (list pinned to 1, one login
funnel, nothing may assume length 1), row-carriers-first + the OWNER fan, the active lanes
(bands/conditions/emotions ON; death OFF **by construction** — player defs author no
`can_die`, the sweep's existing re-validation is the gate, no new switch), the `player_trait`
category (appended to the palette; at LOAD its rows enter the SAME eval lanes — one adapter,
the ONE-eval law holds; `player_emotion` recorded as the next append; player needs are
ordinary `need` defs), and the definition law (npc player-pawns take their BRAIN def, humans
the default `player` def). The `player_pawn_reference` alias row added. **TABLES.md** gained
the `…-player-pawn-0` database row, the `player_pawn` module section (pawn's stamp
re-instantiated, differences named: type nibble + owner fan), and the `players.player_pawns`
linkage shape. npc-host's I10(a)/F7/F8 were annotated at plan time. Verified: docs-check
green (676 files, the 4 standing warnings).
