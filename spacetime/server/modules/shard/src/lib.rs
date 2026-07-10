//! The unified shard module.
//!
//! One module, deployed as either an **object shard** (mobile things/pawns) or a **zone
//! shard** (hot zone cells + — later — the cold-zone tier). Both run the identical
//! `resonantdust_pipeline` engine (event_log / state_log / state / tic_meta + the
//! append_event / bump / claim / resolve / seed_entity / spawn_object / tick_gc reducers
//! + object-id minting), which registers here via the glob re-export. Role is chosen at
//! deploy time (the DB it publishes to + `set_shard_id`); there is no per-role code.
//!
//! Replaces the former `object_shard` / `zone_shard` modules (and all their bitemporal
//! legacy tables) — see docs/simulation.md.
pub use resonantdust_pipeline::*;
