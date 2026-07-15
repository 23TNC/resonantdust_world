# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

All of the below closes [deviations.md](deviations.md) **D-1…D-5** + divergence **#11** — i.e. it
conforms the code back to the plan. Land as one re-cut (they touch the same call sites); validate
each shape against [reference-model.md](../../components/shared/codec/design/reference-model.md) as
you go, and **log any new deviation you stumble on** in [deviations.md](deviations.md).

## T-1 · codec: conform `object.rs` to the plan (D-1, D-2, D-4, D-5)

- **D-1** `pack_type_reference -> u16`; `type_ref_*` take `u16`. `definition_reference` composes
  from its two u16 halves (`type_reference:16 | kind_reference:16`) rather than re-deriving from 4
  ids.
- **D-2** `pack_region_zone` → **`pack_macro_position`** (+ `macro_position_region` /
  `macro_position_zone`, `cold_ref_macro_position`).
- **D-4** add **`position_reference : u32`** = `region:8 | zone:8 | tile:8 | layer_reference:8` as
  its own type sharing the layout with `cold_reference` (per the plan: two types, one layout); add
  `micro_position_reference:16` = `tile_reference:8 | layer_reference:8`. Rename the v1 `u8`
  primitive off the `position_reference` name → the plan's **`tile_reference`**
  (`pack_tile_reference(x, y) -> u8`), since realm/region/zone/tile all share that one packer.
- **D-5** `pack_cold_entry` → **`pack_kind_pos_reference`**; readers `kind_ref_*` →
  `kind_pos_ref_*` (they read an entry, not a `kind_reference`).
- **new** `pack_cold_row_reference(macro_position, type_reference, layer_id) -> u64` +
  accessors — `reserved:28 | macro_position:16 | type_reference:16 | layer_id:4`.
- Unit-test every roundtrip + field-disjointness, as the existing tests do.

## T-2 · shard: re-cut the `cold` + `cold_removed` rows (D-3 / divergence #11)

- `cold` → PK `cold_row_reference:u64`; columns `macro_position:u16` (btree — the subscription key),
  `type_reference:u16`, `layer_id:u8`, `kinds:Vec<u32>`, `version:u32`. **Drop `zone_id`.**
- `cold_removed` → **1:1 on the same key**; `macro_position:u16` (btree), `removed:Vec<u8>`
  (`tile_reference` tombstones), `version:u32`.
- `seed_cold_row(macro_position, type_reference, layer_id, kinds)`; `mint_cold` + `pack_settle`
  updated to the new key/tombstone shape.
- Shard republish (schema change) + regenerate bindings for edge/worker/master.

## T-3 · worker + edge: select the row the target actually names (D-3 live bug)

- `find_or_mint`: filter `(macro_position, type_id, layer_id)` — all three read off the target's
  `cold_reference` — then match the entry whose `tile_reference` equals the target's. **Today it
  ignores `layer_reference` and takes the first row with any entry at the tile.**
- The edge's `handle_interact` scan: same selection.
- `pack_zone` / provenance: recompose against the new key.
- Edge subscribes `WHERE macro_position = …`; reconstructs the client-facing `zone_id` from the
  shard's realm + `macro_position` when relaying `ColdObjectsRow`.

## T-4 · worldgen + wire + client (D-1 narrowing, D-3)

- worldgen: emit `layer_id` per `ColdRow` (currently implicit 0); key by `macro_position`.
- `ColdObjectsRow.type_reference` → `u16`; wasm `zoneColdPrims` / `objectTypeId` → `u16`;
  `client/core` `Event::ColdObjects` likewise.

## T-5 · verify

- **Regression test for D-3**: a tile holding **both** ground and a thing resolves to the one the
  target names (today a coin flip — the ground row can win).
- Browser: terrain + wolves still render; Interact → find-or-mint → PACK settle round-trip.

_(Everything else is done + verified — see [completed.md](completed.md). No blockers open.)_
