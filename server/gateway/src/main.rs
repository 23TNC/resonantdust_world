//! resonantdust gateway — the entrypoint clients hit *before* the game.
//!
//! A client doesn't connect straight to a world server; it first asks the
//! gateway for one. The gateway consults the `index` routing directory, picks
//! the proper world server for the player (reusing the server already holding
//! their session on reconnect, else allocating one from the live pool), and
//! returns that server's endpoint. The client then logs into that world server,
//! which becomes state-authoritative for the player *and* serves the client its
//! assets (`/content` + `/textures` live on the world `server`, not here).
//!
//! So the gateway is deliberately thin: a directory lookup over HTTP. It holds no
//! game state, no client stream, and no assets — that's the world `server`'s job.
//! Its only upstream is the `index` SpacetimeDB database (see [`directory`]).
//!
//! ## HTTP API
//! - `GET /`               — hello banner.
//! - `GET /health`         — liveness probe (`200 ok`).
//! - `GET /server`         — resolve a world server. Optional `?player_id=<u32>`
//!   for reconnect affinity (omitted / `0` ⇒ a client with no player yet).
//!   `200` → `{ "server": { "server_id", "url" }, "reused": <bool> }`;
//!   `503` when the directory is unavailable or no server is registered.

mod bindings;
mod config;
mod directory;
mod resolve;

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing::get, Json, Router};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::cors::CorsLayer;

use crate::config::GatewayConfig;
use crate::directory::{Directory, ResolveError};

#[tokio::main]
async fn main() {
    init_tracing();

    let cfg = GatewayConfig::from_env();
    tracing::info!(
        listen = %cfg.listen,
        index_uri = %cfg.index_uri,
        index_db = %cfg.index_db,
        "gateway starting",
    );

    let directory = Arc::new(Directory::new(cfg.clone()));
    // Best-effort eager connect so the first real request is warm.
    directory.warm_up().await;

    let app = Router::new()
        .route("/", get(hello))
        .route("/health", get(health))
        .route("/server", get(get_server))
        .with_state(directory)
        // The gateway is a public directory consumed by browser clients on other
        // origins (the pixijs dev server, the deployed site). Allow any origin so
        // a cross-origin `GET /server` can be read back — without this the browser
        // blocks the response and the client's fetch fails with "Failed to fetch".
        .layer(CorsLayer::permissive());

    let listener = match TcpListener::bind(&cfg.listen).await {
        Ok(l) => l,
        Err(err) => {
            tracing::error!(listen = %cfg.listen, %err, "failed to bind listener");
            std::process::exit(1);
        }
    };

    let local = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.listen.clone());
    tracing::info!(addr = %local, "gateway listening");

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!(%err, "gateway error");
        std::process::exit(1);
    }

    tracing::info!("gateway stopped");
}

/// Root handler — a hello banner pointing at the real endpoint.
async fn hello() -> &'static str {
    "resonantdust gateway — GET /server to acquire a world server\n"
}

/// Liveness probe — `200 OK`. Independent of the upstream `index` connection so
/// the gateway reports live even while the directory is briefly unreachable.
async fn health() -> &'static str {
    "ok"
}

/// `GET /server` query string.
#[derive(serde::Deserialize)]
struct ServerQuery {
    /// The player to route, when known (reconnect). Omitted or `0` for a client
    /// with no established player yet.
    player_id: Option<u32>,
}

/// Resolve a world server for the requesting player.
async fn get_server(
    State(directory): State<Arc<Directory>>,
    Query(q): Query<ServerQuery>,
) -> impl IntoResponse {
    match directory.resolve(q.player_id).await {
        Ok(resolved) => (StatusCode::OK, Json(resolved)).into_response(),
        Err(err) => {
            let (status, message) = match &err {
                ResolveError::Unavailable(detail) => {
                    tracing::warn!(%detail, "directory unavailable");
                    (StatusCode::SERVICE_UNAVAILABLE, "directory unavailable")
                }
                ResolveError::NoServers => {
                    (StatusCode::SERVICE_UNAVAILABLE, "no server available")
                }
            };
            (status, Json(serde_json::json!({ "error": message }))).into_response()
        }
    }
}

/// Initialize `tracing` as the single logging path. Level via `RUST_LOG`
/// (default `info`).
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

/// Resolve when either SIGINT (Ctrl-C) or SIGTERM (`compose stop`) arrives,
/// triggering graceful shutdown of in-flight requests.
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
