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

- **The `cold` row carries its full design header** ✅ (2026-07-14, divergence #11 / deviation D-3) —
  PK `cold_row_reference:u64` = `reserved:28 | macro_position:16 | type_reference:16 | layer_id:4`,
  the composite of exactly the three header fields the design gives a row, **not** an opaque
  surrogate. Columns: `macro_position:u16` (btree — the subscription key), `type_reference:u16`,
  `layer_id:u8`, `kinds:Vec<u32>` (`kind_pos_reference` entries), `version:u32`. `cold_removed` is
  **1:1** on the same key, its tombstones bare `tile_reference:u8`.

  Row selection is one rule, stated once in the codec (`cold_row_selects()`) and shared by
  `find_or_mint` and the edge's interact scan: filter `(macro_position, type_id, layer_id)` — all
  three read off the target's own `cold_reference` — then match `tile_reference` within the row.
  `subtype_id` is deliberately **not** needed to *find* a row: uniqueness is one object per
  `(type, layer, tile)`, **subtype-agnostic**, so across a zone's several subtype (biome) rows at a
  given `(type, layer)` exactly one holds the tile.

  Live-verified: two targets at the same tile differing only in `layer_reference` each mint exactly
  the object they name (tree vs the ground under it) — previously iteration-order luck.

## Reality that isn't in the design (→ cleanup)

- ~~Dead modules on disk~~ ✅ **gone (2026-07-14, T-7)** — `cold_tiles` / `cold_things` /
  `experiment` and their `redeploy.sh` fam-entries. Their *sources* had in fact been deleted back
  in div #3 (`2b2fc58`/`d47f152`); what survived was untracked build detritus and the dead fam
  arms. [`plan/cleanup.md`](../plan/cleanup.md) is closed.
- **`shared/pkg` / `shared/target`** are build outputs (not shard-specific; noted for the map).
