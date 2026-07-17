# Todo — cold-rework

_Planned, not started. Dependency-ordered by phase (P1 gates the shape; P2/P3 stand up the router +
overlay read-path; P4 is the mutation build, gated on [`blockers.md`](blockers.md)). Move an item to
`remaining.md` on start, `completed.md` when it lands. VARIABLES/TABLES already carry the target shape._

---

## P1 · Subtype fix — restore biome, `type_id` → shard

The defect: cold drops `subtype_id` (biome). Rework the row to `macro:16 | subtype:12 | layer_id:4`.

- **modules** `tile`/`thing`: `cold_tile`/`cold_thing` columns → `cold_row_reference` (new pack),
  `macro_position_reference` (idx), `subtype_id` (u16, idx), `layer_id` (u8); drop `layer_reference`.
  `seed(macro, subtype_id, layer_id, payload)` packs the new key. Regenerate bindings; redeploy.
- **codec** `object`: `pack_cold_row_reference(macro, subtype_id, layer_id)` + accessors
  (`cold_row_subtype`, `cold_row_layer_id`); keep `type_id` out (shard-sourced). Update the 47-test
  suite.
- **worldgen** `zone_cold`: use `gen.biome → biome_subtype_id`; **group a zone's cells by subtype** and
  return one `(subtype, layer_id, payload)` group per biome present (dense tiles: 256 with out-of-biome
  cells `0`; sparse things: only that biome's entries).
- **edge** `seed_zone`: loop the groups, one `seed` call each. `ColdTile`/`ColdThing` frames carry
  `subtype_id` + `layer_id` (not `layer_reference`); relay one frame per biome-row.
- **wasm/client** `zone_cold_prims`: take `subtype_id` + `layer_id`; reconstruct `type_reference =
  shard.type_id | subtype` (shard = which frame) and `layer_reference = shard.type_id | layer_id`;
  full `definition_reference`. pixijs keys cold rows by `(macro, subtype, layer)`.

**Done when:** a cold object round-trips its full `definition_reference` (biome included) and
`position_reference`; all four build gates green; **browser: biomes still flow across zones, multi-biome
zones render every cell once** (no gaps/overlaps at biome seams within a zone).

---

## P2 · Region router — `index.cold_shards`

Stand up the position → cold-shard indirection now, one default row.

- **module** `index`: add `cold_shards` (`route_reference` PK, `type_id`/`region_reference` idx,
  `shard_reference`, `url`, `db_name`) + `set_cold_shard`/`remove_cold_shard` reducers; `init` seeds the
  default route(s) (`region * → shard 0`, per family) — **encoding of "region \*" is [blocker BK3].**
- **edge**: subscribe `cold_shards`; resolve a cold sub by `position → macro → region_reference →
  (type, region)` → endpoint; connect there (today always the single default). Replace the hardcoded
  `tile_db()`/`thing_db()` with the router result.

**Done when:** the edge reaches cold via the router (not hardcoded names); adding a second row would
route a region elsewhere with no code change; login → zone render still works.

---

## P3 · Overlay read-path — `state`/`state_log` on cold

Give cold the hot pair and composite it, before anything writes it.

- **modules** `tile`/`thing`: add `clock` mirror + `state_log` + `state` (identical to `data_shard` —
  see [blocker BK1] on how the machinery is shared vs duplicated). No writes yet beyond `init`.
- **edge**: also subscribe a zone's cold `state` (`WHERE macro_position_reference = <zone>`) and relay
  it; a `ColdState` frame carries the per-entity override.
- **client**: composite **baseline ⊕ state** — index `state` rows by `position_reference`; a cell with a
  `state` override renders from `state` (or hides, if removed), else from the baseline.

**Done when:** with a hand-inserted cold `state` row, the client shows the override in place of the
baseline cell, and dropping it reverts — the composite is correct even though nothing yet *produces* the
row.

---

## P4 · Mutation — mint → compose → fold (the large build)

Gated on [`blockers.md`](blockers.md) (BK1 composition reuse, BK2 minting/routing).

- **mint (`UNPACK`)**: a cold shard mints an `entity_reference` (const `SERVER_REFERENCE` + counter, the
  `event_shard` pattern) for a touched cell and writes its first `state_log` row; deterministic-from-event.
- **compose**: events targeting the cell run on the assigned worker composing cold `state_log` exactly
  like `data_shard`; `promote_state` publishes to `state` (throttled — movement fans out start/end only).
- **route**: the edge routes an entity's `state` by `entity_reference`'s `type_id` to the right cold
  shard (type_id→shard — not built; edge is single-instance today).
- **fold (`PACK`/GC)**: GC queues a fold — write the settled cell back into the baseline, drop its
  `state` row, tombstone (not drop) its `state_log` rows. Removal = a `state_log`/`state` row with the
  removed marker (no `cold_removed` table).
- **core two-phase**: core issues `UNPACK` at N=0 (pre-empted), learns the id by watching the position
  in `state`, issues the op gated on the row.

**Done when:** a scripted mutation (e.g. decrement a thing's `count`, or remove a tile) mints, composes,
promotes, renders on the client, and GC folds it back into the baseline with the `state` row dropped —
all through events, no direct cold write.
