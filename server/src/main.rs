//! resonantdust world server — the bridge between clients and the SpacetimeDB
//! data shards. It serves an inbound WebSocket listener (`ws`) for clients, holds
//! a shared connection to the `index` routing directory (`connections`), and
//! resolves each client's zones to their owning shard (`index`) before relaying
//! the live zone rows back.
//!
//! Two client-facing capabilities today (the rest of the old gateway's surface —
//! worldgen, recipes, content/LOD — is not ported):
//!   * **login** — `claim_or_login` against the `players` auth DB, binding the
//!     connection to a `player_id`;
//!   * **zone subscription** — resolve `zone_id → region → shard` via the index,
//!     connect to that shard, and stream its `cold_zones` + `hot_*` rows.

mod bindings;
mod config;
mod connections;
mod index;
mod protocol;
mod worldgen;
mod ws;

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tokio::signal;

use crate::config::ServerConfig;
use crate::connections::Pool;

/// Address the server listens on. `0.0.0.0` so the published container port
/// reaches it; override with `SERVER_LISTEN`.
const DEFAULT_LISTEN: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() {
    init_tracing();

    let listen = std::env::var("SERVER_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());

    let cfg = ServerConfig::from_env();
    tracing::info!(uri = %cfg.uri, env = %cfg.env, "server config");

    // The shared index connection is a hard startup dependency: without the
    // routing directory the server can't resolve a single zone. Fail fast.
    let pool = match Pool::connect(cfg).await {
        Ok(p) => p,
        Err(err) => {
            tracing::error!(%err, "failed to bring up index; exiting");
            std::process::exit(1);
        }
    };

    // Make this server discoverable: register in the index and heartbeat so the
    // gateway can allocate it to players (and the GC doesn't reap it).
    pool.spawn_registration();

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws::handler))
        .with_state(pool);

    let listener = match TcpListener::bind(&listen).await {
        Ok(l) => l,
        Err(err) => {
            tracing::error!(%listen, %err, "failed to bind listener");
            std::process::exit(1);
        }
    };

    let local = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| listen.clone());
    tracing::info!(addr = %local, "server listening");

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!(%err, "server error");
        std::process::exit(1);
    }

    tracing::info!("server stopped");
}

/// Liveness probe — `200 OK` with a tiny body. Used by compose healthchecks
/// and manual `curl`.
async fn health() -> &'static str {
    "ok"
}

/// Initialize `tracing` as the single logging path (no bare `println!`). Level
/// is controlled by `RUST_LOG`, defaulting to `info`.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

/// Resolve when either SIGINT (Ctrl-C) or SIGTERM (`compose stop`) arrives,
/// triggering graceful shutdown of in-flight connections.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(err) => tracing::error!(%err, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    tracing::info!("shutdown signal received");
}
