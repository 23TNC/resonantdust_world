# Plan — npc-host

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] docs: the module-IS-a-player design (F1/F4), the `brain` object type + player traits +
      player needs (F5/F7/F8) in VARIABLES.md/object-model, the wild-pawn fate stated (I8),
      the five-stage roadmap + emotion intent recorded. Acceptance: docs-check green.
- [ ] The I10 spikes, live: (a) where player need rows can live; (b) whether a module-player's
      mint/intent commands validate without own-session subscriptions. Acceptance: both
      answers written into forks.md with the probe evidence.

## P1 — brains as content

- [ ] content: the `brain` type (`content/brains.toml`, F5) — `wolf_pack` + `bunny_fluffle`
      brain defs with constant trait binds; loader/registry/goldens extended. Acceptance:
      content tests green; the defs registry-number.
- [ ] content: the `player`-tagged traits (F7) — `wolf_pack`/`bunny_fluffle` (per-level group
      size) + `area_of_influence` (per-level radius); the `wolf_pawn`-style granted need
      authored (F8). Acceptance: unit — trait params resolve per level; load green.

## P2 — the host of module-players

- [ ] lib: `Host` runs N modules, each a PLAYER (own login, F1); ONE shared world model read
      by all brains; one tick loop (I1/I4); env = which-brain-where (`NPC_MODULES=
      "wolf_pack@112,68;…"`), single-brain envs kept for the debug harness (I5). Acceptance:
      a two-module host boots; both players visible server-side.
- [ ] Ownership from spawn attribution (F4/I2): a brain commands only its mints and
      RE-ATTACHES to them on restart via the spawn log; kind-global adoption deleted.
      Acceptance: restart drill — the re-attached set equals the minted set exactly.
- [ ] Position + area: each module-player anchors at its position (F2); scans/wander/mints
      clamp to the `area_of_influence` radius (F3/I6). Acceptance: a soaked group stays
      in-area (log sample); an anchor move ~12 tiles migrates the group on camera (I9).

## P3 — stage-1 needs AI

- [ ] The `wolf_pawn` group need live (F8): written at mint/death on the brain-player; the
      banded low state drives re-minting through the needs machinery. Acceptance: kill a
      wolf — the need drops, the band fires, the brain re-mints to its trait's group size.
- [ ] The shared keep-alive policy (F6): banded thirst/hunger → nearest in-area water/food +
      drink/eat, else wander; wolves AND bunnies call it, per-brain copies die. Acceptance:
      both modules' pawns drink and eat via the ONE policy path (worker log).

## P4 — the drills

- [ ] The host control arc (I7): wolves + bunnies modules, fed by their own stage-1 AI (no
      forced bands), population stable over a ≥30-min soak. Acceptance: the soak log in
      completed.md.
- [ ] The measurement harness re-proof (I5): one mover-perf row (`NPC_BRAIN=debug`,
      NPC_COUNT=8) under the host fallback. Acceptance: the row lands with in-step verdict,
      recorded beside mover-perf's table.

## P5 — the truth

- [ ] Docs + memory truth pass + stack bounce with arcs green; **the user's eyes close the
      stream**. Acceptance: docs-check green; captures + logs in completed.md.
