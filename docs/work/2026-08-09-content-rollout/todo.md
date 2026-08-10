# Plan — content-rollout

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

**The phase order is forced, not preferred** ([F1](forks.md#f1)): passive rollout cannot terminate
while the npc caches a boot-time def, and active rollout cannot be composed while the worker holds a
boot-time corpus. P1 before everything.

**How acceptance is measured.** Rust changes land on `rd check` green plus the crate's own tests;
anything touching the corpus adds a `shared/content` test (the golden fixture re-blesses only with
`BLESS_GOLDEN` and a stated reason). Behavioural items land on a **named drill** with its log or
capture recorded in [`completed.md`](completed.md) — a green build is not evidence that a bunny
forages.

## P0 — the paper

- [ ] Add `RESTAMP_DEF` (20) and `ROLLOUT_DEF` (21) to [`docs/ACTIONS.md`](../../ACTIONS.md) §World
      with per-operand read/write marks and the fan-out note (F6). Acceptance: docs-check green;
      the write-set rationale is stated, not implied.
- [ ] Add the `sim: u64` column to [`docs/TABLES.md`](../../TABLES.md) § `definitions` with its
      report-not-refuse semantics (F8). Acceptance: docs-check green; the row says what a mismatch
      means and who reports it.
- [ ] Write `docs/components/dev/scripts/rd/design/content-rollout.md` — the three lanes, who polls
      what, and the conserve/re-derive table (F1/F2/F7). Acceptance: docs-check green; linked from
      the component map.
- [ ] Cross-link the proposed
      [`version-predicates.md`](../../components/server/spacetime/modules/index/intent/version-predicates.md)
      from this stream and back. Acceptance: both name the other; neither claims its scope.

## P1 — the corpus lane (nothing works before this)

- [ ] VERIFY I1 before any repoint: confirm the worker reads no `[[biome]]` data, by grep plus a
      worker test loading the SERVED corpus. Acceptance: a recorded verdict in
      [`issues.md`](issues.md#i1); a failure takes F2's fallback instead.
- [ ] Extract the poll-and-swap into one shared helper (fetch `/content-version`, on change fetch
      `/content`, load, hand back a `Bundle`) usable by worker, master and npc. Acceptance: one
      implementation; each consumer supplies only its swap callback.
- [ ] Worker: hold the corpus as `Arc<Bundle>`, clone it once at the top of each pass, swap from the
      poll task (F3). Acceptance: a corpus change mid-run is picked up with no restart, and no pass
      ever observes two bundles.
- [ ] Worker: retire the `CONTENT_DIR` disk read in favour of the edge's `/content` (F2).
      Acceptance: `CONTENT_DIR` is gone from the worker and its compose; a corpus-less edge still
      exits loudly rather than running on defaults.
- [ ] Master: re-seed the definition registry on every corpus swap, not only at boot (F4).
      Acceptance: a newly authored def gets a registry row within one poll interval, with no master
      restart; a repeat seed writes nothing.
- [ ] npc: re-fetch the corpus on the same poll and swap it between brain ticks, never inside one
      (F3). Acceptance: `NPC_BRAIN=bunnies` picks up a corpus change with no restart.
- [ ] npc: brains RE-RESOLVE `def` / `kind` / cached gameplay refs on a swap instead of caching them
      at boot (F9) — the bug that makes passive rollout never terminate. Acceptance: after a corpus
      change the replenish mint uses the NEW def id, logged.
- [ ] Drill: edit `things.toml`, save, and confirm all five holders (edge, browser, worker, master,
      npc) report the new fingerprint with nothing restarted. Acceptance: five log lines / one
      capture in [`completed.md`](completed.md).

## P2 — staleness made visible

- [ ] Add `sim: u64` to `definitions` and carry `thing_sim_version` / `tile_sim_version` through the
      master's allocator into `ensure_definition` (F8). Acceptance: seeded rows carry a nonzero
      `sim`; a repeat seed on an unchanged corpus is still a no-op.
- [ ] On re-seed, log every tuple whose recorded `sim` differs at the same `version`, naming the def
      and its live count (F8). Acceptance: an in-place bunny edit produces that line within one
      poll; `ensure_definition` still accepts the row.
- [ ] `rd rollout --status <kind>` — live counts by `definition_reference`, newest version marked,
      stale rows flagged, corpus-orphans counted apart (F9/I6). Acceptance: the bunny version split
      prints correctly against a known world.

## P3 — the re-stamp verb

- [ ] Add `RESTAMP_DEF` = 20 to `shared/codec` with signature `[Write, Imm]` and its palette entry.
      Acceptance: `rd check` green; the action round-trips through the packer/decoder test beside
      `SPAWN_REQUEST`'s.
- [ ] Worker arm: resolve old + new defs, diff the non-constant trait binds, write the union rule
      (add new, remove old-def-authored, keep unauthored — F7). Acceptance: a unit test over three
      rows proves an `ACTIVATE_TRAIT`-acquired row survives a re-stamp.
- [ ] Worker arm: re-mint `PART` slots from the new part count, preserving spawn-chosen variant
      nibbles slot-for-slot, including the 1↔2 boundary where the nibble moves (I5). Acceptance: a
      test per direction; a re-stamped human keeps its head variant.
- [ ] Worker arm: need rows keep their value, re-clamp to the new bounds, mint newly authored needs
      at effective max, drop removed ones (F7). Acceptance: a test showing half-full hunger stays
      half-full and a lowered cap clamps, not resets.
- [ ] Route the whole re-stamp through ONE state-write transaction on the pawn module, per the
      slaved-sidecar invariant (I7). Acceptance: no direct `payload`/`needs` poke outside a state
      write; the `spawn` reducer's shape is the reference.
- [ ] Refuse and leave the entity untouched when the target def is unresolvable in the corpus (I6).
      Acceptance: a deleted `[[thing]]` yields a named log line and zero writes.

## P4 — the fan-out and the operator command

- [ ] Add `ROLLOUT_DEF` = 21 to `shared/codec`, signature `[Imm, Imm, Imm]` with an EMPTY write set
      (F6). Acceptance: `rd check` green; a grouping test proves it claims nothing.
- [ ] Worker arm: enumerate live entities on `from` across the `pawn` + `player_pawn` mirrors and
      queue one `PROMOTE RESTAMP_DEF` each (F6/I10). Acceptance: N matches produce N events in N
      separate components, logged with the count.
- [ ] `rd rollout <kind>` — resolve `from`/`to` against `/definitions`, REFUSE if the target id is
      absent, `--wait` to poll for the re-seed (I8). Acceptance: rolling to an unseeded version
      fails at the door with a message naming what is missing.
- [ ] `rd rollout --dry-run` prints the per-entity trait deltas and need clamps it would apply,
      writing nothing (I4). Acceptance: the printed plan matches what a subsequent real run does.
- [ ] Confirm neither verb is in `CLIENT_VERBS` and that the edge rejects both from a client socket
      (F10). Acceptance: a client-issued `RESTAMP_DEF` is refused and logged, like `MOVE_STEP`.

## P5 — the drills that close the stream

- [ ] ACTIVE, the user's case: with bunnies alive, add a trait to `bunny` in place, save, then
      `rd rollout bunny`. Acceptance: every live bunny forages on camera; hunger unchanged across
      the roll; no death the values didn't already imply.
- [ ] PASSIVE, the user's case: with the corpus changed and NO rollout issued, kill bunnies and let
      the warren replenish. Acceptance: `--status` drains old-version count to zero with nothing
      restarted, and the replacements carry the new trait.
- [ ] The AUTHORED-VERSION path: append a `bunny` block with `version = 1`, let the master number
      it, roll v0 → v1. Acceptance: `entity_state.definition_reference` moves to the new id and the
      entities keep their position, needs and conditions.
- [ ] Mid-flight swap (I2): change the corpus while a pawn is walk-then-acting. Acceptance: the log
      shows a clean completion or a named refusal — never a half-executed effect.
- [ ] Soak: leave the world running ≥30 min across at least two corpus swaps with the npc host
      alive. Acceptance: no leaked bundles, no growing lag in the worker's compose line, population
      stable.
