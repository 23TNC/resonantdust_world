# Plan — player-pawns

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md/TABLES.md/object-model: the playerpawn shard, the reference namespace +
      routing lane (I1), the player↔player-pawn linkage (list + ONE active, F2), the `player`
      gameplay subtypes (F4). Acceptance: docs-check green.
- [x] State the active/inactive lanes for player-pawns (I8): bands/conditions/emotions ON,
      death sweep OFF; and the owner fan shape (F6). Acceptance: written in the paper;
      npc-host's I10(a)/F7/F8 annotated as answered/superseded (I5).

## P1 — the shard

- [x] The `player_pawn` module stamped from pawn (F1): entity_tables! + payload + spawn +
      needs sub-tables; published as `…-player-pawn-0`. Acceptance: publish green; a live SQL
      probe reads the empty tables.
- [x] Routing: the reference lane through edge allowlist → orchestrator claim → worker write
      (I1/I2); self-heal covers the new DB. Acceptance: a hand-queued write to a player-pawn
      lands in its entity_state_log.

## P2 — identity

- [x] Mint-at-login, idempotent + non-blocking (F2/I6): the players auth DB carries the
      linkage with the active bit; one enforcement funnel (I4). Acceptance: two logins, one
      row; a bounced shard degrades, not blocks.

## P3 — player gameplay

- [x] content: the `player` gameplay subtypes (F4) — a `wolf_count` need + a starter player
      trait authored; the player-pawn's definition derives them (F5). Acceptance: content
      tests green; the derived rows appear on a minted player-pawn.
- [x] The owner fan (F6/I3): the session receives its OWN player-pawn's rows through the
      edge + client/core surface. Acceptance: the npc Bot (or a probe client) logs its
      wolf_count row after login.

## P4 — the drills

- [x] The eval drill: write wolf_count low through the event system; the band/condition/
      emotion evaluate via the ONE eval; NO death lane at zero (I8). Acceptance: probe
      evidence in completed.md (I7).
- [ ] The npc coupling drill (F5): an npc login plays as its player-pawn with its brain-def
      binds landed. Acceptance: the brain's wolf_count visible on ITS player-pawn.

## P5 — the truth

- [ ] Docs + memory truth pass + stack bounce with arcs green; **the user's eyes close the
      stream**. Acceptance: docs-check green; captures + logs in completed.md.
