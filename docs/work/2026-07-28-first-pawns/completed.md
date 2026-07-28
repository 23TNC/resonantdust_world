# Completed — first-pawns

## 2026-07-28 · P0 — the stack stands (first item)

Blue screen had killed everything (edge/gateway/spacetime 1 h prior; sim crates 5 days). `rd up` +
`rd redeploy --run` restarted the daemon and the detached edge/gateway processes (both listening,
logs clean); `bin/sim build` rebuilt all three sim crates — picking up the **TIC_HZ 2→6 default**
— and `bin/sim run` stood up `rd-master`/`rd-orchestrator`/`rd-worker`. Verified:
`index.master_clock` advanced 40→59 in ~3 s (≈6.3 Hz — the new rate live), orchestrator
"subscriptions applied; grouping", worker "subscriptions applied; resolving", and the browser at
`?focus=100,50&cb=area1` renders the full lit world (screenshot ss_98651l79b), zero console
errors.

## 2026-07-28 · P0 — TABLES.md ratifies the pawn shard

Added the `…-pawn-0` row to §Databases and split the shard-class table: `pawn` =
`entity_tables!{data:u8}` for `TYPE_PAWN` (+ a note that `spawn_log` joins when `CREATE` lands),
`data_shard` re-labelled the hot CATCH-ALL (F1) with its retirement noted as a follow-on.
Verified: `bin/rd docs-check` green.

## 2026-07-28 · P0 — the pawn shard exists

`modules/pawn/` stamped (14-line lib.rs — `entity_tables!(data: u8)`, mirroring `data_shard`,
doc header naming its role + the F1 catch-all split). `rd redeploy --run` auto-discovered it
(zero tooling edits, as designed) and published `resonantdust-dev-pawn-0`. Verified via
spacetime sql: `clock` row `{0, 0}` (init ran), `entity_state` + `entity_state_log` present,
both empty.

## 2026-07-28 · P0 — pawn bindings in both consumers

Edge bindings via `rd build spacetime pawn` (`generate-bindings.sh` appended `pub mod pawn;`
itself); st-bindings via a one-shot `spacetime generate` in the build container (out-dir mounted)
+ the `pub mod pawn;` line. Verified: `bin/sim check worker` (compiles `resonantdust-st-bindings`
with the new module) and `rd build edge` both green (edge's 1 warning is pre-existing).

## 2026-07-28 · P0 — master fans the tic to the pawn shard

`PAWN_DB` env (+ startup log), a fifth shard connection, `bump(tic16)` in the fan-out, `gc` on
the same cadence as `data_shard`. Verified live: rebuilt + restarted `rd-master`; pawn
`clock.master_tic` advanced 1782→1795 over ~2 s (≈6.5 Hz observed).

## 2026-07-28 · P0 — orchestrator + worker route TYPE_PAWN to the pawn shard

Orchestrator: `hot_shard_key(entity)` (type nibble → pawn key 3 vs data 0), pawn connection +
`pawn.claim` arm; hot routing is read OFF the target (an `entity_reference` carries its type,
unlike a cold row — doc'd at the fn). Worker: `Shard::Pawn` in `shard_of`, pawn connection +
same-window `entity_state_log` subscription (+ standup wait), `base_row` dispatches per hot
shard (macro over the two identical-shape distinct-type bindings), hot WRITE split into per-shard
`TargetState` batches. Verified live: queued `[PROMOTE, PLACE, 0x30000001, 13568]` via the
event shard → orchestrator "assigned work-group tic=3006 entities=1" → worker "composed
component tic=3006 hot=1" → pawn `entity_state_log` row `{805306369, macro 0, micro 13568,
tic 3006, dirty=false, worker 0x62, status 17=PROMOTE|PROMOTED}` + promoted `entity_state`
row; data_shard has ZERO rows for that entity (the split is real).
