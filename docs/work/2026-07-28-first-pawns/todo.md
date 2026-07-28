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

## P2 · `MOVE_TO` — the worker chains the hops

- [ ] ACTIONS.md FIRST: un-table §Movement — rewrite in `PROMOTE`-prefix vocabulary: hop chain,
      queue-at-future-tic (`≥ master+3`), cadence = intent once + state seed/final,
      resolve-on-touch. Acceptance: docs-check green; no stale postfix examples remain.
- [ ] event_shard: `queue` accepts a caller-supplied FUTURE `event_tic ≥ master+3` (the
      continuation door), rejecting anything nearer. Acceptance: an event queued at `master+6`
      sits unassigned until its tic freezes, then assigns + composes.
- [ ] worker: `MOVE_TO` steps ONE tile toward dest (greedy straight line — the pathfinding seam,
      a follow-on) and, when not at dest, queues `MOVE_TO obj dest` at `tic + tics_per_tile`
      (F7). Acceptance: a 5-tile move lands 5 hop rows in pawn `entity_state_log` over ~5·k tics.
- [ ] Cadence: `move_to_program` emits `[PROMOTE_EVENT, PROMOTE, MOVE_TO, obj, dest]`;
      continuations are BARE; the final hop carries `PROMOTE`. Acceptance: a 10-tile move fans
      exactly 1 `event` row + 2 `State` frames to a subscribed client — never 10.
- [ ] Verify the intent path: `PROMOTE_EVENT` latch → `settle` → `event` rows (zones from
      `complete`) → edge `ServerMsg::Event` relay. Acceptance: the ws frame carrying the
      MOVE_TO program observed at a subscribed session.

## P3 · Clients speculate — the first time tic matters

- [ ] client/core: a wall↔tic estimate — anchor `(tic, wall)` on every State/Event arrival,
      extrapolate by `TIC_HZ`, refine continuously; exposed to hosts (native + wasm).
      Acceptance: estimate within ±1 tic of a fresh state arrival after 30 s running.
- [ ] client/core: decode a relayed `event` MOVE_TO program into
      `Event::MoveIntent{entity, dest, event_tic}` (the engine currently DROPS
      `ServerMsg::Event`). Acceptance: webgl + npc both log the intent for a wolf move.
- [ ] webgl `MoverLayer`: SPECULATE — walk the pawn fractionally from its last authoritative
      position toward dest at `tics_per_tile`, driven by the tic estimate per frame; facing from
      the hop direction. Acceptance: the wolf GLIDES between tiles; landing tile matches the
      final `State`.
- [ ] Correction (F8): authoritative `State` snaps/reseeds the speculation; `ZoneClosed` drops
      it; record the observed error at each correction. Acceptance: logged error ≤1 tile in the
      quiet case, no oscillation at the destination.

## P4 · npc — harness + the wolves brain, in a container

- [ ] Split `client/npc` into a lib (`Bot` harness + `Brain` trait: on_start / on_event / tick)
      and a thin bin dispatching brains by name; port `wildlife` onto it unchanged. Acceptance:
      `rd build npc` green; existing behavior preserved.
- [ ] Wolves brain v1: spawn ONE wolf via `CREATE`, adopt the minted id from the first wolf
      `StateObject` in the anchored zone (F3), then repeatedly pick a dest tile and issue ONE
      A→B `MOVE_TO`, waiting on the destination `State` (or timeout) before the next.
      Acceptance: the wolf walks multi-tile paths from single commands.
- [ ] Run the npc as a supervised container `rd-npc-<brain>` (F6 — lean: extend `bin/sim`) with
      brain + gateway env. Acceptance: one command starts it; the wolf spawns and wanders with
      zero manual steps.

## P5 · Seen end-to-end + wrap

- [ ] Browser at the wolf's zone: wolf renders with WOLF art (def real at last — today it falls
      back to the tree def), glides A→B, e/s/n facing follows direction. Acceptance: screenshot
      recorded; console clean.
- [ ] Bandwidth proof: count ws frames at a subscribed client across one multi-tile move —
      1 intent + seed/final `State` only; npc log shows the queue→intent→final echo with the
      tic delta recorded. Acceptance: counts + delta in completed.md.
- [ ] Rewrite `client/core/intent/sync.md` in tic/speculation vocabulary (the README's sync
      philosophy; `valid_at` remnants deleted). Acceptance: docs-check green; no `valid_at` in
      live docs.
- [ ] Wrap: component doc for the pawn module + the npc brain seam; fix the stale
      `components/server/spacetime/README.md` module index; completed.md entries; work-index
      row → done; memory updated. Acceptance: docs-check green; `rd work brief` clean.
