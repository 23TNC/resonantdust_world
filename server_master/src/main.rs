//! server_master — the simulation metronome.
//!
//! Connects to each shard and, every `1/TIC_HZ`, advances its `master_tic` by one via
//! the `bump` reducer (which also work-generates the tic's pending rows; the workers
//! resolve them). `bump` is idempotent, so a call that races ahead of the previous
//! one's propagation is a harmless no-op — we read the authoritative `master_tic` off
//! the subscription each tick and request `master_tic + 1`, which self-corrects.
//!
//! Phase A: one object shard, config from env. The shard set and a coarse runaway
//! guard (`docs/simulation-plan.md`) come later.

use std::sync::Arc;
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

mod bindings;
use bindings::object_shard::{bump as _, DbConnection, TicMetaTableAccess};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server_master=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let db = env_or("OBJECT_DB", "resonantdust-dev-object-0");
    let tic_hz: f64 = env_or("TIC_HZ", "2").parse().unwrap_or(2.0);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    tracing::info!(%uri, %db, tic_hz, "server_master starting");

    let conn = DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&db)
        .on_connect(|_ctx, identity, _token| tracing::info!(%identity, "connected"))
        .on_connect_error(|_ctx, err| tracing::error!(%err, "connect error"))
        .on_disconnect(|_ctx, err| match err {
            Some(err) => tracing::warn!(%err, "disconnected"),
            None => tracing::info!("disconnected"),
        })
        .build()
        .expect("build connection");
    let conn = Arc::new(conn);
    conn.run_threaded();

    // Watch the authoritative tic so each bump targets exactly `master_tic + 1`.
    conn.subscription_builder()
        .on_applied(|_ctx| tracing::info!("tic_meta subscription applied"))
        .subscribe(["SELECT * FROM tic_meta"]);

    let mut ticker = tokio::time::interval(period);
    // A stall (worker/backpressure) would be handled by a runaway guard here later.
    loop {
        ticker.tick().await;
        let cur = conn
            .db()
            .tic_meta()
            .iter()
            .map(|m| m.master_tic)
            .max()
            .unwrap_or(0);
        match conn.reducers().bump(cur + 1) {
            Ok(()) => tracing::debug!(to = cur + 1, "bump"),
            Err(err) => tracing::warn!(%err, "bump failed"),
        }
    }
}
