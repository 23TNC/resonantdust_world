# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

## Re-cut the `cold` row to its design header + composite key (divergence #11)

**2026-07-14 · A regression I introduced in the re-cut — not a "marginal compaction".** The plan's
cold row header is **`macro_position_reference:16` + `type_reference:16` + `layer_id:4`**, and the
key is their composite. I dropped two of the three: kept `zone_id:u32` in place of `macro_position`,
and — having correctly moved `layer` out of `type_reference` (it's a tile-slot, not a type property)
— **never re-homed `layer_id` in the row**. v1's `type_reference` had `layer` *inside* it, so the old
key discriminated layer; mine doesn't. (I then wrongly rationalised `macro_position` as a deferrable
2-byte compaction. It isn't: the row header is what a reader reconstructs a `position_reference`
from.)

- **`cold`** → PK `cold_row_reference : u64` = `reserved:28 | macro_position:16 | type_reference:16
  | layer_id:4`; columns `macro_position:u16` (indexed — the subscription key; realm implied by the
  shard), `type_reference`, `layer_id`, `kinds`, `version`. Drop `zone_id`.
- **`cold_removed`** → **1:1 on the same key**; tombstones shrink `u16 → u8` (a bare
  `tile_reference`) since macro/type/layer are in the key. Smaller re-sends (only that row's delta,
  not the whole zone's), no cross-filtering, can't dangle.
- **`find_or_mint` + the edge's interact scan** → select `(macro_position, type_id, layer_id)` off
  the target's `cold_reference`, then match `tile_reference`. **This is the live bug**: today the
  lookup ignores `layer_reference` and takes the first row with any entry at the tile — and the
  dense ground layer means every occupied cell matches ≥2 rows, so it's iteration-order luck.
- **worldgen** → emit `layer_id` per `ColdRow` (currently implicit 0); seed by `macro_position`.
- **edge** → subscribe `WHERE macro_position`; reconstruct the client-facing `zone_id` from the
  shard's realm + `macro_position` when relaying `ColdObjectsRow`.
- **codec** → `pack_macro_position`/accessors (rename from `pack_region_zone`, per the plan's
  `macro_position_reference` / `micro_position_reference` naming); `pack_cold_row_reference`.
- Shard republish. **Test to add:** a tile holding ground **and** a thing resolves to the one the
  target names (today a coin flip).

_(Everything else is done + verified — see [completed.md](completed.md). No blockers open.)_
