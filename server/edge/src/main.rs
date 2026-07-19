//! resonantdust world server — the bridge between clients and the SpacetimeDB
//! data shards. It serves an inbound WebSocket listener (`ws`) for clients, holds
//! a shared connection to the `index` routing directory (`connections`), and
//! serves the content + texture corpora over HTTP.
//!
//! Client-facing capabilities today:
//!   * **login** — `claim_or_login` against the `players` auth DB, binding the
//!     connection to a `player_id`;
//!   * **clock sync** — the `ping`/`pong` round trip the client seeds its offset from;
//!   * **assets** — `/content` + `/textures` (disk or R2), hot-reloaded from the tree.
//!
//! **Zone subscription is gone**, along with the shard module and tick pipeline it
//! streamed from — deleted deliberately for a ground-up rebuild
//! (`docs/intent/spacetime-again/`). What survived and still waits for it: the `index`
//! routing chain (see `index`) and `worldgen`, which has no seeding path until a shard
//! exists to seed.

mod bindings;
mod config;
mod connections;
mod content;
mod protocol;
mod tex_manifest;
mod textures;
mod worldgen;
mod ws;

use std::sync::Arc;

use axum::extract::{FromRef, Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::cors::CorsLayer;

use crate::config::{R2Settings, ServerConfig};
use crate::content::{ContentSource, ContentStore, R2Config};
use crate::connections::Pool;
use crate::tex_manifest::TextureManifest;
use crate::textures::TextureSource;

/// The server's shared HTTP state: the game `Pool` plus the (optional) content
/// store and texture source it serves. `FromRef` lets the WS / health handlers
/// keep extracting just `Arc<Pool>` while the asset routes read the whole state.
#[derive(Clone)]
struct AppState {
    pool: Arc<Pool>,
    /// `None` when content serving is disabled (initial load failed) — the
    /// `/content*` handlers then answer `503` while `/ws` keeps working.
    content: Option<Arc<ContentStore>>,
    /// The texture source (disk / R2); always `Some` in practice (a disk source is
    /// just a path — a miss surfaces per-request as a `404`).
    textures: Option<Arc<TextureSource>>,
    /// The texture LOD manifest (hash + max size + generated LODs per stem). `Some`
    /// whenever `textures` is; empty for an R2 source (its manifest is future work).
    tex_manifest: Option<Arc<TextureManifest>>,
}

impl FromRef<AppState> for Arc<Pool> {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

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

    // Watch the content tree and hot-reload worldgen on a change, so an edited
    // biome / tile / thing applies to newly generated zones without a restart
    // (already-seeded zones are stored and unaffected). Disabled with
    // RD_CONTENT_POLL_SECS=0.
    pool.spawn_content_poll(pool.cfg.content_poll_secs);

    // The server is also the ASSET host the client fetches from once it logs in:
    // the DSL corpus (`/content`) it renders with, and the texture masters/previews
    // (`/textures`). Best-effort — a failed content load disables `/content*` but
    // leaves the game (`/ws`) serving. (Ported from the gateway: assets belong to
    // the world server, not the thin routing gateway.)
    let content = build_content_store(&pool.cfg).await;
    let textures = build_texture_source(&pool.cfg);
    // The texture manifest indexes the masters (hash + max size) so the client builds
    // LOD URLs from it and never speculatively 404s R2. A background poll re-scans for
    // re-mastered assets; LOD generation bumps its version in-band.
    let tex_manifest = textures.as_ref().map(|src| Arc::new(TextureManifest::build(src)));
    if let (Some(m), Some(src)) = (&tex_manifest, &textures) {
        tex_manifest::spawn_poll(m.clone(), src.clone(), pool.cfg.content_poll_secs);
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws::handler))
        .route("/content", get(serve_content))
        .route("/content-version", get(serve_content_version))
        .route("/content/refresh", post(refresh_content))
        .route("/textures/master/{*stem}", get(serve_master))
        .route("/textures/preview/{*stem}", get(serve_preview))
        .route("/textures/meta/{*stem}", get(serve_meta))
        .route("/textures/lod/{hash}/{size}/{map}/{*stem}", get(serve_lod))
        .route("/textures-manifest", get(serve_texture_manifest))
        .route("/textures-manifest-version", get(serve_texture_manifest_version))
        .with_state(AppState { pool, content, textures, tex_manifest })
        // Assets are consumed by the browser client on another origin (the pixijs
        // dev server / deployed site), so `/content` + `/textures` must be
        // cross-origin readable. `expose_headers(ETag)` lets the texture client
        // read the ETag off a cross-origin response for its IndexedDB revalidation.
        .layer(CorsLayer::permissive().expose_headers([ETAG]));

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

/// Build the content store from config, or `None` if the initial load fails.
/// Spawns the background re-poll on success. `disk` reads `content_dir`; `r2`
/// pulls from the asset bucket. A failure is logged, not fatal — the game stream
/// keeps serving.
async fn build_content_store(cfg: &ServerConfig) -> Option<Arc<ContentStore>> {
    let source = match cfg.content_source.as_str() {
        "r2" => ContentSource::R2(r2_config(&cfg.r2)),
        _ => ContentSource::Disk(std::path::PathBuf::from(&cfg.content_dir)),
    };
    let label = source.label();
    match ContentStore::load(source).await {
        Ok(store) => {
            tracing::info!(source = %label, version = %store.version_hex(), poll_secs = cfg.content_poll_secs, "content serving enabled");
            content::spawn_poll(store.clone(), cfg.content_poll_secs);
            Some(store)
        }
        Err(err) => {
            tracing::warn!(source = %label, %err, "content serving disabled (initial load failed)");
            None
        }
    }
}

/// Build the texture source from config. `disk` points at the local texture tree
/// (dev); `r2` streams from the asset bucket. Always `Some` — a disk source is
/// just a path, so nothing to fail at startup (a miss surfaces as a `404`).
fn build_texture_source(cfg: &ServerConfig) -> Option<Arc<TextureSource>> {
    let source = match cfg.texture_source.as_str() {
        "r2" => TextureSource::R2(r2_config(&cfg.r2)),
        _ => TextureSource::Disk {
            root: std::path::PathBuf::from(&cfg.texture_dir),
            cache: std::path::PathBuf::from(&cfg.texture_cache_dir),
        },
    };
    tracing::info!(source = %source.label(), "texture serving enabled");
    Some(Arc::new(source))
}

/// Map the config's [`R2Settings`] to the content/texture modules' [`R2Config`].
fn r2_config(r2: &R2Settings) -> R2Config {
    R2Config {
        endpoint: r2.endpoint.clone(),
        bucket: r2.bucket.clone(),
        region: r2.region.clone(),
        prefix: r2.prefix.clone(),
        access_key: r2.access_key.clone(),
        secret_key: r2.secret_key.clone(),
    }
}

/// `GET /content` — the DSL corpus JSON the client renders with.
async fn serve_content(State(state): State<AppState>) -> impl IntoResponse {
    match &state.content {
        Some(store) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            store.payload_json(),
        )
            .into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "content unavailable").into_response(),
    }
}

/// `GET /content-version` — the cheap corpus fingerprint the client polls.
async fn serve_content_version(State(state): State<AppState>) -> impl IntoResponse {
    match &state.content {
        Some(store) => (StatusCode::OK, store.version_hex()).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "content unavailable").into_response(),
    }
}

/// `POST /content/refresh` — force an immediate source re-poll (e.g. right after
/// `dsl upload`), rather than waiting for the background tick.
async fn refresh_content(State(state): State<AppState>) -> impl IntoResponse {
    let Some(store) = &state.content else {
        return (StatusCode::SERVICE_UNAVAILABLE, "content unavailable").into_response();
    };
    match store.refresh().await {
        Ok(changed) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            format!("{{\"changed\":{changed},\"version\":\"{}\"}}\n", store.version_hex()),
        )
            .into_response(),
        Err(err) => {
            tracing::warn!(%err, "content refresh failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "content refresh failed").into_response()
        }
    }
}

/// `GET /textures/master/{*stem}` — the full-res master albedo for a stem.
async fn serve_master(State(state): State<AppState>, headers: HeaderMap, Path(stem): Path<String>) -> impl IntoResponse {
    texture_response(&state, &stem, TextureTier::Master, &headers).await
}

/// `GET /textures/preview/{*stem}` — the half-res preview, derived from the master.
async fn serve_preview(State(state): State<AppState>, headers: HeaderMap, Path(stem): Path<String>) -> impl IntoResponse {
    texture_response(&state, &stem, TextureTier::Preview, &headers).await
}

/// `GET /textures/meta/{*stem}` — the stem's `meta.json` (channel tints + shadow outline), on demand.
async fn serve_meta(State(state): State<AppState>, headers: HeaderMap, Path(stem): Path<String>) -> impl IntoResponse {
    texture_response(&state, &stem, TextureTier::Meta, &headers).await
}

/// `GET /textures/lod/{hash}/{size}/{map}/{*stem}` — one LOD (short axis `size` px) of a
/// stem's `map` (albedo|normal|layers|surface|…), derived from that map's master and clamped
/// to it. The `hash` is validated against the current master: a stale (re-mastered) or
/// oversize request `404`s, so the client refetches the manifest and retries with the fresh
/// hash. A stem whose leaf lacks the requested map (e.g. no `normal.png`) is a clean `404`
/// the client falls back for. The URL is content-addressed, so a hit is
/// `immutable`-cacheable — no revalidation. (The hash/max-size are keyed off the albedo
/// master; the sibling maps share its dimensions, authored at matched resolution.)
async fn serve_lod(
    State(state): State<AppState>,
    Path((hash, size, map, stem)): Path<(String, u32, String, String)>,
) -> impl IntoResponse {
    let (Some(source), Some(manifest)) = (&state.textures, &state.tex_manifest) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "textures unavailable").into_response();
    };
    if !textures::MAPS.contains(&map.as_str()) {
        return (StatusCode::NOT_FOUND, "unknown map").into_response();
    }
    match manifest.lookup(&stem) {
        Some((current, max_size)) if current == hash && size <= max_size => {}
        _ => return (StatusCode::NOT_FOUND, "stale or unknown lod").into_response(),
    }
    match source.serve_lod(&stem, size, &map).await {
        Ok(Some(asset)) => {
            manifest.note_generated(&stem, size);
            (
                StatusCode::OK,
                [
                    (CONTENT_TYPE, HeaderValue::from_static(asset.content_type)),
                    (CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable")),
                ],
                asset.bytes,
            )
                .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "texture not found").into_response(),
        Err(err) => {
            tracing::warn!(%stem, %err, "lod serve failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "texture error").into_response()
        }
    }
}

/// `GET /textures-manifest` — the LOD index (hash + max size + generated LODs per
/// stem) the client builds every LOD URL from.
async fn serve_texture_manifest(State(state): State<AppState>) -> impl IntoResponse {
    match &state.tex_manifest {
        Some(m) => (StatusCode::OK, [(CONTENT_TYPE, "application/json")], m.payload_json()).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "manifest unavailable").into_response(),
    }
}

/// `GET /textures-manifest-version` — the cheap manifest fingerprint the client polls.
async fn serve_texture_manifest_version(State(state): State<AppState>) -> impl IntoResponse {
    match &state.tex_manifest {
        Some(m) => (StatusCode::OK, m.version_hex()).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "manifest unavailable").into_response(),
    }
}

/// Which resolution of a stem to serve.
enum TextureTier {
    Master,
    Preview,
    /// The `meta.json` sidecar (channel tints + shadow outline) — served on demand (JSON, not a PNG).
    Meta,
}

/// Shared body of the two texture routes: honour a conditional `If-None-Match`
/// (→ `304`, skipping any derivation), else resolve the stem at `tier` and frame
/// the bytes / miss / error — tagging a hit with the master's `ETag`.
async fn texture_response(state: &AppState, stem: &str, tier: TextureTier, headers: &HeaderMap) -> Response {
    let Some(source) = &state.textures else {
        return (StatusCode::SERVICE_UNAVAILABLE, "textures unavailable").into_response();
    };

    // The stem's ETag is its master's identity (both tiers derive from it). If the
    // client already holds it, short-circuit to 304 before reading / downscaling.
    let etag = source.etag(stem);
    if let Some(tag) = &etag {
        if headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(tag.as_str()) {
            return (StatusCode::NOT_MODIFIED, [(ETAG, tag.clone())]).into_response();
        }
    }

    let served = match tier {
        TextureTier::Master => source.serve_master(stem).await,
        TextureTier::Preview => source.serve_preview(stem).await,
        TextureTier::Meta => source.serve_meta(stem).await,
    };
    match served {
        Ok(Some(asset)) => {
            let mut resp = (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, asset.content_type)],
                asset.bytes,
            )
                .into_response();
            if let Some(v) = etag.and_then(|t| HeaderValue::from_str(&t).ok()) {
                resp.headers_mut().insert(ETAG, v);
            }
            resp
        }
        Ok(None) => (StatusCode::NOT_FOUND, "texture not found").into_response(),
        Err(err) => {
            tracing::warn!(%stem, %err, "texture serve failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "texture error").into_response()
        }
    }
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
