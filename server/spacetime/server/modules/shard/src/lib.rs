//! The object/zone shard module.
//!
//! One module, deployed as either an **object shard** (mobile things/pawns) or a **zone
//! shard** in its legacy sense. Invokes `resonantdust_pipeline::decl_tick_pipeline!` with
//! the **spatial payload** objects and zones share today; the macro registers the tick
//! tables + reducers here. Role is chosen at deploy time (the DB it publishes to +
//! `set_shard_id`); there is no per-role code. A divergent payload (the `[u64;256]` zone
//! cell array) lives in its own module — see `docs/components/server/spacetime/pipeline/design/pipeline-generalization.md`.
//!
//! `zone_id` is a routing key (the edge subscribes `WHERE zone_id`); it stays an ordinary
//! column — promoting it to an indexed routing key is a later refinement.

resonantdust_pipeline::decl_tick_pipeline! {
    payload: {
        kind: u16,
        zone_id: u32,
        location: u8,
        rotation: u8,
        offset: u8,
        data0: u64,
        data1: u64,
    }
}
