//! master — the simulation metronome.
//!
//! The tic's one durable home is `index.master_clock` (per realm), NOT the shard clocks. The master:
//!   1. runs the metronome — every `1/TIC_HZ` it calls `index.bump_tic(realm)`, so the authoritative
//!      counter lives entirely in that row and the master holds none of its own (a restart resumes
//!      from the durable value, never resets);
//!   2. copies that tic into the SpacetimeDB *modules* — the event/data shards **and the cold shards
//!      (tile/thing)** can't subscribe cross-database, so the master alone `bump`s every one of their
//!      local `clock` mirrors, then sweeps (`settle` terminal events, `gc` old rows). The cold shards
//!      need the tic just as much: their `state_log`/`state` overlay and the `tic` on the baseline
//!      `cold_tile`/`cold_thing` rows all order by it.
//!
//! Every *other* server (orchestrator, worker, edge) is an SDK client and reads the tic straight from
//! its `index.master_clock` subscription — the subscription push is their fan-out, so the master
//! never has to know they exist. It also assigns each event shard its orchestrator at standup.
//!
//! `master_clock.tic` is a `u32` absolute counter; the shards take its low 16 bits as their wrapping
//! `master_tic`. The truncation happens only here, at the fan-out boundary.

use std::time::Duration;

use spacetimedb_sdk::DbContext;

use resonantdust_st_bindings::{data_shard, event_shard, index, thing, tile};
// Reducer + table-access traits (method resolution keys off the connection type).
use data_shard::{bump as _, gc as _};
use event_shard::{bump as _, set_orchestrator as _, settle as _};
use index::{bump_tic as _, MasterClockTableAccess as _};
use thing::bump as _;
use tile::bump as _;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parse a `u8` written either `0x61` or decimal.
fn parse_u8(s: &str, default: u8) -> u8 {
    s.strip_prefix("0x")
        .and_then(|h| u8::from_str_radix(h, 16).ok())
        .or_else(|| s.parse().ok())
        .unwrap_or(default)
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
    let realm = parse_u8(&env_or("REALM", "0"), 0);
    let tic_hz: f64 = env_or("TIC_HZ", "2").parse().unwrap_or(2.0);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    let gc_every: u32 = env_or("GC_EVERY", "20").parse().unwrap_or(20);
    // How far behind `master_tic` the GC horizon sits — old settled rows past it are reaped. Must
    // stay well under TIC_WINDOW (32767).
    let gc_behind: u16 = env_or("GC_BEHIND", "64").parse().unwrap_or(64);
    let orchestrator = parse_u8(&env_or("ORCHESTRATOR", "0x61"), 0x61);
    let index_db = env_or("INDEX_DB", "resonantdust-dev-index-0");
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    // Cold shards ride the same clock — their `state_log`/`state` overlay + the `tic` on cold rows
    // need the tic to advance, so the master fans out to them too.
    let tile_db = env_or("TILE_DB", "resonantdust-dev-tile-0");
    let thing_db = env_or("THING_DB", "resonantdust-dev-thing-0");
    tracing::info!(%uri, realm, tic_hz, %index_db, %event_db, %data_db, %tile_db, %thing_db, "master starting");

    // ── index: the tic authority. Subscribe our realm's row and wait for it before ticking. ──────
    let (tx_i, rx_i) = std::sync::mpsc::channel::<()>();
    let index = index::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&index_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "index connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "index connect error"))
        .build()
        .expect("build index connection");
    index.run_threaded();
    index
        .subscription_builder()
        .on_applied(move |_| {
            let _ = tx_i.send(());
        })
        .subscribe([format!("SELECT * FROM master_clock WHERE realm = {realm}")]);

    // ── the shard modules: call surfaces for bump / settle / gc (no subscription needed). ────────
    let event = event_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&event_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "event_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "event_shard connect error"))
        .build()
        .expect("build event_shard connection");
    event.run_threaded();

    let data = data_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&data_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "data_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "data_shard connect error"))
        .build()
        .expect("build data_shard connection");
    data.run_threaded();

    let tile = tile::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&tile_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "tile shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "tile shard connect error"))
        .build()
        .expect("build tile shard connection");
    tile.run_threaded();

    let thing = thing::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&thing_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "thing shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "thing shard connect error"))
        .build()
        .expect("build thing shard connection");
    thing.run_threaded();

    rx_i
        .recv_timeout(Duration::from_secs(5))
        .expect("index master_clock subscription applied");

    // Stand up initial state: hand the event shard its orchestrator (idempotent).
    if let Err(err) = event.reducers().set_orchestrator(orchestrator) {
        tracing::warn!(%err, orchestrator, "set_orchestrator failed");
    } else {
        tracing::info!(orchestrator = format!("{orchestrator:#04x}"), "assigned orchestrator to event_shard");
    }

    let durable = index.db().master_clock().realm().find(&realm).map(|c| c.tic).unwrap_or(0);
    tracing::info!(durable_tic = durable, "index clock applied; metronome starting");

    // The master owns NO counter — `last_fanned` is only a dedup so we push each tic to the shards
    // once. A restart re-derives everything from the durable row (last_fanned starts None ⇒ the
    // first observed tic is re-pushed, restoring any reset shard).
    let mut last_fanned: Option<u32> = None;
    let mut since_gc: u32 = 0;

    let mut ticker = tokio::time::interval(period);
    loop {
        ticker.tick().await;

        // Advance the authority. The increment lands asynchronously; we fan out whatever the durable
        // row currently shows (≤ 1 tic behind), which keeps the shards a faithful mirror.
        if let Err(err) = index.reducers().bump_tic(realm) {
            tracing::warn!(%err, "bump_tic failed");
        }

        let tic = match index.db().master_clock().realm().find(&realm) {
            Some(c) => c.tic,
            None => continue, // row not yet delivered
        };
        if last_fanned == Some(tic) {
            continue; // already pushed this value; the increment hasn't landed yet
        }
        last_fanned = Some(tic);

        // Copy into the module mirrors (low 16 bits = the shard's wrapping ring), then sweep.
        let tic16 = tic as u16;
        if let Err(err) = event.reducers().bump(tic16) {
            tracing::warn!(%err, tic = tic16, "event_shard bump failed");
        }
        if let Err(err) = data.reducers().bump(tic16) {
            tracing::warn!(%err, tic = tic16, "data_shard bump failed");
        }
        if let Err(err) = tile.reducers().bump(tic16) {
            tracing::warn!(%err, tic = tic16, "tile shard bump failed");
        }
        if let Err(err) = thing.reducers().bump(tic16) {
            tracing::warn!(%err, tic = tic16, "thing shard bump failed");
        }

        // A tic seals once no append can reach it: appends land at master + 3, so `tic16 - 3`.
        let sealed = tic16.wrapping_sub(3);
        if let Err(err) = event.reducers().settle(sealed) {
            tracing::warn!(%err, "settle failed");
        }

        since_gc += 1;
        if since_gc >= gc_every {
            since_gc = 0;
            let horizon = tic16.wrapping_sub(gc_behind);
            if let Err(err) = data.reducers().gc(horizon) {
                tracing::warn!(%err, "gc failed");
            }
        }

        tracing::debug!(tic, tic16, "fanned out");
    }
}
