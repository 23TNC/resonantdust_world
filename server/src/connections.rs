//! Upstream SpacetimeDB connections — the server's link to the control-plane
//! (`index`, `players`) and the data `shard`s.
//!
//! Two lifetimes:
//!   * The **index** connection is server-global and long-lived: one shared
//!     [`Pool::index`], subscribed to the whole routing directory at startup, is
//!     read by every client's zone lookups. It's a single subscriber to a tiny,
//!     low-write table, so the SpacetimeDB subscription set-semantics hazard (a
//!     second subscriber silently dropping a first's initial rows on a shared
//!     connection) doesn't apply.
//!   * The **players** and **shard** connections are built *per client*, in
//!     [`crate::ws`], precisely to avoid that hazard — each WS gets its own so
//!     their zone subscriptions don't collide. They're created with the
//!     `connect_*` helpers below and torn down when the WS closes.
//!
//! Every connection runs the SDK message loop on its own thread
//! (`run_threaded`); row/applied callbacks fire there and hand frames to the
//! per-client async writer over a channel.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::{DbContext, Table as _};
use tokio::sync::oneshot;

use crate::bindings;
use crate::bindings::index::region_shards_table::RegionShardsTableAccess;
use crate::bindings::index::set_server;
use crate::bindings::index::shards_table::ShardsTableAccess;
use crate::config::ServerConfig;
use crate::worldgen::Worldgen;

/// How long to wait for an upstream's `on_connect` (auth handshake) before
/// giving up. Matches the old gateway's 5s budget.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How often the server refreshes its index registration. Must stay comfortably
/// under the index's `SERVER_TTL_MS` (60s) so the GC never reaps a live server
/// between beats; a third of the TTL absorbs a missed beat and reducer latency.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);

/// Generate a `connect_<module>` helper that builds a per-client upstream to one
/// module's database, starts its threaded message loop, and returns the
/// connection paired with a oneshot that fires once the connection is live
/// (its `on_connect` ran). The caller `await`s the oneshot (see [`await_ready`])
/// before issuing subscriptions — subscribing on a not-yet-open connection
/// loses the initial rows.
macro_rules! connector {
    ($fn:ident, $module:ident) => {
        /// Build + start a `
        #[doc = stringify!($module)]
        /// ` upstream to `db_name` on `uri`. `None` if the builder fails
        /// (bad uri/db); a *slow* connect surfaces as a never-firing oneshot,
        /// which [`await_ready`] times out on.
        pub fn $fn(
            uri: &str,
            db_name: &str,
        ) -> Option<(
            Arc<bindings::$module::DbConnection>,
            oneshot::Receiver<()>,
        )> {
            use bindings::$module::DbConnection;
            let label = stringify!($module);
            let (ready_tx, ready_rx) = oneshot::channel();
            let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));
            let built = DbConnection::builder()
                .with_uri(uri)
                .with_database_name(db_name)
                .on_connect({
                    let ready_tx = ready_tx.clone();
                    move |_ctx, identity, _token| {
                        tracing::debug!(db = label, %identity, "upstream connected");
                        if let Some(tx) = ready_tx.lock().unwrap().take() {
                            let _ = tx.send(());
                        }
                    }
                })
                .on_connect_error(move |_ctx, err| {
                    tracing::error!(db = label, %err, "upstream connect error")
                })
                .on_disconnect(move |_ctx, err| match err {
                    Some(err) => tracing::warn!(db = label, %err, "upstream disconnected"),
                    None => tracing::debug!(db = label, "upstream disconnected"),
                })
                .build();
            match built {
                Ok(conn) => {
                    conn.run_threaded();
                    Some((Arc::new(conn), ready_rx))
                }
                Err(err) => {
                    tracing::error!(db = label, %err, "failed to build upstream");
                    None
                }
            }
        }
    };
}

connector!(connect_index, index);
connector!(connect_players, players);
// The generic data-shard connector currently targets the `region_shard` module;
// an `object_shard` connector will join it when mobile objects land.
connector!(connect_shard, region_shard);

/// Await an upstream's readiness oneshot with the connect timeout. `true` once
/// the connection's `on_connect` fired; `false` on timeout (or a dropped sender,
/// i.e. the connection died before connecting).
pub async fn await_ready(ready: oneshot::Receiver<()>) -> bool {
    matches!(tokio::time::timeout(CONNECT_TIMEOUT, ready).await, Ok(Ok(())))
}

/// Server-global shared state: config + the live index connection. Shared as
/// axum router state (`Arc<Pool>`), so it must stay `Send + Sync` — hence it
/// holds no subscription handle (the index subscription is leaked at startup;
/// see [`Pool::connect`]).
pub struct Pool {
    pub cfg: ServerConfig,
    /// The shared routing-directory connection, subscribed to `region_shards` +
    /// `shards`. Read by [`crate::index`] resolution.
    pub index: Arc<bindings::index::DbConnection>,
    /// The loaded content corpus + tile ids worldgen seeds fresh zones from.
    /// `None` if the content tree failed to load — seeding is then skipped (the
    /// server still routes existing zones); a startup warning says why.
    pub worldgen: Option<Arc<Worldgen>>,
}

impl Pool {
    /// Connect the shared index upstream, subscribe to the routing tables, and
    /// wait until both the connection and the subscription are live. Returns an
    /// error string (for a clean process exit) if either step fails — the server
    /// can't route a single zone without the index, so this is fatal at startup.
    pub async fn connect(cfg: ServerConfig) -> Result<Arc<Pool>, String> {
        let index_db = cfg.index_db();
        tracing::info!(uri = %cfg.uri, db = %index_db, "connecting index upstream");

        let (index, ready) = connect_index(&cfg.uri, &index_db)
            .ok_or_else(|| "failed to build index upstream".to_string())?;
        if !await_ready(ready).await {
            return Err("index upstream connect timed out".to_string());
        }

        // Subscribe to the full routing directory and block until the initial
        // rows are cached, so the first client's zone lookup sees a populated
        // index rather than racing an empty cache.
        let (applied_tx, applied_rx) = oneshot::channel();
        let applied_tx = Arc::new(Mutex::new(Some(applied_tx)));
        let sub = index
            .subscription_builder()
            .on_applied({
                let applied_tx = applied_tx.clone();
                move |_ctx| {
                    if let Some(tx) = applied_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                }
            })
            .on_error(|_ctx, err| tracing::error!(%err, "index subscription error"))
            .subscribe(["SELECT * FROM region_shards", "SELECT * FROM shards"]);

        if !matches!(
            tokio::time::timeout(CONNECT_TIMEOUT, applied_rx).await,
            Ok(Ok(()))
        ) {
            return Err("index subscription apply timed out".to_string());
        }

        // The index subscription lives for the whole process; leak the handle so
        // `Pool` (shared as axum state) carries no possibly-!Sync handle and the
        // subscription is never torn down. Dropping it would unsubscribe.
        std::mem::forget(sub);

        let regions = index.db().region_shards().count();
        let shards = index.db().shards().count();
        tracing::info!(regions, shards, "index ready");

        // Load the DSL content tree worldgen seeds zones from. Non-fatal: a
        // server with no content can still route + relay zones that already
        // exist; it just can't generate new terrain, so warn loudly.
        let worldgen = match Worldgen::load(std::path::Path::new(&cfg.content_dir)) {
            Ok(w) => {
                let tiles = w.bundle().tile_names().len();
                tracing::info!(dir = %cfg.content_dir, tiles, "worldgen content loaded");
                Some(Arc::new(w))
            }
            Err(err) => {
                tracing::warn!(dir = %cfg.content_dir, %err, "worldgen disabled (content load failed)");
                None
            }
        };

        Ok(Arc::new(Pool { cfg, index, worldgen }))
    }

    /// Register this server in the index `servers` table and keep it fresh. The
    /// gateway only hands out servers present in that table, and the index GC
    /// reaps any whose heartbeat is older than `SERVER_TTL_MS` — so without this
    /// a freshly-started server is never allocated to a player. Call once after
    /// the pool is up; spawns a detached task that re-registers on an interval
    /// until the process exits.
    ///
    /// Each beat re-sends the full `set_server` (url + timestamp) rather than a
    /// bare heartbeat, so a server that briefly outlived a GC reap re-creates its
    /// row instead of beating a row that no longer exists. The reducer is an
    /// idempotent upsert and the table is tiny, so the extra url per beat is free.
    pub fn spawn_registration(self: &Arc<Pool>) {
        let pool = self.clone();
        let server_id = pool.cfg.server_id;
        let url = pool.cfg.public_url.clone();

        // Reducer calls are fire-and-forget; a send failure just means the next
        // beat retries. Register synchronously first so a client arriving right
        // after startup finds the server already in the directory.
        match pool
            .index
            .reducers()
            .set_server(server_id, url.clone(), now_ms())
        {
            Ok(()) => tracing::info!(server_id, %url, "registered server in index"),
            Err(err) => {
                tracing::warn!(%err, server_id, "set_server failed; will retry on next beat")
            }
        }

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
            tick.tick().await; // interval fires immediately; skip — we just registered.
            loop {
                tick.tick().await;
                if let Err(err) = pool
                    .index
                    .reducers()
                    .set_server(server_id, url.clone(), now_ms())
                {
                    tracing::warn!(%err, server_id, "server heartbeat failed");
                }
            }
        });
    }
}

/// Wall-clock milliseconds since the Unix epoch — the `now_ms` the index
/// reducers stamp `last_seen_ms` with. Mirrors the gateway's helper.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
