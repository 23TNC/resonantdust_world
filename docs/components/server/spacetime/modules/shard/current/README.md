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

## Geometry + cold side — DONE (2026-07-14, verified)

- **Geographic `zone_id` (G1)** ✅ — legacy `region_x:8|region_y:8|surface:8|…` retired →
  `realm:8 | region:8 | zone:8 | reserved:8`; `surface` (old-game z-axis) dropped. Browser-verified.
- **Geographic cold `entity_reference` (G2)** ✅ — `REF_COLD | server_reference | cold_reference:32`;
  find-or-mint decodes it. Verified via a live Interact.
- **`PACK` trigger (G3)** ✅ — worker `pack_idle` settles idle `REF_COLD` objects back to cold via
  `pack_settle`, restoring the exact cold entry from provenance stashed in `data0` at mint. Verified
  end-to-end (cold→hot→cold round-trip).

- **PACK is an enqueued execute op** ✅ — a periodic worker sweep appends a
  `[OBJECT(zone), ACTION(PACK)]` event per zone with at-rest packable objects; execute settles the
  zone and completes the row via the normal fenced `resolve`. Closes divergence #2.
- **Cross-shard foreign Phase-1 hold** ✅ — `stand_up_foreign` stands up the pending row + hold on a
  foreign target's home shard during the in-flight window (so its read rule defers readers and GC
  keeps the row); `resolve_foreign` releases it with the write. Holds key on `(source_shard,
  event_reference)`. Verified on a real 2-shard rig.

## Not yet implemented (one deliberate non-item — → [`work/…/todo.md`](../../../../../../work/spacetime-rewrite/todo.md))

- `cold` table `region_zone:u16` key (#4 remnant) — the table keys by the (geographic) `zone_id:u32`,
  which is correct; the `u16` narrowing is a ~2-byte wire compaction that would need the edge to
  track each shard's realm and reconstruct `zone_id` per relayed row. **Recommended to wait for
  multi-realm sharding** — no payoff in a single-realm world.

## Reality that isn't in the design (→ cleanup)

- **Dead modules still on disk:** `cold_tiles`, `cold_things` (retired div #3, superseded by this
  module's `cold` table) and `experiment` (sync-experiment leftover) — plus their `redeploy.sh`
  fam-entries — were never deleted. They are **not** part of the design; their removal is a
  [`plan/cleanup.md`](../plan/cleanup.md) item.
- **`shared/pkg` / `shared/target`** are build outputs (not shard-specific; noted for the map).
