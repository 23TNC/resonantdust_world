//! Upstream SpacetimeDB connections — the server's link to the control-plane
//! (`index`, `players`).
//!
//! Two lifetimes:
//!   * The **index** connection is server-global and long-lived: one shared
//!     [`Pool::index`], subscribed to the whole routing directory at startup, is
//!     read by every client's zone lookups. It's a single subscriber to a tiny,
//!     low-write table, so the SpacetimeDB subscription set-semantics hazard (a
//!     second subscriber silently dropping a first's initial rows on a shared
//!     connection) doesn't apply.
//!   * The **players** connection is built *per client*, in [`crate::ws`], precisely to
//!     avoid that hazard — each WS gets its own so their subscriptions don't collide.
//!     It's created with the `connect_*` helpers below and torn down when the WS closes.
//!     Data-shard connections belonged here too until the shard was deleted for the
//!     rebuild; they return the same way.
//!
//! Every connection runs the SDK message loop on its own thread
//! (`run_threaded`); row/applied callbacks fire there and hand frames to the
//! per-client async writer over a channel.

use crate::lock::RwRecover;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::{DbContext, Table as _};
use tokio::sync::oneshot;

use crate::bindings;
use crate::bindings::index::set_server;
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
// The rebuild's sim shards (`docs/intent/spacetime-again/`): the edge queues client intents to
// `event_shard` and relays `state`/`event` rows from both, per client (own connection = own
// subscriptions, avoiding the set-semantics hazard).
connector!(connect_event_shard, event_shard);
connector!(connect_data_shard, data_shard);
connector!(connect_pawn, pawn);
// The cold shards (`docs/intent/world-storage/`): a zone's ground + scatter, relayed per zone.
connector!(connect_tile, tile);
connector!(connect_thing, thing);

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
    /// The shared control-plane connection. Used write-only: the edge registers
    /// itself in the index's `servers` table via [`set_server`] (see
    /// [`spawn_registration`](Pool::spawn_registration)) so the gateway can allocate
    /// it to players. No subscription — the old zone→shard router that read
    /// `region_shards`/`shards` off this connection is gone (coord-purge C).
    pub index: Arc<bindings::index::DbConnection>,
    /// Hot-reloadable content-derived state (currently the worldgen runtime),
    /// behind an `RwLock` so [`spawn_content_poll`](Pool::spawn_content_poll) can
    /// swap in a rebuilt runtime when the corpus changes. Consumers read the live
    /// value with [`current_worldgen`](Pool::current_worldgen) **at the point of
    /// use** (per zone seed) — never cache the `Arc` across a reload, or a new zone
    /// would generate from stale content.
    content: RwLock<ContentState>,
    /// Zones this edge has already generated + seeded into the cold shards, so a subscribe seeds a
    /// zone at most once per process. Generation is deterministic and `seed` idempotent, so a double
    /// seed (a race, or another edge) is harmless — this just avoids the redundant work.
    seeded_zones: Mutex<HashSet<u16>>,
}

/// The server's content-derived state — the worldgen runtime plus the corpus
/// fingerprint the reload compares against. A single lock covers the pair so a
/// swap is atomic. As more content-derived state appears (recipes, pawn
/// behaviours), it joins here and rebuilds on the same reload.
struct ContentState {
    /// [`resonantdust_dsl::content::content_version`] of the loaded corpus (`0`
    /// when content failed to load).
    version: u64,
    /// The generation runtime worldgen seeds fresh zones from. `None` if the
    /// content tree failed to load — seeding is then skipped (the server still
    /// routes existing zones); a startup warning says why.
    worldgen: Option<Arc<Worldgen>>,
}

impl Pool {
    /// Connect the shared control-plane (index) upstream and wait until it's live.
    /// Returns an error string (for a clean process exit) if the connect fails —
    /// the edge can't register itself for allocation without the index, so this is
    /// fatal at startup. Subscribes `cold_shards` (the region→cold-shard router the
    /// edge resolves cold subscriptions through); the connection is otherwise
    /// write-only (the `set_server` registration heartbeat).
    pub async fn connect(cfg: ServerConfig) -> Result<Arc<Pool>, String> {
        use crate::bindings::index::cold_shards_table::ColdShardsTableAccess as _;

        let index_db = cfg.index_db();
        tracing::info!(uri = %cfg.uri, db = %index_db, "connecting index upstream");

        let (index, ready) = connect_index(&cfg.uri, &index_db)
            .ok_or_else(|| "failed to build index upstream".to_string())?;
        if !await_ready(ready).await {
            return Err("index upstream connect timed out".to_string());
        }

        // Subscribe the cold router and block on the initial rows so the first
        // zone's cold resolution sees a populated table (not an empty cache).
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
            .on_error(|_ctx, err| tracing::error!(%err, "cold_shards subscription error"))
            .subscribe(["SELECT * FROM cold_shards"]);
        if !matches!(tokio::time::timeout(CONNECT_TIMEOUT, applied_rx).await, Ok(Ok(()))) {
            return Err("cold_shards subscription apply timed out".to_string());
        }
        std::mem::forget(sub); // lives for the whole process; Pool stays Send + Sync
        tracing::info!(routes = index.db().cold_shards().count(), "index ready");

        // Load the DSL content tree worldgen seeds zones from. Non-fatal: a
        // server with no content can still route + relay zones that already
        // exist; it just can't generate new terrain, so warn loudly.
        let content = load_content_state(&cfg.content_dir);

        Ok(Arc::new(Pool {
            cfg,
            index,
            content: RwLock::new(content),
            seeded_zones: Mutex::new(HashSet::new()),
        }))
    }

    /// The live worldgen runtime (a cloned `Arc`), or `None` if content isn't
    /// loaded. Read this **per zone seed** so a content hot-reload applies to
    /// zones generated after the swap.
    pub fn current_worldgen(&self) -> Option<Arc<Worldgen>> {
        self.content.read_r().worldgen.clone()
    }

    /// The cold shard endpoint `(url, db_name)` serving `(type_id, region_reference)`, from the
    /// `index.cold_shards` router — or `None` if the region isn't routed (the caller falls back to
    /// the configured default while single-shard). One row per assigned `(type, region)`.
    pub fn cold_endpoint(&self, type_id: u8, region_reference: u8) -> Option<(String, String)> {
        use crate::bindings::index::cold_shards_table::ColdShardsTableAccess as _;
        self.index
            .db()
            .cold_shards()
            .iter()
            .find(|r| r.type_id == type_id && r.region_reference == region_reference)
            .map(|r| (r.url, r.db_name))
    }

    /// Claim the first-seed of `zone`: `true` if this call is the one to generate + seed it (it was
    /// not yet in the set and is now), `false` if already seeded. Cheap compare-and-insert.
    pub fn claim_zone_seed(&self, zone: u16) -> bool {
        self.seeded_zones.lock().unwrap().insert(zone)
    }

    /// The loaded corpus fingerprint (`0` if content failed to load).
    pub fn content_version(&self) -> u64 {
        self.content.read_r().version
    }

    /// Re-read the content tree and, on a fingerprint change, rebuild + swap in
    /// the worldgen. **Refuses** a corpus whose tile/thing ids aren't
    /// append-compatible with the live one — a reorder or removal would renumber
    /// ids, so zones already stored with the old ids would be misread; that needs
    /// a restart (and matching client + stored-zone migration), not a live swap.
    /// Parsing happens before the lock, so an unchanged or bad corpus never blocks
    /// readers. Returns whether it swapped.
    pub fn reload_content(&self) -> Result<bool, String> {
        let loaded = Worldgen::load_versioned(std::path::Path::new(&self.cfg.content_dir))?;
        let mut state = self.content.write_r();
        if loaded.version == state.version {
            return Ok(false);
        }
        if let Some(prev) = &state.worldgen {
            if !loaded.worldgen.is_append_compatible_with(prev) {
                return Err(
                    "tile/thing ids changed (reorder or removal) — refusing hot-reload; restart to apply"
                        .to_string(),
                );
            }
        }
        state.version = loaded.version;
        state.worldgen = Some(Arc::new(loaded.worldgen));
        Ok(true)
    }

    /// Spawn the background content poll: every `secs` seconds re-read the content
    /// tree and hot-reload the worldgen on a change (via [`reload_content`], so an
    /// id-incompatible edit is refused). `secs == 0` disables it (load-once). The
    /// re-read runs on a blocking thread so the runtime isn't stalled. New zones
    /// then generate from the swapped-in content; already-seeded zones are stored
    /// and unaffected.
    pub fn spawn_content_poll(self: &Arc<Pool>, secs: u64) {
        if secs == 0 {
            tracing::info!("worldgen content hot-reload disabled (RD_CONTENT_POLL_SECS=0)");
            return;
        }
        let pool = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(secs));
            tick.tick().await; // the first tick fires immediately — skip it (just loaded)
            loop {
                tick.tick().await;
                let p = pool.clone();
                match tokio::task::spawn_blocking(move || p.reload_content()).await {
                    Ok(Ok(true)) => tracing::info!(
                        version = format!("{:016x}", pool.content_version()),
                        "worldgen content hot-reloaded"
                    ),
                    Ok(Ok(false)) => {}
                    Ok(Err(reason)) => tracing::warn!(%reason, "worldgen content reload skipped"),
                    Err(err) => tracing::warn!(%err, "worldgen reload task failed"),
                }
            }
        });
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

/// Load the initial content-derived state (worldgen + fingerprint) from
/// `content_dir`. Non-fatal: a failed load yields an empty state (no worldgen,
/// version `0`), logged, so the server still routes existing zones — the poll can
/// later pick content up once it becomes valid.
fn load_content_state(content_dir: &str) -> ContentState {
    match Worldgen::load_versioned(std::path::Path::new(content_dir)) {
        Ok(loaded) => {
            let tiles = loaded.worldgen.bundle().tile_names().len();
            tracing::info!(
                dir = %content_dir,
                tiles,
                version = format!("{:016x}", loaded.version),
                "worldgen content loaded"
            );
            ContentState { version: loaded.version, worldgen: Some(Arc::new(loaded.worldgen)) }
        }
        Err(err) => {
            tracing::warn!(dir = %content_dir, %err, "worldgen disabled (content load failed)");
            ContentState { version: 0, worldgen: None }
        }
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
