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

## Not yet implemented (→ [`plan/`](../plan/), most blocked → blockers)

- The representation re-keys: `hot_reference` u32 (D3), `region_zone` cold key (#4), geographic
  `server_reference` (#10) — **blocked** on the object-model decisions.
- `event_reference` → `u32` — design-decided (see [`../intent/event-reference.md`](../intent/event-reference.md)),
  not yet applied; the PK is still `u64`.
- `PACK` trigger (who calls `pack_settle`; needs a hot→cold type map).
- Cross-shard foreign Phase-1 hold (in-flight read-rule/GC visibility on the home shard).

## Reality that isn't in the design (→ cleanup)

- **Dead modules still on disk:** `cold_tiles`, `cold_things` (retired div #3, superseded by this
  module's `cold` table) and `experiment` (sync-experiment leftover) — plus their `redeploy.sh`
  fam-entries — were never deleted. They are **not** part of the design; their removal is a
  [`plan/cleanup.md`](../plan/cleanup.md) item.
- **`shared/pkg` / `shared/target`** are build outputs (not shard-specific; noted for the map).
