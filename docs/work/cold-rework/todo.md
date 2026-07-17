# Todo — cold-rework

_Planned, not started. Dependency-ordered by phase (P1 gates the shape; P2/P3 stand up the router +
overlay read-path; P4 is the mutation build). All blockers resolved — decisions in
[`forks.md`](forks.md). Move an item to `completed.md` when it lands.
VARIABLES/TABLES already carry the target shape._

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

Stand up the position → cold-shard indirection now. **Explicit region rows** (no wildcard — [F3]).

- **module** `index`: add `cold_shards` (`route_reference` PK = `type_id:4 | region_reference:8`,
  `type_id`/`region_reference` idx, `shard_reference`, `url`, `db_name`) + `set_cold_shard`/
  `remove_cold_shard` reducers. The **master** assigns `(type, region) → shard` as regions come online;
  seed the region(s) in use today (around the origin) → shard 0, per family. An unassigned region has no
  row.
- **edge**: subscribe `cold_shards`; resolve a cold sub by `position → macro → region_reference →
  (type, region)` → endpoint; connect there. Replace the hardcoded `tile_db()`/`thing_db()`. This same
  lookup answers P4's `state` routing ([F2]).

**Done when:** the edge reaches cold via the router (not hardcoded names); adding a row routes a region
elsewhere with no code change; login → zone render still works.

---

## P3 · Overlay read-path — `state`/`state_log` on cold (shared macro)

Give cold the hot pair and composite it, before anything writes it. Via the shared macro ([F1]).

- **step 1 — extract `tick_pipeline!`**: lift `data_shard`'s `clock`/`state_log`/`state` +
  `init`/`bump`/`claim`/`write`/`gc` into a shared macro crate; reduce `data_shard` to the payload +
  the invocation. **Prove byte-identical** — regenerate bindings (clean diff), the wolf still moves —
  *before* anything else builds on it. (Re-touches the live pipeline.)
- **step 2 — cold invokes it**: `tile`/`thing` invoke `tick_pipeline!` (same three-ref payload) +
  keep their baseline tables. No writes yet beyond `init`/`seed`.
- **edge**: also subscribe a zone's cold `state` (`WHERE macro_position_reference = <zone>`) and relay
  it; a `ColdState` frame carries the per-entity override.
- **client**: composite **baseline ⊕ state** — index `state` rows by `position_reference`; a cell with a
  `state` override renders from `state` (or hides, if removed), else from the baseline.

**Done when:** `data_shard` is macro-generated and unchanged (wolf moves); and with a hand-inserted cold
`state` row the client shows the override in place of the baseline cell, reverting when it's dropped —
the composite is correct even though nothing yet *produces* the row.

---

## P4 · Mutation — mint → compose → fold (the large build)

Decisions settled ([`forks.md`](forks.md) F1/F2). Builds on P2's router + P3's overlay.

- **server_reference (master-assigned, [F2])**: each cold shard gets a `server` table set by the master
  at standup (like `set_orchestrator`); the mint reads it. (`event_shard`'s hardcoded const migrates to
  this pattern as a small follow-up.)
- **mint (`UNPACK`)**: a cold shard mints an `entity_reference` (`server_reference` + counter, the
  `event_shard` pattern) for a touched cell and writes its first `state_log` row; deterministic-from-event.
- **compose**: events targeting the cell run on the assigned worker composing cold `state_log` exactly
  like `data_shard` (same macro); `promote_state` publishes to `state` (throttled — movement fans out
  start/end only).
- **route ([F2])**: the edge routes an entity's `state` by `entity_reference`'s `type_id` → region →
  shard through the **same `index.cold_shards`** lookup P2 built.
- **fold (`PACK`/GC)**: GC queues a fold — write the settled cell back into the baseline, drop its
  `state` row, tombstone (not drop) its `state_log` rows. Removal = a `state_log`/`state` row with the
  removed marker (no `cold_removed` table).
- **core two-phase**: core issues `UNPACK` at N=0 (pre-empted), learns the id by watching the position
  in `state`, issues the op gated on the row.

**Done when:** a scripted mutation (e.g. decrement a thing's `count`, or remove a tile) mints, composes,
promotes, renders on the client, and GC folds it back into the baseline with the `state` row dropped —
all through events, no direct cold write.
