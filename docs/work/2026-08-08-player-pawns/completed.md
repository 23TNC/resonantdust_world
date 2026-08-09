# Completed — player-pawns

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
the `…-playerpawn-0` database row, the `player_pawn` module section (pawn's stamp
re-instantiated, differences named: type nibble + owner fan), and the `players.player_pawns`
linkage shape. npc-host's I10(a)/F7/F8 were annotated at plan time. Verified: docs-check
green (676 files, the 4 standing warnings).
