# Plan — npc-host

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] docs/components/client/npc: the host/module/anchor design (F1–F4), the sense/decide/act
      seam, and the five-stage AI roadmap recorded as future intent; the work index row.
      Acceptance: docs-check green.

## P1 — the host

- [ ] lib: `Host` owns the ONE `Bot` + a module registry; events noted once then fanned
      read-only to each module; one tick loop over modules (I1/I4). The `Brain` trait becomes
      the module API (sense/decide/act seam, F6). Acceptance: cargo build green.
- [ ] main.rs: parse `NPC_MODULES` (`<module>@<x>,<y>,r<n>`; F5) into registered modules; the
      single-brain envs (`NPC_BRAIN`/`NPC_KIND`/`NPC_COUNT`) keep working as a one-module
      fallback (I5). Acceptance: both spellings boot in the container.
- [ ] Wolves + bunnies + debug become modules of the host (ownership: minted-by + in-area
      adoption, F4/I2). Acceptance: a two-module host (wolves + bunnies) runs both groups over
      ONE login, worker log shows both minting/moving.

## P2 — position, area, migration

- [ ] Each module opens its OWN named anchor (`npc:<module>`) at its configured position (F2);
      zones stream per anchor. Acceptance: a host with two far-apart modules streams BOTH
      neighborhoods (log: zone counts per anchor, I3 recorded).
- [ ] The area clamp in the module helpers: scans, wander targets, and mint sites stay within
      `(center, radius)` (F3), wandering when the area starves (I6). Acceptance: a soaked
      group's positions stay in-area (log sample).
- [ ] The anchor-move API: a module re-anchors to a new center; the group MIGRATES by successive
      commands (I9 churn watched). Acceptance: on camera — move the bunny module's anchor
      ~12 tiles; the warren re-forms near the new center.

## P3 — stage-1 needs AI

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
