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

## 2026-07-28 · P0 COMPLETE — the edge relays the pawn shard; a placed pawn reaches the browser

Edge: `pawn_db()` + `connector!(connect_pawn, pawn)` + `World.pawn` + insert/update/delete →
`pawn_state_frame` (same wire `State` frame, distinct bindings type) + per-zone subscription in
`ZoneSub`. One addition beyond the plan wording: the pawn zone-subscription replays the zone's
rows **on apply** — a RESTING pawn arrives in the initial snapshot and never updates, so
`on_insert` alone would miss it (the documented cold-baseline delivery guarantee applies to
resting movers too; the data_shard sub predates this and only ever carried always-moving wolves).
Verified live at the user's vantage: pawn `0x30000001` PLACEd at world tile (102,51) (macro 99,
via the event pipeline), browser at `?focus=100,50&cb=area1` → `__viewport.warm.prims` holds
exactly one prim at world px (6496,3200) = tile (102,51), and the screenshot (ss_90667kvwo)
shows it rendered (green def-0 fallback box, casting a shadow) beside the torch clearing.
Also recorded [I1](issues.md): a shard republish silently orphans connected SDK clients —
restart the sim trio after any republish (the first compose after the redeploy wrote into the
void; sim-self-heal territory).

## 2026-07-28 · P1 — CREATE: server-minted pawns, event-driven

TABLES.md first (`## pawn` section: `spawn_log` PK `reserved:16|event:32|index:16`,
`spawn_counter` module-internal; docs-check green). The pawn module gained the spawn machinery:
`spawn(worker, tic, event, index, def, position, promote)` — replay-ledger check → mint
(counter starts at `SPAWN_BASE = 0x800000`, the TOP half of the object space, so server ids
never meet the legacy client-minted band) → first `entity_state_log` row (absolute,
`dirty=false`) → promote upsert, one transaction, reusing the macro's own `TargetState` +
`entity_tables_upsert_state` (the macro expands into module scope). Verified: two identical
`spawn` calls → ONE `spawn_log` row + ONE pawn (id `0x30800000`).

codec `CREATE` framing already existed (`&[Imm, Imm]`, write set excludes the minted id, test
`create_target_is_not_in_the_write_set`) — acceptance run: 51/51 codec tests green in docker.

Worker: a SPAWN pass in the tic loop (apply's arm stays no-scratch — the minted id is not an
operand): scans each event's program with the PROMOTE-prefix state machine, calls `spawn` per
`(event, index)`, defers the tic on failure (write pattern), and merges each CREATE's spawn
zone into `complete`'s zones (a CREATE-only event previously completed with NO zones — found
at build). Bindings regenerated (st-bindings + edge). Verified live: queued
`[PROMOTE, CREATE, 458759, 6513408]` → spawn_log row keyed by the real event_reference
(`0x50000005`) → minted `0x30800001`, `entity_state` promoted with the def + position.

## 2026-07-28 · P1 — the wolf def id has ONE authority: the corpus

F5 resolved as the lean: npc fetches the world server's `/content` (the WS login URL
scheme-swapped to HTTP, `/ws` dropped — found live: reqwest rejects `ws://…/content`) and
resolves `thing_object_id("wolf")` with the shared DSL — the SAME corpus the browser renders
with. New npc deps: `resonantdust-dsl`, `reqwest` (core's no-TLS config), `serde_json`.
`things.rd`'s two phantom `KIND_WOLF` comments rewritten (the constant never existed — the
append rule + by-name resolution is the contract). Verified live: `rd build npc` green; a
20 s run against the real gateway logs "wolf def resolved from the content corpus def=7"
(wolf is 7th in `things.rd` — agreement BY CONSTRUCTION, nothing pinned). Runbook note: the
npc container needs `--network host` (the gateway resolves the edge as `localhost:8473`).

## 2026-07-28 · P2 — ACTIONS.md §Movement un-tabled

Rewritten in the PROMOTE-prefix vocabulary: chain semantics (greedy-line step behind the
pathfinding seam), queue-at-future-tic (`≥ master+3`, barrier unaffected), the cadence
(`PROMOTE_EVENT PROMOTE MOVE_TO` initial · bare continuations · `PROMOTE` final ·
resolve-on-touch free · every-N held for F8 data), the wall↔tic speculation model, and the
`tics_per_tile`-in-wall-time seam (F7). `PROMOTE_EVENT`'s palette row un-tabled. Verified:
docs-check green; no stale postfix (`PROMOTE_STATE`) examples remain.

## 2026-07-28 · P2 — `queue_at`: the continuation door

`queue` refactored to `queue_common`; new `queue_at(actions, event_tic)` rejects
`tic_before(event_tic, master+3)` (serial arithmetic) and otherwise queues at the caller's tic.
Verified live: `queue_at` at master+20 accepted, sat QUEUED (worker 0) until its tic froze, then
assigned + "composed component tic=10205"; the same call at `master` rejected with "inside the
barrier (min …)". Bindings regenerated both consumers. Side-observation recorded: the legacy
npc place/move path now lands on the PAWN shard purely via routing (its 4 wolves' rows are in
pawn `entity_state` with zero npc changes).

## 2026-07-28 · P2 — the worker chains MOVE_TO; the cadence is live

Codec seams first: `position_to_tile`/`tile_to_position` LIFTED from client/core into
`codec::object` (the worker steps on the exact math the clients decode with; core re-exports),
`tic::TIC_HZ = 6` (the one authority — all three sim binaries now DEFAULT their env from it),
and `speed::tics_per_tile(def)` authored in wall-time (2 tiles/s ÷ TIC_HZ → 3 tics/tile, unit
test). Worker `apply` MOVE_TO: greedy straight-line ONE-tile step (the pathfinding seam) +
FACING from the step stamped into `data`'s rotation bits (e/w win diagonals). A CONTINUE pass
in the tic loop queues the next hop via `queue_at(t + tics_per_tile)`: bare while distance > 1,
`PROMOTE`-prefixed when the next hop lands; a failed queue KILLS the chain by choice (deferring
would re-queue on the re-pass and DUPLICATE the chain — reasoning in the code). Client
`move_to_program` = `[PROMOTE_EVENT, PROMOTE, MOVE_TO, obj, dest]`.

Verified live (minted wolf `0x30800001`, move (102,51)→(107,51)): 5 `entity_state_log` hops at
tics 12060/63/66/69/72 — EXACT 3-tic spacing — walking micro 0x73→0x83→0x93→0xA3→0xB3 (a clean
eastward line), `data=64` (facing east) on every hop, **seed + final status 17
(PROMOTE|PROMOTED), all middle hops status 0** — and exactly ONE intent row in `event`
(zone 99). Per-hop state fan-out: none, by construction.

## 2026-07-28 · P2+P3 — intent path, tic estimate, speculation: LIVE (one flaky link recorded)

**Intent path:** `PROMOTE_EVENT` latch → settle → ONE `event` row per move (zone 99) → edge
`ServerMsg::Event` → client `Event::MoveIntent` — observed end-to-end in the browser
("[mover] intent armed dest=(107,51) tic=17724 d=4.7"). Delivery is FLAKY (absent/doubled at
times — [I3](issues.md); server side proven clean; the npc soak is the reproduction tool).

**Tic estimate:** `core::ticclock::TicEstimate` — max-implied-current-tic anchor rule, serial
across the wrap, unit-tested; both engines observe every `state`/`event` arrival and emit
sparse `Event::TicAnchor`; hosts extrapolate by `ticHz()` (wasm export of the codec
authority). Verified live BEYOND the acceptance: a 90-second-old anchor predicted a fresh
seed's tic dead-on (declined to re-anchor — already within jitter; the bar was ±1 tic
after 30 s).

**Speculation:** `MoveIntent` decode in both engines (they had DROPPED `ServerMsg::Event`);
`MoverLayer.tick()` (from `WorldScene.update`) walks the pawn fractionally along `walkGreedy`
— the server's stepping rule mirrored exactly, e/w-first facing — at `ticsPerTile` on the tic
estimate; `placeThing` is linear so fractional tiles glide. Verified live: the wolf GLIDED
(107,51)→(100,54) at exactly 2 tiles/s (51 px per 400 ms, diagonal-then-straight greedy
shape, fractional coordinates), landing e=0.01 tiles, no snap-jump. F8: corrections snap +
log error; stale-intent guards added ([I2](issues.md)): no-clock/finished/serially-older
intents ignored, `lastIntentTic` dedups. When no intent arrives (I3) the pawn falls back to
seed→final snapping — correctness rides state, exactly as designed.

## 2026-07-28 · P4 — the Brain seam + the wolves brain, supervised

**The split:** `client/npc` is now a lib (`Bot` harness — login, parameterized-radius
anchor+wait, event pump, pause tracking — plus the `Brain` trait `{async on_start, on_event,
tick}`, the `run_brain` runner, and `resolve_thing_def`) + `brains/{wildlife,wolves}` + a thin
bin dispatching by argv/`NPC_BRAIN`. `wildlife` is the legacy 4-wolf zone-0 behavior ported
verbatim (compile-verified; logic identical). Cargo paths normalized to REPO-relative and the
compose mounts collapsed to one whole-repo mount — the same shape `bin/sim` uses (F6).

**The wolves brain (v1):** anchor at `NPC_HOME` (default the dev vantage 100,50) with an
active radius covering the wander disc (+found: the original 2-tile radii would have spawned
into an unsubscribed zone); resolve the wolf def from the corpus (F5); ADOPT-FIRST (a restart
re-uses an existing minted wolf from the zone snapshot — no pack accumulation), CREATE one
otherwise; then endless single A→B `MOVE_TO`s, waiting on the authoritative arrival (or a
deadline sized from `tics_per_tile` + 5 s — a lost chain re-issues). Adoption filters on the
minted band (`>= 0x800000`) + the wolf def, and SURVIVES the cross-zone `StateGone` artifact
(recorded as [I4](issues.md) — a zone-subscription migration is not a despawn; the first build
unadopted and deadlocked on it).

**Supervised:** `bin/sim` generalized with `crate_dir()` (npc → `client/npc`) — `bin/sim run
npc NPC_BRAIN=wolves` starts container `rd-npc` on the host network, zero manual steps.
Verified live (soak left RUNNING): "minted wolf adopted 0x30800003" → 8 consecutive trips,
each arriving at EXACTLY hops×0.5 s (4 hops→2.0 s, 6→3.0 s, 2→1.0 s; the one cross-zone trip
ran +3 s — the I4 churn). Two build snags eaten en route: the docker mtime miss struck again
(stale binary ran twice — the recorded `touch` rule fixed it) and `bin/sim run` happily runs a
stale binary after a failed build (worth a guard someday).
