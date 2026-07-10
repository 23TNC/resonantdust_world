//! The zone shard module — the cold store on the tick pipeline.
//!
//! Invokes `resonantdust_pipeline::decl_tick_pipeline!` with a **divergent payload** from
//! the object shard's: a zone entity carries the whole packed `[u64;256]` cell array (as
//! a `Vec<u64>`) plus its biome and a version bumped on mutation. Same engine (event_log
//! / state_log / state / fencing / work-gen / GC), a completely different data structure
//! — the concrete proof the pipeline is generalized over its payload
//! (`docs/pipeline-generalization.md`, Phase 4).
//!
//! A zone entity is **positional** (`entity_reference` = `pack_positional_entity(zone_id,
//! …)`); `zone_id` is kept as a routing column so the edge/client can subscribe to a
//! zone's cells by it. Hot tiles/objects living in their own shards `pack` into a zone
//! via the transfer saga (a `store` action writes `cells[location]`).

resonantdust_pipeline::decl_tick_pipeline! {
    payload: {
        zone_id: u32,
        cells: Vec<u64>,
        biome: u16,
        version: u32,
    }
}
