//! The server directory — the gateway's live view of the `index` routing DB and
//! the resolution that turns "give me a server for this player" into a concrete
//! world-server endpoint.
//!
//! The gateway holds **one** upstream connection to the `index` database and
//! subscribes to its `servers` (the live pool, with heartbeats) and
//! `player_servers` (per-player session pins) tables. The SDK keeps that
//! subscription's rows mirrored in the connection's client cache, so resolving a
//! request is a local read — no per-request round-trip to SpacetimeDB.
//!
//! Resolution (see [`Directory::resolve`]) is two cases:
//!   1. **Affinity** — the player already has a pin to a still-registered server.
//!      Reuse it (so the world server can stream the session delta on reconnect
//!      rather than rebuild it) and refresh the pin's activity stamp.
//!   2. **Allocate** — no usable pin. Pick a live server via
//!      [`crate::resolve::pick_server`] (the pluggable policy) and, for a known
//!      player, pin it for next time.
//!
//! Self-healing rides [`resonantdust_uplink`] (movement-hardening P4 — this file
//! is where the pattern was BORN; the crate lifted it, grew sub-`on_error`
//! awareness + capped backoff, and now the original consumes it back): the
//! connection is built best-effort, gated on the directory subscription applying,
//! and rebuilt on the next request after any death.

use std::time::{SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::{DbContext, Table};
use tracing::warn;

use crate::bindings::index;
use crate::bindings::index::{assign_player, touch_player, PlayerServersTableAccess, ServersTableAccess};
use crate::config::GatewayConfig;
use crate::resolve::pick_server;

/// A resolved world server to hand back to the client.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInfo {
    pub server_id: u16,
    /// The endpoint the client connects to for the live game stream.
    pub url: String,
}

/// The outcome of resolving a request.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Resolved {
    pub server: ServerInfo,
    /// `true` when an existing session pin was reused (reconnect affinity),
    /// `false` when a server was freshly allocated.
    pub reused: bool,
}

/// Why a request couldn't be resolved.
#[derive(Debug)]
pub enum ResolveError {
    /// The `index` directory connection is unavailable (couldn't connect /
    /// subscribe). Transient — a retry may succeed. → `503`.
    Unavailable(String),
    /// No world server is currently registered, so there's nothing to hand out.
    /// → `503`.
    NoServers,
}

/// The gateway's handle to the routing directory.
pub struct Directory {
    up: resonantdust_uplink::Uplink<index::DbConnection, index::SubscriptionHandle>,
}

impl Directory {
    pub fn new(cfg: GatewayConfig) -> Self {
        let up = resonantdust_uplink::subbed_uplink!(
            index,
            "index directory",
            cfg.index_uri,
            cfg.index_db,
            vec!["SELECT * FROM servers".to_string(), "SELECT * FROM player_servers".to_string()]
        );
        Self { up }
    }

    /// Eagerly establish the connection at startup. Best-effort: a failure is
    /// logged and left for the first request to retry (so the gateway still binds
    /// its HTTP port and serves `/health` even if `index` is briefly down).
    pub async fn warm_up(&self) {
        if let Err(err) = self.up.get().await {
            warn!(%err, "index directory not reachable at startup; will retry on first request");
        }
    }

    /// Resolve "a server for `player_id`" — affinity reuse, else allocate.
    /// `player_id` is `None` (or `0`) for a client with no established player yet
    /// (first ever connect): it's allocated a server but not pinned — the pin is
    /// created once the player logs in at that server and a real id exists.
    pub async fn resolve(&self, player_id: Option<u32>) -> Result<Resolved, ResolveError> {
        let conn = self.up.get().await.map_err(ResolveError::Unavailable)?;
        let now = now_ms();
        let player_id = player_id.filter(|&p| p != 0);

        // 1. Affinity: reuse a pin to a still-registered server.
        if let Some(pid) = player_id {
            if let Some(pin) = conn.db().player_servers().player_id().find(&pid) {
                if let Some(server) = conn.db().servers().server_id().find(&pin.server_id) {
                    // Best-effort activity refresh so GC doesn't release the pin.
                    if let Err(err) = conn.reducers().touch_player(pid, now) {
                        warn!(%err, player_id = pid, "touch_player failed");
                    }
                    return Ok(Resolved {
                        server: ServerInfo {
                            server_id: server.server_id,
                            url: server.url,
                        },
                        reused: true,
                    });
                }
            }
        }

        // 2. Allocate from the live pool via the pluggable policy.
        let pool: Vec<_> = conn.db().servers().iter().collect();
        let chosen = pick_server(&pool).ok_or(ResolveError::NoServers)?;
        let server = ServerInfo {
            server_id: chosen.server_id,
            url: chosen.url.clone(),
        };

        // Pin a known player to the chosen server so a reconnect lands back here.
        if let Some(pid) = player_id {
            if let Err(err) = conn.reducers().assign_player(pid, server.server_id, now) {
                warn!(%err, player_id = pid, "assign_player failed");
            }
        }

        Ok(Resolved {
            server,
            reused: false,
        })
    }
}

/// Wall-clock milliseconds since the Unix epoch — the `now_ms` the `index`
/// heartbeat/activity reducers expect (gateway/server-resolved time).
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
