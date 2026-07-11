//! The zone-**cold-things** module — a zone's sparse settled thing list on the tick
//! pipeline.
//!
//! Invokes `resonantdust_pipeline::decl_tick_pipeline!` with a things payload: one entity
//! per zone, keyed by a **`zone_reference`** (`pack_zone_reference(server_id, zone_id)` —
//! `server_id:16 | reserved:16 | zone_id:32`), carrying the zone's `things` as a **sparse**
//! `Vec<u64>` — each entry `kind:16 | x:4 | y:4 | data:5 | layer:3 | variant:5 |
//! reserved:27` (see `shared/codec/src/packed.rs`). Only occupied `(cell, layer)` slots are
//! stored, so a mostly-empty zone costs a few bytes instead of a fixed 256-slot array.
//!
//! "Cold" = packed, settled, no per-thing id — the counterpart to the coming **hot-things**
//! shard, whose things each get a *minted* `u64 entity_reference` so they can be
//! actors/targets and carry richer state; a settled hot thing folds back into this cold
//! list (pack/unpack). Held in its own DB, separate from the dense tile grid (the
//! `cold_tiles` module) and the mobile-object shard. The edge seeds it from worldgen via `seed_entity`
//! on first subscribe. `zone_id` is a routing column so the edge subscribes `WHERE
//! zone_id`. See `docs/data-shards.md`.

resonantdust_pipeline::decl_tick_pipeline! {
    payload: {
        zone_id: u32,
        things: Vec<u64>,
    }
}
