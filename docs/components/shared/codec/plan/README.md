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
6. **⛔ OPEN — the `cold` row re-cut (divergence #11).** The row must carry the design header
   (`macro_position_reference:16 | type_reference:16 | layer_id:4`) and key on its composite. The
   re-cut moved `layer` out of `type_reference` (correct) but never re-homed it in the row, and kept
   `zone_id:u32` instead of `macro_position` — so the key can no longer select a row by layer, and
   `find_or_mint` ignores the target's `layer_reference` outright
   ([work/…/todo.md](../../../../work/spacetime-rewrite/todo.md)).
