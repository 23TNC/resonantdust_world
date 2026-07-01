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
//! The connection self-heals: each build shares an `alive` flag the SDK clears on
//! disconnect/connect-error, and [`Directory::conn`] rebuilds a dead connection
//! on the next request (the SDK does not auto-reconnect).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::{DbContext, Table};
use tracing::{error, info, warn};

use crate::bindings::index::{
    assign_player, touch_player, DbConnection, PlayerServersTableAccess, ServersTableAccess,
    SubscriptionHandle,
};
use crate::config::GatewayConfig;
use crate::resolve::pick_server;

/// How long to wait for the directory subscription to apply before giving up on a
/// (re)connect attempt.
const SUB_TIMEOUT: Duration = Duration::from_secs(5);

/// The rows the gateway needs mirrored to resolve requests.
const SUBSCRIPTION_QUERIES: [&str; 2] = ["SELECT * FROM servers", "SELECT * FROM player_servers"];

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

/// A live upstream connection to the `index` DB plus its liveness flag and the
/// kept-alive directory subscription.
struct Conn {
    conn: Arc<DbConnection>,
    /// Cleared by the SDK on disconnect/connect-error; checked on every request.
    alive: Arc<AtomicBool>,
    /// Held so the directory subscription isn't torn down (dropping the handle
    /// unsubscribes, which would stop the cache from being maintained).
    _sub: SubscriptionHandle,
}

/// The gateway's handle to the routing directory.
pub struct Directory {
    cfg: GatewayConfig,
    /// `tokio` mutex so the (re)connect path — which awaits the subscription —
    /// can be serialized without blocking the runtime, and concurrent requests
    /// share one connection.
    inner: tokio::sync::Mutex<Option<Conn>>,
}

impl Directory {
    pub fn new(cfg: GatewayConfig) -> Self {
        Self {
            cfg,
            inner: tokio::sync::Mutex::new(None),
        }
    }

    /// Eagerly establish the connection at startup. Best-effort: a failure is
    /// logged and left for the first request to retry (so the gateway still binds
    /// its HTTP port and serves `/health` even if `index` is briefly down).
    pub async fn warm_up(&self) {
        if let Err(err) = self.conn().await {
            warn!(%err, "index directory not reachable at startup; will retry on first request");
        }
    }

    /// Resolve "a server for `player_id`" — affinity reuse, else allocate.
    /// `player_id` is `None` (or `0`) for a client with no established player yet
    /// (first ever connect): it's allocated a server but not pinned — the pin is
    /// created once the player logs in at that server and a real id exists.
    pub async fn resolve(&self, player_id: Option<u32>) -> Result<Resolved, ResolveError> {
        let conn = self.conn().await.map_err(ResolveError::Unavailable)?;
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

    /// Return the live connection, rebuilding it if absent or dead.
    async fn conn(&self) -> Result<Arc<DbConnection>, String> {
        let mut guard = self.inner.lock().await;
        if let Some(c) = guard.as_ref() {
            if c.alive.load(Ordering::Acquire) {
                return Ok(c.conn.clone());
            }
            info!("index connection dead; reconnecting");
        }

        let (conn, alive) = build_conn(&self.cfg)?;
        let sub = subscribe_and_wait(&conn).await?;
        *guard = Some(Conn {
            conn: conn.clone(),
            alive,
            _sub: sub,
        });
        info!(db = %self.cfg.index_db, "index directory connected + subscribed");
        Ok(conn)
    }
}

/// Build (and start the message loop for) a connection to the `index` DB.
fn build_conn(cfg: &GatewayConfig) -> Result<(Arc<DbConnection>, Arc<AtomicBool>), String> {
    let alive = Arc::new(AtomicBool::new(true));
    let built = DbConnection::builder()
        .with_uri(&cfg.index_uri)
        .with_database_name(&cfg.index_db)
        .on_connect(|_ctx, identity, _token| info!(%identity, "index upstream connected"))
        .on_connect_error({
            let alive = alive.clone();
            move |_ctx, err| {
                alive.store(false, Ordering::SeqCst);
                error!(%err, "index upstream connect error");
            }
        })
        .on_disconnect({
            let alive = alive.clone();
            move |_ctx, err| {
                alive.store(false, Ordering::SeqCst);
                match err {
                    Some(err) => warn!(%err, "index upstream disconnected"),
                    None => info!("index upstream disconnected"),
                }
            }
        })
        .build()
        .map_err(|err| format!("build index connection: {err}"))?;
    built.run_threaded();
    Ok((Arc::new(built), alive))
}

/// Subscribe to the directory tables and await the first applied snapshot (so the
/// first request reads a populated cache), bounded by [`SUB_TIMEOUT`].
async fn subscribe_and_wait(conn: &DbConnection) -> Result<SubscriptionHandle, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    // Either on_applied or on_error fires once; whichever wins sends.
    let tx = Arc::new(Mutex::new(Some(tx)));
    let tx_ok = tx.clone();
    let tx_err = tx.clone();
    let handle = conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            if let Some(t) = tx_ok.lock().unwrap().take() {
                let _ = t.send(Ok(()));
            }
        })
        .on_error(move |_ctx, err| {
            if let Some(t) = tx_err.lock().unwrap().take() {
                let _ = t.send(Err(err.to_string()));
            }
        })
        .subscribe(SUBSCRIPTION_QUERIES.map(String::from).to_vec());

    match tokio::time::timeout(SUB_TIMEOUT, rx).await {
        Ok(Ok(Ok(()))) => Ok(handle),
        Ok(Ok(Err(err))) => Err(format!("directory subscription error: {err}")),
        Ok(Err(_)) => Err("directory subscription channel dropped".to_string()),
        Err(_) => Err("directory subscription timed out".to_string()),
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
