# Zones to screen — the wiring that gets biomes + trees back in the browser

**Status (2026-07-11, branch `0.2.2`).** The generalized tick pipeline core is built and
proven (P1–P4c green), but **nothing on the zone path reaches pixels yet**. This doc is the
concrete, ground-truthed gap list and the shortest path to close it. It refines step 4
("client zone rendering") of the *Next* list in
[`pipeline-generalization.md`](pipeline-generalization.md) and supersedes the storage half
of [`world-on-pipeline.md`](world-on-pipeline.md) (which still holds the milestone framing).

## Current reality — every stage of zone → screen

| Stage | State | File | Gap |
|---|---|---|---|
| Generalized pipeline core (P1–P4c) | ✅ built, green | `shared/tick`, `resonantdust_pipeline` macro | — |
| `zone` module (divergent-payload proof) | ⚠️ exists, **superseded payload** | [`server/spacetime/server/modules/zone/src/lib.rs`](../server/spacetime/server/modules/zone/src/lib.rs) | payload is the old `cells: Vec<u64>` `[u64;256]`; no worldgen feed, no edge sub, no render — proof only |
| Worldgen (biome classify → tiles + things) | ✅ intact, **called by nobody** | [`server/edge/src/worldgen.rs`](../server/edge/src/worldgen.rs) | `zone_terrain(zone_id) -> (Vec<u16>, Vec<u32>)` returns terrain no one consumes |
| Edge zone subscription / seeding | ❌ missing | [`server/edge/src/ws.rs`](../server/edge/src/ws.rs) `handle_sub_zone` | subscribes `state` only; no `seed_zone_if_empty`, no zone-row relay |
| Client protocol row type | ❌ missing | [`client/core/src/protocol.rs`](../client/core/src/protocol.rs) `RowData` | only `State`; no zone row variant |
| Client events / wasm marshalling | ❌ stripped | `client/core/src/{api,protocol,web}.rs`, `shared/wasm/src/lib.rs` | `ZoneTiles`/`ZoneThings` removed in `f366c21`, `19845d1` |
| pixijs terrain painter | ❌ gutted | [`client/pixijs/src/game/world/WorldBridge.ts`](../client/pixijs/src/game/world/WorldBridge.ts) | camera/anchor bridge only; `zoneTilePrims`/`zoneThingPrims` maps removed |
| `shared/codec/cells.rs` | ⚠️ old design | [`shared/codec/src/cells.rs`](../shared/codec/src/cells.rs) | `[u64;256]` packed cell + biome-folding; **to be replaced** by `zone_tiles`/`zone_objects` |

**The render primitives survive.** `Content::zoneTilePrims` / `zoneThingPrims` (shared/wasm)
still expand packed Vecs → texture + tint via the DSL — kept precisely for the restore.

## The fork — two scopes hide in "get zones generating again"

**Scope A — static terrain on screen.** Biomes + trees render while panning; no in-world
edits. A seeded zone that is never mutated **never ticks**, so this needs **neither** the
worker `data_type` dispatch **nor** the pack/unpack saga. This is the short path below.

**Scope B — live, mutable world.** Pickup/place, hot per-cell cells, terrain edits. Adds the
rest of the [`pipeline-generalization.md`](pipeline-generalization.md) *Next* list: worker
`data_type` dispatch → pack/unpack saga + `event_log` provenance → per-layer client overlay.
Scope A is a strict prefix of B — nothing built for A is thrown away.

## Scope A — the shortest path to pixels

1. **Reshape the `zone` module payload** — from the superseded `cells: Vec<u64>` to the
   settled split ([`pipeline-generalization.md`](pipeline-generalization.md) status log):
   - `zone_tiles`: `{ tiles: Vec<u8>, biome: u16, version: u32 }` — dense, len 256, one floor
     per cell, present for every zone (~0.25 KB).
   - `zone_objects`: `{ objects: Vec<u32> }` — sparse, only occupied cells; entry =
     `x:4 | y:4 | kind:16 | layer:3 | data:5`.
   - Add a `seed_zone(zone_id, tiles, biome[, objects])` reducer that **inserts only if
     absent** (a worldgen re-run must never clobber a live/edited zone).
   - Retire [`shared/codec/cells.rs`](../shared/codec/src/cells.rs) in the same pass.
   - **Decision to lock first:** fold `zone_objects` in now, or ship tiles-only first?
     Recommended **now** — worldgen already returns `things: Vec<u32>`, so a tiles-only
     first cut just means a second wiring round-trip for the same data.

2. **Edge — seed + subscribe** ([`server/edge/src/ws.rs`](../server/edge/src/ws.rs) `handle_sub_zone`):
   - `seed_zone_if_empty(shard, zone_id)`: query `zone_tiles` for `zone_id`; if absent,
     `worldgen.zone_terrain(zone_id)` → `reducers.seed_zone`.
   - Add the per-zone `SELECT * FROM zone_tiles WHERE zone_id = {zone_id}` (and
     `zone_objects`) subscription alongside the existing `state` sub; relay its rows.
   - Regenerate `zone` (and `shard`) bindings.

3. **Client protocol** ([`client/core/src/protocol.rs`](../client/core/src/protocol.rs)):
   - Re-add a zone row variant to `RowData` and the `ZoneTiles` / `ZoneThings` events plus
     the wasm→JS marshalling. **Restore-from-git:** `f366c21` / `19845d1` hold the exact
     removed code — re-source it against the new `zone_tiles`/`zone_objects` shapes.

4. **pixijs terrain painter** ([`WorldBridge.ts`](../client/pixijs/src/game/world/WorldBridge.ts)):
   - Restore `onZoneTiles` / `onZoneThings` → `zoneTilePrims` / `zoneThingPrims` maps over
     the surviving `Content` DSL helpers, `onZoneClosed` drop, `setContent` re-expand,
     alongside the camera/anchor bridge that was kept.

**Deliverable:** biomes + trees render while panning, entirely through the pipeline — and the
legacy `cold_zones` / `hot_*` tables can finally be deleted (gap #9).

## Pitfall — do not resurrect the cold-blob

[`world-on-pipeline.md`](world-on-pipeline.md) M1 describes a `zone_data` **side-table on the
`shard` module**. That storage is **retired**. Zones are their own module/pipeline; the table
above is a payload on the **`zone`** module, not a table on `shard`. (An exploration pass
following the old doc mis-recommended the `shard` side-table — hence this note.)
