//! The **cold-tiles** module — a zone's dense ground grid on the tick pipeline.
//!
//! Invokes `resonantdust_pipeline::decl_tick_pipeline!` with a tiles payload: one entity
//! per zone (keyed by the edge under a `zone_reference`) carrying the whole zone's
//! `tiles` as a dense `Vec<u8>` of length 256 — one tile-kind per cell, row-major
//! (`0` = empty). The edge seeds it from worldgen via `seed_entity` on first subscribe;
//! static (no events target it → never ticks), streamed to clients on the zone
//! subscription. `zone_id` stays a routing column so the edge subscribes `WHERE zone_id`.
//!
//! A zone's **things** (sparse `Vec<u64>`) live in the SEPARATE `cold_things` module/DB,
//! so a mostly-empty zone costs only its 256-byte tile grid, not a fixed thing array.
//! Both ride the same `decl_tick_pipeline!` engine with divergent payloads. See
//! `docs/data-shards.md`.

resonantdust_pipeline::decl_tick_pipeline! {
    payload: {
        zone_id: u32,
        tiles: Vec<u8>,
    }
}
