# Completed — spacetime rewrite

Executes: the `shard` component (docs migrated to
`docs/components/server/spacetime/modules/shard/` — design/intent/current/plan). Done + verified work, chronological (oldest first).
Each row: date · item · verification · commit. Rich detail for the full-design mechanisms
follows the log. (Was `docs/spacetime-implementation/completion-log.md`, now retired.)

## Log

- **2026-07-13** · **S0–S7 core hot path** — codec word frame, DSL interpreter, pipeline module
  (event_log/state/holder/cold/meta + lifecycle reducers), worker two-phase loop, master
  drop→bump, edge word-stream producers, await-gate/abort, terrain `seed_cold_row`, cold→hot
  `find-or-mint`, hot→cold `pack_settle`, legacy module retirement (div #3). Live-verified with
  real binaries. · commits …d47f152
- **2026-07-13** · **#1 Deterministic composition + tic-gated execution** — worker resolves only a
  sealed tic (master ≥ event_tic); each target folds ALL its events by `event_reference`. Live:
  two same-tic moves gated then folded 34→51. · 830d6ba
- **2026-07-13** · **#2 RUNNING-row recovery** — `claim` re-claims a RUNNING row on lease expiry.
  Live: takeover refused while lease held, succeeded after expiry. · 8571018
- **2026-07-14** · **#3 Actor-reading verbs (DAMAGE) + read-rule defer** — `Reads` trait; OBJECT
  operand (mint_server,entity_id, D3 48-bit identity) → actor hp at ≤T−1; unsettled actor defers.
  Live: hp 50 → victim 100→75; unit dead-actor void. · 0bdd2f3
- **2026-07-14** · **#4 Convergent cross-shard writes** — foreign targets apply via idempotent
  `resolve_foreign` (dedup by (source_shard, event_reference) in `applied_foreign`); row completes
  once all shards ack; re-drive dedups → exactly-once. Live on TWO real shard DBs. · 1843276
- **2026-07-14** · **#5 Forward SKIP branch (`?:`) + FAIL + multi-action vectors** — index-driven
  `run`; skip-if-false + `encode_if`; FAIL halt; multi-verb programs. 32 unit tests. · 2749902
- **2026-07-14** · **fix: home_shard routes unminted targets locally** — `mint_server == 0`
  (SERVER_REF_NONE) is unminted ⇒ resolve local, not foreign shard 0. Regression from #4, caught
  by the first real end-to-end run (wolves stuck at kind 0). · d14cf1b
- **2026-07-14** · **#6 Integration — self-driving dev stack + in-browser render** — `rd run`
  stands up worker+master (issue 004 B, light form); full stack (npc→gateway→edge→shard→worker→
  state) renders a moving wolf pack in pixijs. Client already new-schema (no porting). · e78db22
- **2026-07-14** · **`/pause` + `/unpause` simulation freeze** — `tic_meta.paused` + `set_paused`;
  `bump` no-ops paused; master skips paused shards; edge relays `Paused` to all subscribers;
  client/core + wasm + npc + pixijs chat wired. Live: browser `/pause` froze the tic, npc stopped,
  `/unpause` resumed. · b5337d8
- **2026-07-14** · **Hot/identity/event re-cut (codec reference model — hot side)** — the settled,
  geometry-free half of the codec re-cut, landed + browser-verified end-to-end:
  - **`refs.rs` → reference model** — `server_reference = realm_id:8 | server_id:8` (geographic,
    **#10**); `entity_reference = reserved:10 | reference_id:6 | server_reference:16 |
    object_reference:32` (**#5**), a hot object = `REF_HOT | server_reference | hot_reference:32`;
    dropped the dead functional machinery (`server_type`/`action_reference`/`zone_reference`/
    `entity_type_data_type` — 0 live uses). Cold/positional kept as a `REF_COLD` variant carrying
    the world-global `zone_id:32|location:8|layer:8` (interim, pre-**B-2**).
  - **`event_reference : u64 → u32`** — `event_log` PK + `holder`/`open_rows` cols + every reducer
    sig; `vm::await_gate`/`encode_await` now u32 (closes the "u32↔u64 reconciliation of S7" TODO).
  - **`entity_key` kept `u64`** holding the new-layout `entity_reference` (server-qualified) — `#5`
    realized without narrowing the PK ([forks.md](forks.md)).
  - Propagated through the whole stack + regenerated ST bindings (edge/worker/master): pipeline
    macro (`mint_hot`), worker (`home_shard`/actor-reads/find-or-mint), edge (`pack_hot_entity`/
    `pack_cold_entity`), tick vm, client/core (obj_type = `reference_id`), npc, wasm, pixijs
    (`MoverLayer` filters `REF_HOT`). Terrain (cold path, `object.rs` v1) untouched + still renders.
  - **Verified:** all crates compile; shard republished (fresh schema); worker+master subscribe on
    the u32 schema; npc spawns 4 wolves; `state` shows `entity_key = 0x0001_0000_0000_000N`
    (`REF_HOT`+hot_reference N), `kind=7`, locations advancing per tic; browser renders the 4
    directional wolves moving through the forest. · _(uncommitted — commit on request)_
- **2026-07-14** · **Object-model re-cut (`object.rs` → reference model — cold side)** — after the
  user corrected the B-2 confusion (surface is legacy; geometry is **realm · region · zone · tile ·
  layer**), re-cut `object.rs` to the honest **definition / position / data** split + propagated:
  - `definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4` — **dropped
    `subkind`**, widened `kind` 10→12b; `type_reference:u16` / `kind_reference:u16` halves.
  - Cold entry `kind_pos_reference : u32 = kind_reference:16 | tile:8 | data:8`; `data:8 =
    sub_position:3 | rotation:2 | aux:3` (replaces the old `rotation:2|count:4`).
  - Geographic `cold_reference : u32 = region:8 | zone:8 | tile:8 | layer_reference:8` (realm omitted
    — rides `server_reference`); `region_zone_reference` naming (not `macro_*`); the `u8` nibble
    primitive at four scales per [spatial-references.md](../../components/shared/codec/design/references/spatial-references.md).
  - Propagated: worldgen (`pack_cold_entry`), worker/edge/wasm decode unchanged (entry readers kept
    their names). Kept the cold *table* on its `zone_id:u32` key for now (the legacy-`zone_id`
    retirement is the remaining cleanup below — NOT a blocker; B-2 retracted).
  - **Verified:** codec unit tests pass; shard republished (fresh); edge reseeded cold in the new
    entry format; browser renders the forest (conifers + bushes + ground) correctly **and** the
    wolves — cold (new object encoding) + hot (identity re-cut) both live. · 11df80b
- **2026-07-14** · **G1 — retire the legacy `zone_id` → geographic realm/region/zone (drop
  surface)** — repartition `packed.rs`'s world-global routing key from `region_x:8 | region_y:8 |
  surface:8 | zone_x:4 | zone_y:4` (old-game geometry) to **`realm:8 | region:8 | zone:8 |
  reserved:8`** (each level `hi:4|lo:4`; `surface`/z-axis retired). `region_of` → realm|region;
  `zone_region_zone` (the region:8|zone:8 cold key); realm accessors; `zone_and_location` /
  `global_tile` nest realm ⊃ region ⊃ zone ⊃ tile. Propagated: biome, wasm (`zoneSurface`→
  `zoneRealm`, dropped the `surface` param from the whole `set_anchor` chain wasm→client/core→
  pixijs→npc), edge index, tests. **Browser-verified** (terrain + wolves render unchanged — zone 0
  is invariant across the repartition). · a470495
- **2026-07-14** · **G2 — cold `entity_reference` → geographic `cold_reference`** — replace the
  interim world-global cold form with `REF_COLD | server_reference | cold_reference:32` (region:8 |
  zone:8 | tile:8 | layer:8); realm rides `server_reference` (a shard is realm-scoped).
  `entity_ref_zone_id` reconstructs realm|region|zone; edge `Interact` builds a `cold_reference`.
  **Browser+DB-verified**: right-click a tree → find-or-mint decodes the geographic `cold_reference`
  and promotes it hot (state +1, `cold_removed` +1, worker logs the decoded target). Cold *table*
  keeps its geographic `zone_id:u32` key (the `region_zone:u16` narrowing is a marginal
  wire-compaction needing multi-realm edge plumbing — see todo). · 34420c7
- **2026-07-14** · **G3 — PACK trigger (hot→cold settle)** — the last representation item, worker-
  only. `find_or_mint` stashes cold provenance (`type_reference:32 | cold_entry:32`) in `data0`; a
  `pack_idle` sweep settles idle `REF_COLD` objects (no holders, idle > `PACK_IDLE_TICS`) back via
  the existing `pack_settle` reducer, recomposing the exact cold row + entry. Pawns (`REF_HOT`)
  never match → never pack (the design criterion). **Verified end-to-end**: drive a cold-target
  event for a tree → hot (state +1, tombstoned) → after the idle window → `hot → cold (PACK settle)`
  (state −1, tombstone cleared, entry restored to the cold row verbatim) — a faithful
  cold→hot→cold round-trip. · 7454e39
- **2026-07-14** · **PACK promoted to an enqueued execute op — closes divergence #2** — the doc
  audit caught that the G3 sweep called `pack_settle` **directly**, i.e. the very "out-of-band
  reducer racing the tick loop" the design rejects. Reworked: `vm::encode_pack`/`vm::pack_target`
  (a `[OBJECT(zone), ACTION(PACK)]` program — `ACTION_PACK` was previously unused); the worker's
  periodic sweep now **appends a PACK event** per zone holding at-rest packable objects (one in
  flight per zone), and **execute** recognises the program, settles the zone, and completes the row
  via the normal fenced `resolve`. Targets the **zone/cold row**, never an object (an
  object-targeted PACK would hold the object it means to pack → the refcount gate would always
  refuse); the row carries no `targets`, so enqueue holds nothing. **Verified:** `find-or-mint` →
  `enqueued PACK zone=0` → `hot → cold (PACK settle)` → `PACK resolved ev=7391 packed=1`, exactly
  one enqueue, wolves unaffected. Mint (enqueue-absorbed) + PACK (execute op) both now match the
  design → **divergence #2 CLOSED**. · _(this commit)_

---

## Detail — the full-design mechanisms (matching the shard `design/`+`intent/` in full)

### #1 Deterministic composition + tic-gated execution
The worker's `execute` resolves a row only when `master_tic ≥ event_tic` (the tic is *sealed*, no
further event can target it), and computes each target's value as `base@(tic−1)` folded over
**all** applicable events targeting that `(entity, tic)` in **`event_reference` order** — not
arrival order, not last-writer-wins. Idempotent + order-free. Realizes the designed `+3` latency.

### #2 RUNNING-row recovery
`claim` re-claims a `RUNNING` row when the fence is free (unowned, ours, or **lease-expired**) —
a worker that died mid-execute is taken over (status stays `RUNNING`, reassigned).

### #3 Actor-reading verbs (DAMAGE) + read-rule defer
`Reads` trait: an `OBJECT` operand `(mint_server, entity_id)` (the 48-bit identity that fits the
word — no re-key, per D3) resolves to the actor's hp at `≤ T−1`. `DAMAGE` = `[OBJECT(actor),
LITERAL(amount), ACTION(DAMAGE)]`: a **live** actor's blow lands; dead/absent/unsettled reads 0
and voids. Worker enforces the **read rule** (`resolved_through`): a row whose actor isn't settled
through `≤ T−1` defers, and such events aren't folded (`applicable`). The full `hot_reference`
re-key (#5) is *not* needed for actor-reads after all.

### #4 Convergent cross-shard writes
The worker reads each target's base from its **home shard**, partitions targets by home shard,
applies foreign groups first via that shard's `resolve_foreign(source_shard, event_reference, tic,
results)` — **idempotent** by `(source_shard, event_reference)` recorded in `applied_foreign` —
and only once **every** foreign shard acks does it `resolve` the local group (which completes the
row + releases holds). Crash before the final resolve → re-drive re-applies (foreign dedups) then
completes → **exactly-once, eventually-consistent**, never 2PC. *Follow-up (see todo):* a foreign
target gets no Phase-1 pending row/holder on its home shard during the in-flight window.

### #5 Full control flow — forward `SKIP` branch + multi-action vectors
`run` is index-driven: a **forward-only `SKIP`** pops a `LITERAL` count + boolean, jumps forward
when the boolean is false (`skip-if-false`); `FAIL` halts (abort branch). `encode_if(cond, then,
else)` composes `cond ? then : else`. No backward jumps ⇒ bounded execution. A program may carry
multiple action words (multi-verb row), applied left-to-right. Condition may come from an `OBJECT`
actor read. Rides the same `vm::run` the worker drives live.

### #6 Integration — self-driving dev stack + in-browser render
(a) **worker/master standup** via `rd run <up|worker|master|…>` (issue 004 option B, light form:
one persistent `rd-run-<env>` container, `--network host`, native SDK binaries vs
`resonantdust-<env>-zone-0`). (b) **browser client** — `rd redeploy --run` + `rd up gateway` +
`rd index seed` bring up edge/gateway/index; `npc` drives a wolf pack; pixijs renders it. Verified:
4 wolves spawn kind 7 and move in `state` AND render as moving pawns in the browser.
