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
use bindings::shard::{
    bump as _, drop_timed_out as _, tick_gc as _, DbConnection, TicMetaTableAccess,
};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "master=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let tic_hz: f64 = env_or("TIC_HZ", "2").parse().unwrap_or(2.0);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    // Run the GC sweep every this many tics (default 20 → ~10s at 2 Hz).
    let gc_every: u32 = env_or("GC_EVERY", "20").parse().unwrap_or(20);
    // Every shard this master drives, kept in LOCKSTEP so a tic number means the same
    // logical tic on all of them — the precondition for cross-shard reads (an actor on
    // shard B read at `tic T` by a target on shard A). `SHARDS="1=db_a,2=db_b"`; falls
    // back to a single `1=<OBJECT_DB>`. (Lockstep assumes the shards start aligned;
    // catch-up for a lagging shard is future work — reset a shard to realign.)
    let shard_cfg = parse_shards();
    tracing::info!(%uri, tic_hz, gc_every, shards = shard_cfg.len(), "master starting");

    let mut shards: Vec<Arc<DbConnection>> = Vec::new();
    for (_, db) in &shard_cfg {
        let conn = DbConnection::builder()
            .with_uri(&uri)
            .with_database_name(db)
            .on_connect({
                let db = db.clone();
                move |_ctx, identity, _token| tracing::info!(%identity, %db, "connected")
            })
            .on_connect_error(|_ctx, err| tracing::error!(%err, "connect error"))
            .on_disconnect(|_ctx, err| match err {
                Some(err) => tracing::warn!(%err, "disconnected"),
                None => tracing::info!("disconnected"),
            })
            .build()
            .expect("build connection");
        let conn = Arc::new(conn);
        conn.run_threaded();
        conn.subscription_builder()
            .subscribe(["SELECT * FROM tic_meta"]);
        shards.push(conn);
    }

    let mut ticker = tokio::time::interval(period);
    let mut since_gc: u32 = 0;
    loop {
        ticker.tick().await;
        for conn in &shards {
            // Debug freeze: while a shard's `tic_meta.paused` is set, don't advance its tic or
            // run its drop sweep — the whole simulation on that shard holds still (set via the
            // `set_paused` reducer, driven from the client's `/pause`).
            let meta = conn.db().tic_meta().iter().max_by_key(|m| m.master_tic);
            if meta.as_ref().map_or(false, |m| m.paused) {
                continue;
            }
            let cur = meta.map(|m| m.master_tic).unwrap_or(0);
            // The per-tic drop barrier: cancel stale enqueue/queueing rows *before* the tic
            // rolls (docs/spacetime-tables/lifecycle.md — causality guard, no watermark).
            let _ = conn.reducers().drop_timed_out();
            if let Err(err) = conn.reducers().bump(cur + 1) {
                tracing::warn!(%err, to = cur + 1, "bump failed");
            }
        }
        since_gc += 1;
        if since_gc >= gc_every {
            since_gc = 0;
            for conn in &shards {
                let _ = conn.reducers().tick_gc();
            }
        }
    }
}

/// Parse the shard set from `SHARDS="1=db_a,2=db_b"`; fall back to `1=<SHARD_DB>`.
fn parse_shards() -> Vec<(u16, String)> {
    let raw = std::env::var("SHARDS").unwrap_or_default();
    if raw.trim().is_empty() {
        return vec![(1, env_or("SHARD_DB", "resonantdust-dev-zone-0"))];
    }
    raw.split(',')
        .filter_map(|part| {
            let (id, db) = part.split_once('=')?;
            Some((id.trim().parse().ok()?, db.trim().to_string()))
        })
        .collect()
}
