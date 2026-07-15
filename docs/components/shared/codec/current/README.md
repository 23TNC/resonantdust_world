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
  = region:8 | zone:8 | tile:8 | layer:8` (realm rides `server_reference`). Terrain renders on the
  new encoding. (B-2 was **retracted** — it treated the legacy `zone_id`/`surface` as a constraint;
  geometry is realm/region/zone/tile/layer. A `macro_* → region_zone_reference` rename was also made
  here and has since been **reverted** — the plan's `macro_position_reference` /
  `micro_position_reference` naming stands.)

- **`packed.rs` `zone_id` ✅ geographic (2026-07-14, G1)** — `realm:8 | region:8 | zone:8 |
  reserved:8`; the old-game `surface` z-axis is retired. Browser-verified.
- **Cold `entity_reference` ✅ geographic (G2)** + **PACK trigger ✅ (G3)** — both verified.

- **Conformance re-cut ✅ (2026-07-14)** — closes deviations **D-1…D-5** + divergence **#11**.
  `object.rs` now tracks the design verbatim: `type_reference:u16` (the *high half* of
  `definition_reference:u32`, which composes from its two u16 halves); `pack_macro_position` /
  `pack_micro_position`; **`position_reference:u32`** = `macro:16 | micro:16` as its own type, with
  `cold_reference` sharing the layout as a distinct *type* (a position is *a location*; a
  `cold_reference` denotes *the settled object there*); `pack_tile_reference` for the `u8`
  `x:4|y:4` primitive; `pack_kind_pos_reference` + `kind_pos_ref_*` for the row's entries; and
  `pack_cold_row_reference` (`reserved:28 | macro_position:16 | type_reference:16 | layer_id:4`)
  + `cold_row_of` / `cold_row_selects` for row identity and selection.

**No open decisions, no open bugs.** 28 unit tests pin every roundtrip + field-disjointness,
including the D-3 property (a target mints the object it names, not the ground under it).
