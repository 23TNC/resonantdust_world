# The tile world on the pipeline — biomes, terrain & trees

How the tile-based world (biome-classified terrain + scattered trees/flora) moves
onto the 0.2 tick pipeline, replacing the deleted `cold_zones` / `hot_*` tables.
Realises gap **#5** in [`gaps.md`](gaps.md) ("hot/cold world objects") and the
**#6** layer plan. Decision (2026-07-10): **cold-blob first**, hot per-cell cells
layered on after.

## What already exists — this is plumbing, not new content

- **Worldgen + biomes + DSL are intact.** `Worldgen::zone_terrain(zone_id) -> (tiles:
  Vec<u16>, things: Vec<u32>)` (server/src/worldgen.rs) classifies each cell's biome
  and returns packed terrain: `tiles` = 256 slots (`pack_tile(def_id, 0)` =
  `def_id:12 | reserved:4`), `things` = scattered flora (`pack_thing_at`). Trees are
  already there. **No worldgen/DSL/biome changes in any milestone below.**
- **The zone entity key stacks layers.** `pack_zone_key(zone_id, location, layer)` with
  `LAYER_FLOOR/WALL/THING` (shared/codec). A terrain cell is a first-class entity.
- **Client expansion helpers survive.** `Content::zoneTilePrims` / `zoneThingPrims` /
  `freeThingPrim` (shared/wasm) still take the packed Vecs and run the DSL for
  texture + tint. Kept precisely for this.
- **The engine** (event/state/resolve/GC, cross-shard C2, migration saga C3) is proven.

## Why cold-blob

The `state` row is fixed-width (`kind:u16` + `data0/data1:u64`) — it can't hold a
256-tile zone. Two ways out (see the M1 decision):

- **Cold blob** — the packed Vecs live in a `zone_data` side-table (1 row/zone), the
  "game-data" half of the state-log meta/game split. Reuses the client renderer
  unchanged; subscriptions stay ~1 row/zone like the old `cold_zones`.
- **Per-cell** — 256 entities/zone keyed by `(zone_id, location, layer)`. Natural
  pipeline granularity but 256×-row subscriptions and a new client renderer.

They converge: the end-state is **cold blob (static terrain) + hot per-cell cells
(active terrain)** — so blob-first is a strict prefix of the final design, not throwaway.

---

## Milestone 1 — static terrain back on screen (cold blob)

Goal: pan the world in the browser and see biomes + trees, sourced entirely through
the pipeline. Static only (no in-world edits yet).

### Module (`spacetime/server/pipeline` → the `shard` module)
- **`zone_data` table** (public), the pipeline's game-data half — `cold_zones` reborn:
  - `#[primary_key] zone_id: u32`, `tiles: Vec<u16>`, `things: Vec<u32>`,
    `version: u32` (bumped on any later mutation → drives client re-send), `generated_at`.
  - Not tick-managed (no `dirty`); `tick_gc` ignores it.
- **`seed_zone(zone_id, tiles, things)` reducer** — insert **only if absent** (a re-seed
  on an existing zone is a no-op, so later in-world edits are never clobbered by a
  worldgen re-run). Sets `version = 1`.

### Edge (`server/src/ws.rs`)
- **`seed_zone_if_empty(shard, zone_id)`** — restore the pre-merge helper, retargeted:
  query `zone_data` for `zone_id`; if missing, `worldgen.zone_terrain(zone_id)` →
  `reducers.seed_zone`. Call from `handle_sub_zone`.
- **Second per-zone subscription** — `SELECT * FROM zone_data WHERE zone_id = {zone_id}`
  alongside the existing `state` sub; relay its rows. (One shard connection multiplexes;
  route by `zone_id` as today.)
- Regenerate the `shard` bindings to include `zone_data` + `seed_zone`.

### Protocol + client (largely restore-from-git, re-sourced)
- **`RowData::ZoneData { zone_id, tiles, things, version }`** (server + client mirror) —
  the old `ColdZone` shape, now fed by `zone_data`. Re-add `Event::ZoneTiles` /
  `ZoneThings` (or a combined `ZoneTerrain`) and the wasm→JS marshalling + WasmClient
  `onZoneTiles`/`onZoneThings` handlers that the merge removed (git `f366c21`/`19845d1`
  have the exact code to restore).
- **WorldBridge** regains its terrain painter (`zonePrims`/`zoneThings` maps,
  `onZoneTiles`/`onZoneThings` → `zoneTilePrims`/`zoneThingPrims`, `onZoneClosed` drop,
  `setContent` re-expand) — restore alongside the camera/anchor bridge that was kept.

**Deliverable:** biomes + trees render while panning, all through the pipeline; the old
`cold_zones`/`hot_*` tables stay deleted.

---

## Milestone 2 — hot per-cell cells (efficiency + mutation substrate)

Goal: only *active* cells become individual entities; static terrain stays folded in the
blob. This is where the layer key earns its keep.

- **Per-cell ZONE entities** keyed by `pack_zone_key(zone_id, location, layer)`; `kind` =
  def_id, `rotation`/`data` as needed. Resolved by the existing engine (key is opaque —
  no read-rule/priority/GC change).
- **`unpack` (cold→hot)** — promote a blob cell to a hot `state` entity on first touch.
  **`pack`/`fold` (hot→cold)** — reabsorb a settled hot cell into the blob (delete the hot
  entity, bump `zone_data.version`).
- **Client overlay** — render hot per-cell `state` rows (ZONE keys) on top of the cold
  blob, per-layer z-order (floor < wall < things). Adds the per-cell renderer; the blob
  path from M1 is unchanged.

---

## Milestone 3 — mutations + pickup / place

Goal: change the world through events.

- **Terrain edits as ACTION events** on cell entities (place/remove wall, retile, chop
  tree): `unpack`-on-mutate → resolve → `pack`-on-settle → `version` bump → client
  re-render.
- **Pickup / place** via the C3 object↔zone transfer saga: chop tree → thing cell →
  dropped mobile object (zone→object transfer); place → object→zone transfer into a
  chosen `layer`.
- **Layer-aware placement** — `receive` carries the destination layer (per the #6 plan).

---

## Cross-cutting notes
- **Two subs per zone now** (`state` + `zone_data`); SubStats gains a `zone_data` tag.
- **Determinism vs content edits** — `seed_zone` is seed-only-if-absent; a `.rd` edit that
  renumbers def_ids must not re-seed a live zone (the old `is_append_compatible_with`
  guard). Worldgen content is append-only by convention.
- **The meta/game split** (user-requested) is realised minimally here: fixed metadata
  (`version`, later a per-zone frontier) vs the variable packed blob in `zone_data`. If a
  zone later needs a true tick frontier, add a zone `state` entity carrying it.
- **Scope of change is mostly edge + client**; the module gains one table + one reducer in
  M1. Worldgen/DSL/biomes: untouched throughout.
