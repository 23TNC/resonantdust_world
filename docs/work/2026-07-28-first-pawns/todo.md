# Todo — first-pawns

_Items tick in place; the box is the move. Design: [`README`](README.md) ·
[`TABLES.md`](../../TABLES.md) · [`ACTIONS.md`](../../ACTIONS.md) ·
[`world-storage`](../../intent/world-storage/README.md)._

---

## P0 · Stack standup + the `pawn` shard (server spine)

- [ ] Stand the sim stack up: `rd up`/`redeploy`, then `bin/sim run` master + orchestrator +
      worker. Acceptance: `index.master_clock` tic advancing (spacetime sql), edge + gateway
      logs clean, browser still renders terrain.
- [ ] TABLES.md FIRST: add the `…-pawn-0` DB row + a `pawn` row in the shard-class table
      (`entity_tables!{data:u8}`), noting `data_shard` stays the catch-all (F1). Acceptance:
      `bin/rd docs-check` green.
- [ ] Stamp `server/spacetime/server/modules/pawn/` (mirror `data_shard`'s 12-line
      `entity_tables!(data: u8)` lib.rs) and publish. Acceptance: `resonantdust-dev-pawn-0`
      live; `entity_state`/`entity_state_log`/`clock` present via spacetime sql.
- [ ] Generate bindings: `server/st-bindings/src/pawn/` + `pub mod pawn;`, and edge bindings via
      `generate-bindings.sh pawn`. Acceptance: `st-bindings` + `edge` build green.
- [ ] master: `PAWN_DB` env + connection + tic `bump` fan-out + periodic `gc`. Acceptance: the
      pawn shard's `clock.master_tic` advances while `rd-master` runs (spacetime sql).
- [ ] orchestrator: split hot routing — `TYPE_PAWN` targets claim on the pawn shard
      (`pawn.claim`), everything else keeps `data.claim`. Acceptance: a queued
      `[PROMOTE, PLACE, 0x30…, pos]` gets its claim stamped in pawn `entity_state_log`.
- [ ] worker: `Shard::Pawn` in `shard_of()` + pawn `base_row` + pawn `write`. Acceptance: that
      queued PLACE composes — pawn `entity_state` row at the position, log `dirty=false`,
      status PROMOTED.
- [ ] edge: `pawn_db()` config + `connector!(connect_pawn, pawn)` + `World` slot + `entity_state`
      insert/update/delete → `state_frame` relay + zone-subscribe SQL on pawn `entity_state`.
      Acceptance: a browser session at the zone receives the placed pawn's `State` frame.

## P1 · `CREATE` — server-minted pawns

- [ ] TABLES.md FIRST: `spawn_log` on the pawn shard — `(event_reference, index) → minted
      entity_reference` — plus the mint counter (`ACTIONS.md` §CREATE). Acceptance:
      `bin/rd docs-check` green.
- [ ] pawn module: an idempotent `spawn` reducer — mint + `entity_state_log` write + `spawn_log`
      record in ONE transaction; a replay returns the recorded id, no re-mint (F4). Acceptance:
      calling twice with one `(event, index)` leaves exactly one pawn.
- [ ] codec: `CREATE` framing — arity 2, the minted target is NOT an operand (conflict-free
      singleton at grouping); write/read-set tests. Acceptance: codec tests green (in-docker
      cargo test).
- [ ] worker: the `CREATE` arm calls `spawn` with def + position + the pending PROMOTE bit.
      Acceptance: queue `[PROMOTE, CREATE, def_wolf, pos]` → new pawn `entity_state` with
      `definition_reference` = wolf, plus its `spawn_log` row.
- [ ] Resolve the wolf def id for npc (F5) and fix `content/data/things.rd`'s phantom
      `KIND_WOLF` comment. Acceptance: npc + content agree on one authoritative value;
      docs-check green.

## P2 · npc — harness + the first brain, in a container

- [ ] Split `client/npc` into a lib (`Bot` harness + `Brain` trait: on_start / on_event / tick)
      and a thin bin dispatching brains by name; port `wildlife` onto it unchanged. Acceptance:
      `rd build npc` green; existing wolves behavior preserved.
- [ ] Wolf brain v1: spawn ONE wolf via `CREATE`, adopt the minted id from the first wolf
      `StateObject` in the anchored zone (F3), then wander by ADJACENT-tile `MOVE_TO` hops each
      `MOVE_MS` (F2). Acceptance: pawn `entity_state` steps tile-by-tile, never jumps.
- [ ] Run the npc as a supervised container `rd-npc-<brain>` (F6 — lean: extend `bin/sim`) with
      brain + gateway env. Acceptance: one command starts it; the wolf spawns and wanders with
      zero manual steps.

## P3 · Seen end-to-end + wrap

- [ ] Browser at the wolf's zone: the wolf renders with WOLF art (def real at last — today it
      falls back to the tree def) and e/s/n facing follows movement direction. Acceptance:
      screenshot recorded; console clean.
- [ ] npc observes its own pawn round-trip: log each wolf `StateObject` against the issued hop.
      Acceptance: npc log shows a queue→state echo for every hop; the tic delta (expect ~3,
      `TIC_GAP`) recorded in completed.md.
- [ ] Wrap: component doc for the pawn module + the npc brain seam; fix the stale
      `components/server/spacetime/README.md` module index; completed.md entries; work-index
      row → done; memory updated. Acceptance: docs-check green; `rd work brief` clean.
