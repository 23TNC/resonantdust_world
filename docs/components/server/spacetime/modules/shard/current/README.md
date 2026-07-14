# Current — `shard` (where we are now)

_A snapshot of implemented-vs-not. Authoritative history:
[`work/spacetime-rewrite/completed.md`](../../../../../../work/spacetime-rewrite/completed.md);
this is a convenience summary. Last updated: 2026-07-14._

See also [`divergences.md`](divergences.md) — the design-vs-code gap list.

## Implemented + live-verified

- **Core hot path (S0–S7):** the word codec, DSL interpreter, the pipeline module (event_log /
  state / state_log / holder / cold / meta + lifecycle reducers), the two-phase worker loop, the
  master drop→bump, edge word-stream producers, await-gate/abort, terrain `seed_cold_row`,
  cold→hot `find-or-mint`, hot→cold `pack_settle`.
- **Full behavioral design:** deterministic composition + tic-gated execution; RUNNING-row
  recovery; actor-reading `DAMAGE` + the read-rule defer; convergent cross-shard writes; forward
  `SKIP` branch + `FAIL` + multi-action vectors.
- **Debug:** `/pause` — `tic_meta.paused` + `set_paused`; `bump` no-ops while paused.
- **Integration:** the full stack self-drives (`rd run`) and renders in the browser.

## Representation re-keys — hot/identity/event side DONE (2026-07-14, browser-verified)

- **`hot_reference` re-key (#5)** ✅ — `state`/`state_log` key `entity_key:u64` now holds the
  new-layout `entity_reference` (`reserved:10 | reference_id:6 | server_reference:16 |
  object_reference:32`); a hot object is `REF_HOT | server_reference | hot_reference:32`. Column
  kept `u64` (server-qualified) — see [`work/…/forks.md`](../../../../../../work/spacetime-rewrite/forks.md).
- **Geographic `server_reference` (#10)** ✅ — `realm_id:8 | server_id:8`; dead functional
  `server_type`/`action_reference` machinery removed.
- **`event_reference` → `u32`** ✅ — `event_log` PK + `holder`/`open_rows` + every reducer sig +
  `vm::await_gate`/`encode_await`.

## Not yet implemented (→ [`plan/`](../plan/) / blockers)

- **Cold-side re-cut — blocked on world geometry** ([`work/…/blockers.md`](../../../../../../work/spacetime-rewrite/blockers.md)
  B-2): `region_zone` cold key (#4) + `object.rs` compressed `position_reference`/`cold_reference`
  addressing + the cold row's `macro` header. The compressed `region:8|zone:8` conflicts with the
  live world-global `zone_id:u32`. Cold keeps its **working** `zone_id:u32` key meanwhile.
- `PACK` trigger — rides the new cold `data:8` decode → deferred with the cold-side re-cut.
- Cross-shard foreign Phase-1 hold (in-flight read-rule/GC visibility on the home shard).

## Reality that isn't in the design (→ cleanup)

- **Dead modules still on disk:** `cold_tiles`, `cold_things` (retired div #3, superseded by this
  module's `cold` table) and `experiment` (sync-experiment leftover) — plus their `redeploy.sh`
  fam-entries — were never deleted. They are **not** part of the design; their removal is a
  [`plan/cleanup.md`](../plan/cleanup.md) item.
- **`shared/pkg` / `shared/target`** are build outputs (not shard-specific; noted for the map).
