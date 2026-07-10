//! Region routing — the zone → region → shard-endpoint resolution chain, read
//! off the `index` database's subscribed cache.
//!
//! The index is a two-step lookup directory (see the `index` module docs):
//!
//! ```text
//! region_shards:  region_id (u32) -> shard_id (u16)
//! shards:         shard_id  (u16) -> { url, db_name }
//! ```
//!
//! A zone's region is its `zone_id` with the low byte cleared. The server keeps
//! one shared connection to the index, subscribed to both tables, and resolves
//! against that connection's local cache — no per-lookup round trip.

use crate::bindings::index::region_shards_table::RegionShardsTableAccess;
use crate::bindings::index::shards_table::ShardsTableAccess;
use crate::bindings::index::DbConnection as IndexConnection;
use crate::config::ServerConfig;
use spacetimedb_sdk::{DbContext, Table};

// The `zone_id → region_id` mask is defined once in the shared `resonantdust_codec`
// and re-exported here, so the native server and the `index` SpacetimeDB module
// resolve regions with the same code (they used to carry duplicated masks with a
// "keep the two in lockstep" comment).
pub use resonantdust_codec::packed::region_of;

/// Where a shard physically lives: the SpacetimeDB server URL plus the database
/// name on it holding the shard's zones. This is what a shard upstream connects
/// to. Two regions can resolve to the same endpoint (same shard) or to different
/// servers entirely — regions may live on different SpacetimeDB databases.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShardEndpoint {
    pub url: String,
    pub db_name: String,
}

/// Resolve the shard endpoint holding `zone_id`'s region, with the single-shard
/// fallback applied: an unrouted region (no `region_shards` entry) maps to the
/// env's default shard DB on the control-plane server — single-shard deployments
/// seed no index rows, so this is the common path. Returns `Err` only when the
/// region *is* routed but its shard endpoint row is missing — a partial index the
/// server can't act on.
pub fn resolve_zone_or_default(
    conn: &IndexConnection,
    cfg: &ServerConfig,
    zone_id: u32,
) -> Result<ShardEndpoint, String> {
    let region_id = region_of(zone_id);
    match conn
        .db()
        .region_shards()
        .iter()
        .find(|r| r.region_id == region_id)
    {
        Some(rs) => match conn.db().shards().iter().find(|s| s.shard_id == rs.shard_id) {
            Some(shard) => Ok(ShardEndpoint {
                url: shard.url,
                db_name: shard.db_name,
            }),
            None => Err(format!(
                "region {region_id:#010x} routes to shard {} but no shards row exists",
                rs.shard_id
            )),
        },
        None => Ok(ShardEndpoint {
            url: cfg.uri.clone(),
            db_name: cfg.default_shard_db(),
        }),
    }
}
