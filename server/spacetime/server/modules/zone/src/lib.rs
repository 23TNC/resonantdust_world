//! The zone-terrain shard module — the cold store for static terrain on the tick pipeline.
//!
//! Invokes `resonantdust_pipeline::decl_tick_pipeline!` with a **terrain payload**: one
//! **positional** entity per zone (`entity_reference =
//! pack_positional_entity(ENTITY_TYPE_ZONE_TERRAIN, zone_id, 0, 0)`) carrying the whole
//! zone's packed `tiles` (`pack_tile`, one `u16` per cell, row-major) and its scattered
//! `things` (`pack_thing_at`) — exactly `worldgen::zone_terrain()`'s output. The edge seeds
//! one per zone via `seed_entity` on first subscribe; it is **static** (no events target it,
//! so it never ticks) — it just sits in `state` and streams to clients on the zone
//! subscription. `zone_id` stays a routing column so the edge subscribes `WHERE zone_id`.
//!
//! Deployed to its **own DB**, separate from the object shard (which carries mobile
//! objects/pawns on the same engine with a spatial payload) — the two pipelines share the
//! `decl_tick_pipeline!` engine but not a schema. See `docs/pipeline-generalization.md` and
//! `docs/zones-to-screen.md`.

resonantdust_pipeline::decl_tick_pipeline! {
    payload: {
        zone_id: u32,
        tiles: Vec<u16>,
        things: Vec<u32>,
    }
}
