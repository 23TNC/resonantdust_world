//! master — the simulation metronome.
//!
//! Connects to every shard (event + data) and, each `1/TIC_HZ`, advances `master_tic` by one via
//! each shard's `bump`, in lockstep — a tic number means the same logical tic everywhere, which is
//! what lets the orchestrator's completeness barrier (`master ≥ T-2`) mean the same thing on every
//! shard. It then sweeps: `settle` moves terminal events out of the event queue; `gc` drops old
//! settled `state_log` rows. Owns no state logic.
//!
//! `bump` writes an absolute `master_tic`, and the master is its sole writer, so a call is
//! self-correcting — we read the shard's current tic off the subscription and request `+1`.

use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_st_bindings::{data_shard, event_shard};
// Reducer + table-access traits (method resolution keys off the connection type).
use data_shard::{bump as _, gc as _, ClockTableAccess as _};
use event_shard::{bump as _, settle as _, ClockTableAccess as _};

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
    let gc_every: u32 = env_or("GC_EVERY", "20").parse().unwrap_or(20);
    // How far behind `master_tic` the GC horizon sits — old settled rows past it are reaped. Must
    // stay well under TIC_WINDOW (32767).
    let gc_behind: u16 = env_or("GC_BEHIND", "64").parse().unwrap_or(64);
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    tracing::info!(%uri, tic_hz, %event_db, %data_db, "master starting");

    // A subscription applies asynchronously, so the clock row isn't readable the instant we
    // subscribe. If we bumped before it landed we'd read 0 and RESET the tic — so wait for the
    // initial `on_applied`, seed the counter once, then own it locally.
    let (tx_e, rx_e) = std::sync::mpsc::channel::<()>();
    let event = event_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&event_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "event_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "event_shard connect error"))
        .build()
        .expect("build event_shard connection");
    event.run_threaded();
    event
        .subscription_builder()
        .on_applied(move |_| { let _ = tx_e.send(()); })
        .subscribe(["SELECT * FROM clock"]);

    let (tx_d, rx_d) = std::sync::mpsc::channel::<()>();
    let data = data_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&data_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "data_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "data_shard connect error"))
        .build()
        .expect("build data_shard connection");
    data.run_threaded();
    data
        .subscription_builder()
        .on_applied(move |_| { let _ = tx_d.send(()); })
        .subscribe(["SELECT * FROM clock"]);

    let applied = Duration::from_secs(5);
    rx_e.recv_timeout(applied).expect("event_shard clock subscription applied");
    rx_d.recv_timeout(applied).expect("data_shard clock subscription applied");

    // Seed once from the shard's persisted tic; from here the master owns the counter (it is the
    // sole writer), so it never re-reads and never resets on a lagging subscription.
    let mut tic = event.db().clock().iter().next().map(|c| c.master_tic).unwrap_or(0);
    tracing::info!(seed = tic, "clocks applied; metronome starting");

    let mut ticker = tokio::time::interval(period);
    let mut since_gc: u32 = 0;
    loop {
        ticker.tick().await;

        tic = tic.wrapping_add(1); // u16 ring — wraps, compared with tic:: serial math elsewhere
        let next = tic;

        if let Err(err) = event.reducers().bump(next) {
            tracing::warn!(%err, to = next, "event_shard bump failed");
        }
        if let Err(err) = data.reducers().bump(next) {
            tracing::warn!(%err, to = next, "data_shard bump failed");
        }

        // Settle terminal events through the tic that is now fully sealed. A tic seals once no new
        // event can target it: appends go to master + 3, so tic `next - 3` is sealed.
        let sealed = next.wrapping_sub(3);
        if let Err(err) = event.reducers().settle(sealed) {
            tracing::warn!(%err, "settle failed");
        }

        since_gc += 1;
        if since_gc >= gc_every {
            since_gc = 0;
            let horizon = next.wrapping_sub(gc_behind);
            if let Err(err) = data.reducers().gc(horizon) {
                tracing::warn!(%err, "gc failed");
            }
        }

        tracing::debug!(tic = next, "bumped");
    }
}
