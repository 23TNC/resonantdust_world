# Current — `shared/codec` (where we are)

_Last updated: 2026-07-14._

The reference layouts + object model are implemented in `shared/codec/src` (`event_word.rs`,
`refs.rs`, `object.rs`) and consumed across the stack. Detail + runbook:
[`object-model-status.md`](object-model-status.md).

**Re-cut status (2026-07-14): `refs.rs` DONE, `object.rs` deferred.**

- **`refs.rs` ✅ re-cut + browser-verified** — geographic `server_reference = realm_id:8 |
  server_id:8` (#10); `entity_reference = reserved:10 | reference_id:6 | server_reference:16 |
  object_reference:32` (#5, hot = `REF_HOT | server_reference | hot_reference:32`); the dead
  functional machinery removed. `event_reference` is now `u32` on the shard side.
  (`work/spacetime-rewrite/completed.md`.)
- **`object.rs` ✅ re-cut + browser-verified** — the **definition / position / data** split:
  `definition_reference:u32` (drop `subkind`, `kind` 12b), `type_reference`/`kind_reference` halves,
  the cold entry `kind_reference:16 | tile:8 | data:8`, `data:8` decode, geographic `cold_reference
  = region:8 | zone:8 | tile:8 | layer:8` (realm rides `server_reference`), `region_zone_reference`
  naming (not `macro_*`). Terrain renders on the new encoding. (B-2 was **retracted** — it treated
  the legacy `zone_id`/`surface` as a constraint; geometry is realm/region/zone/tile/layer.)

- **`packed.rs` `zone_id` ✅ geographic (2026-07-14, G1)** — `realm:8 | region:8 | zone:8 |
  reserved:8`; the old-game `surface` z-axis is retired. Browser-verified.
- **Cold `entity_reference` ✅ geographic (G2)** + **PACK trigger ✅ (G3)** — both verified.

**No open decisions; the re-cut is complete.** Only a marginal `cold`-table `region_zone:u16`
key-width compaction remains ([work/…/todo.md](../../../../work/spacetime-rewrite/todo.md)).
