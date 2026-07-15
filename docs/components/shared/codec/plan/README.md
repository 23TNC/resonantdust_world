# Plan — `shared/codec` (current → design)

_Last updated: 2026-07-14._

1. **`refs.rs` ✅ DONE (2026-07-14)** — `entity_reference = reserved:10 | reference_id:6 |
   server_reference:16 | object_reference:32`; geographic `server_reference = realm:8 | server_id:8`;
   the `object_reference` union via `reference_id`; dead functional machinery removed. Carried the
   shard's `hot_reference` (#5), `server_reference` (#10) re-keys + `event_reference:u32`. Landed +
   browser-verified (`work/spacetime-rewrite/completed.md`).
2. **`object.rs` ✅ DONE (2026-07-14)** — the **definition / position / data** split:
   `definition_reference:u32` (drop `subkind`, `kind` 12b), `type_reference`/`kind_reference`
   halves, the cold entry `kind_reference:16 | tile:8 | data:8`, `data:8` decode, geographic
   `cold_reference = region:8 | zone:8 | tile:8 | layer:8`, `macro_position_reference` /
   `micro_position_reference` (the two u16 halves of a `position_reference`).
   Landed + browser-verified (terrain renders on the new encoding). B-2 retracted (it wrongly
   treated the legacy `zone_id`/`surface` as a constraint).
3. **`packed.rs` `zone_id` ✅ DONE (2026-07-14, G1)** — repartitioned to geographic
   `realm:8 | region:8 | zone:8 | reserved:8`; `surface` (old-game z-axis) retired. Browser-verified.
4. **Cold `entity_reference` ✅ DONE (2026-07-14, G2)** — `REF_COLD | server_reference |
   cold_reference:32` (geographic); the interim world-global form removed. Verified via Interact.
5. **PACK trigger ✅ DONE (2026-07-14, G3)** — worker settles idle `REF_COLD` objects back to cold
   (provenance in `data0`); verified round-trip.
6. **Cold row re-cut ✅ DONE (2026-07-14, divergence #11 / deviation D-3)** — the row carries the
   design header (`macro_position_reference:16 | type_reference:16 | layer_id:4`) and keys on its
   composite `cold_row_reference:u64`. Selection is `cold_row_selects()`: filter
   `(macro_position, type_id, layer_id)` off the target's `cold_reference`, then match
   `tile_reference`. Live-verified — a target mints the object it names, not the ground under it.
7. **Conformance re-cut ✅ DONE (2026-07-14)** — closes deviations **D-1…D-5**: `type_reference:u16`,
   `macro_position`/`micro_position`, `position_reference:u32` as its own type, `tile_reference:u8`,
   `kind_pos_reference` + `kind_pos_ref_*`. See
   [work/…/deviations.md](../../../../work/spacetime-rewrite/deviations.md).

**The plan is fully landed — no open items.**
