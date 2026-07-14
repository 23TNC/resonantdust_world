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
   `cold_reference = region:8 | zone:8 | tile:8 | layer:8`, `region_zone_reference` (not `macro_*`).
   Landed + browser-verified (terrain renders on the new encoding). B-2 retracted (it wrongly
   treated the legacy `zone_id`/`surface` as a constraint).
3. **Remaining — retire the legacy `zone_id`** (normal cleanup, not blocked): drop `surface`,
   repartition to realm/region/zone, move `cold` to a `region_zone:u16` key, and switch the cold
   `entity_reference` to the geographic `cold_reference` form (`pack_cold_reference` exists). See
   [work/…/todo.md](../../../../work/spacetime-rewrite/todo.md).
