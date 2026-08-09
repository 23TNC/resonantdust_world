# Completed — player-pawns

## 2026-08-08 — P3 (first half): the player gameplay lane

**codec** — `GAMEPLAY_PLAYER_TRAIT = 7` appended (palette 7 entries, the frozen-order test
extended). **loader** — `[[player_trait]]` parses through the ONE trait conversion (chained,
split by count); `Bundle.player_traits` + names + `player_trait_params` +
`thing_player_traits`; ONE bind namespace (collision refused); player-trait binds are
CONSTANT-only on every carrier (no stored row can name the category until a payload opcode
does — refined from the paper's "same lanes" to a DEDICATED accessor, the safer v1).
**content** — `players.toml`: `wolf_count` (0..16, deplete 0, `packless` band at 0..1 with
scared/uncomfortable emotions — bands, never kills); `wolf_pack` player trait; the `player`
thing def (subtype `player` id 3) LIVES AT THE END of things.toml — the first attempt
mid-corpus shifted every thing's seed order (191 divergences); the one mis-recorded registry
row was surgically SQL-deleted and re-seeded clean at 0x30030120. **edge mint** — resolves
the `player` def + full-at-max need rows from the live worldgen bundle (constant player
traits derive, never mint). Consumer sweep run (deny_unknown_fields makes old loaders refuse
the new table): worker/master/npc/wasm/webgl rebuilt, sims restarted. Verified: 70+2 content
tests green (golden re-blessed deliberately); a fresh login's mint carries def 0x30030120 +
needs row 0xFFFF0050 (wolf_count FULL); every player incl. Developer now owns a linked
player-pawn.

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
