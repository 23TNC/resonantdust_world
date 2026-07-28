//! master — the simulation metronome.
//!
//! The tic's one durable home is `index.master_clock` (per realm), NOT the shard clocks. The master:
//!   1. runs the metronome — every `1/TIC_HZ` it calls `index.bump_tic(realm)`, so the authoritative
//!      counter lives entirely in that row and the master holds none of its own (a restart resumes
//!      from the durable value, never resets);
//!   2. copies that tic into the SpacetimeDB *modules* — the event/data/pawn shards **and the cold
//!      shards (tile/thing)** can't subscribe cross-database, so the master alone `bump`s every one
//!      of their local `clock` mirrors, then sweeps (`settle` terminal events, `gc` old rows).
//!
//! Every *other* server (orchestrator, worker, edge) is an SDK client and reads the tic straight from
//! its `index.master_clock` subscription — the subscription push is their fan-out, so the master
//! never has to know they exist.
//!
//! **Self-healing (sim-self-heal P2).** Every upstream is an [`Uplink`]: built best-effort, rebuilt
//! on next use with capped backoff, never panicking. A pass skips only the work whose uplink is
//! dead (a down cold shard doesn't stop the event shard's settle); a dead index skips the pass (no
//! authority to fan). `set_orchestrator` re-stamps on every event-shard RECONNECT (generation
//! change) — a republished shard forgets its orchestrator and would reject every queue until then.
//!
//! `master_clock.tic` is a `u32` absolute counter; the shards take its low 16 bits as their wrapping
//! `master_tic`. The truncation happens only here, at the fan-out boundary.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use spacetimedb_sdk::DbContext;

use resonantdust_st_bindings::{data_shard, event_shard, index, pawn, thing, tile};
// Reducer + table-access traits (method resolution keys off the connection type).
use data_shard::{bump as _, gc as _};
use event_shard::{bump as _, set_orchestrator as _, settle as _};
use index::{bump_tic as _, MasterClockTableAccess as _};
use pawn::{bump as _, gc as _};
use thing::bump as _;
use tile::bump as _;

use resonantdust_uplink::Uplink;

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

/// Stamp a per-module `Uplink` build closure: connection + `alive` flag wired into
/// `on_connect_error` AND `on_disconnect` (the SDK never auto-reconnects; the flag is how a
/// dead conn is noticed). One macro because each module's `DbConnection` is a distinct type.
macro_rules! shard_uplink {
    ($module:ident, $name:literal, $uri:expr, $db:expr) => {{
        let uri = $uri.to_string();
        let db = $db.to_string();
        Uplink::new($name, move || {
            let alive = Arc::new(AtomicBool::new(true));
            let (a1, a2) = (alive.clone(), alive.clone());
            let conn = $module::DbConnection::builder()
                .with_uri(&uri)
                .with_database_name(&db)
                .on_connect(|_c, id, _t| tracing::info!(%id, concat!($name, " connected")))
                .on_connect_error(move |_c, err| {
                    tracing::warn!(%err, concat!($name, " connect error"));
                    a1.store(false, Ordering::Release);
                })
                .on_disconnect(move |_c, err| {
                    tracing::warn!(?err, concat!($name, " disconnected"));
                    a2.store(false, Ordering::Release);
                })
                .build()
                .map_err(|e| e.to_string())?;
            conn.run_threaded();
            Ok((Arc::new(conn), alive))
        })
    }};
}

/// `get()` an uplink, logging only on up/down TRANSITIONS (the uplink's own backoff already
/// paces its warn lines; a 6 Hz loop must not add one per tic).
async fn acquire<C, S>(up: &Uplink<C, S>, was_up: &mut bool, name: &str) -> Option<Arc<C>> {
    match up.get().await {
        Ok(conn) => {
            if !*was_up {
                tracing::info!(%name, "uplink up — resuming its work");
            }
            *was_up = true;
            Some(conn)
        }
        Err(err) => {
            if *was_up {
                tracing::warn!(%name, %err, "uplink down — skipping its work until it heals");
            }
            *was_up = false;
            None
        }
    }
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
    let tic_hz: f64 = env_or("TIC_HZ", &resonantdust_codec::tic::TIC_HZ.to_string()).parse().unwrap_or(resonantdust_codec::tic::TIC_HZ as f64);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    let gc_every: u32 = env_or("GC_EVERY", "20").parse().unwrap_or(20);
    // How far behind `master_tic` the GC horizon sits — old settled rows past it are reaped. Must
    // stay well under TIC_WINDOW (32767).
    let gc_behind: u16 = env_or("GC_BEHIND", "64").parse().unwrap_or(64);
    let orchestrator = parse_u8(&env_or("ORCHESTRATOR", "0x61"), 0x61);
    let index_db = env_or("INDEX_DB", "resonantdust-dev-index-0");
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    let pawn_db = env_or("PAWN_DB", "resonantdust-dev-pawn-0");
    let tile_db = env_or("TILE_DB", "resonantdust-dev-tile-0");
    let thing_db = env_or("THING_DB", "resonantdust-dev-thing-0");
    tracing::info!(%uri, realm, tic_hz, %index_db, %event_db, %data_db, %pawn_db, %tile_db, %thing_db, "master starting");

    // ── the index: the tic authority. Its uplink carries the `master_clock` subscription
    // (subscribe-and-wait), so a `get()`ed connection always has the row mirrored. ────────────
    let index_up: Uplink<index::DbConnection, index::SubscriptionHandle> = {
        let uri = uri.clone();
        let db = index_db.clone();
        Uplink::with_subscription(
            "index",
            move || {
                let alive = Arc::new(AtomicBool::new(true));
                let (a1, a2) = (alive.clone(), alive.clone());
                let conn = index::DbConnection::builder()
                    .with_uri(&uri)
                    .with_database_name(&db)
                    .on_connect(|_c, id, _t| tracing::info!(%id, "index connected"))
                    .on_connect_error(move |_c, err| {
                        tracing::warn!(%err, "index connect error");
                        a1.store(false, Ordering::Release);
                    })
                    .on_disconnect(move |_c, err| {
                        tracing::warn!(?err, "index disconnected");
                        a2.store(false, Ordering::Release);
                    })
                    .build()
                    .map_err(|e| e.to_string())?;
                conn.run_threaded();
                Ok((Arc::new(conn), alive))
            },
            move |conn, alive| {
                Box::pin(async move {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
                    let a3 = alive.clone();
                    let handle = conn
                        .subscription_builder()
                        .on_applied(move |_| {
                            let _ = tx.send(());
                        })
                        .on_error(move |_ctx, err| {
                            tracing::warn!(%err, "index subscription error");
                            a3.store(false, Ordering::Release);
                        })
                        .subscribe([format!("SELECT * FROM master_clock WHERE realm = {realm}")]);
                    tokio::time::timeout(Duration::from_secs(5), rx.recv())
                        .await
                        .map_err(|_| "master_clock subscription apply timed out".to_string())?
                        .ok_or_else(|| "subscription channel closed".to_string())?;
                    Ok(handle)
                })
            },
        )
    };

    // ── the shard call surfaces (sub-less). ──────────────────────────────────────────────────
    let event_up: Uplink<event_shard::DbConnection> = shard_uplink!(event_shard, "event_shard", uri, event_db);
    let data_up: Uplink<data_shard::DbConnection> = shard_uplink!(data_shard, "data_shard", uri, data_db);
    let pawn_up: Uplink<pawn::DbConnection> = shard_uplink!(pawn, "pawn", uri, pawn_db);
    let tile_up: Uplink<tile::DbConnection> = shard_uplink!(tile, "tile", uri, tile_db);
    let thing_up: Uplink<thing::DbConnection> = shard_uplink!(thing, "thing", uri, thing_db);

    // Best-effort warm-up: a down upstream logs and is retried in the loop (F2 — startup and
    // mid-run recovery are the same code path; no panic, no special phase).
    for (name, ok) in [
        ("index", index_up.get().await.is_ok()),
        ("event_shard", event_up.get().await.is_ok()),
        ("data_shard", data_up.get().await.is_ok()),
        ("pawn", pawn_up.get().await.is_ok()),
        ("tile", tile_up.get().await.is_ok()),
        ("thing", thing_up.get().await.is_ok()),
    ] {
        if !ok {
            tracing::warn!(%name, "not reachable at startup; will keep retrying");
        }
    }
    tracing::info!("metronome starting (uplinks lazy — a dead upstream skips only its own work)");

    // The master owns NO counter — `last_fanned` is only a dedup so we push each tic to the shards
    // once. A restart re-derives everything from the durable row (last_fanned starts None ⇒ the
    // first observed tic is re-pushed, restoring any reset shard).
    let mut last_fanned: Option<u32> = None;
    let mut since_gc: u32 = 0;
    // `set_orchestrator` re-stamps per event-shard CONNECTION (a republished shard resets the
    // assignment and would reject every `queue` until re-stamped).
    let mut orchestrator_stamped_gen: u64 = 0;
    // Transition trackers so a dead upstream logs once, not once per tic.
    let (mut up_i, mut up_e, mut up_d, mut up_p, mut up_t, mut up_h) = (true, true, true, true, true, true);

    let mut ticker = tokio::time::interval(period);
    loop {
        ticker.tick().await;

        // No index ⇒ no authority to advance or fan — skip the whole pass.
        let Some(index) = acquire(&index_up, &mut up_i, "index").await else { continue };

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
        // Each shard's fan-out rides its own uplink — a dead one skips only itself.
        let tic16 = tic as u16;
        if let Some(event) = acquire(&event_up, &mut up_e, "event_shard").await {
            // Standup-per-connection: (re)stamp the orchestrator on a fresh event-shard conn.
            let gen = event_up.generation();
            if gen != orchestrator_stamped_gen {
                match event.reducers().set_orchestrator(orchestrator) {
                    Ok(()) => {
                        orchestrator_stamped_gen = gen;
                        tracing::info!(orchestrator = format!("{orchestrator:#04x}"), "assigned orchestrator to event_shard");
                    }
                    Err(err) => tracing::warn!(%err, orchestrator, "set_orchestrator failed"),
                }
            }
            if let Err(err) = event.reducers().bump(tic16) {
                tracing::warn!(%err, tic = tic16, "event_shard bump failed");
            }
            // A tic seals once no append can reach it: appends land at master + 3, so `tic16 - 3`.
            if let Err(err) = event.reducers().settle(tic16.wrapping_sub(3)) {
                tracing::warn!(%err, "settle failed");
            }
        }
        if let Some(data) = acquire(&data_up, &mut up_d, "data_shard").await {
            if let Err(err) = data.reducers().bump(tic16) {
                tracing::warn!(%err, tic = tic16, "data_shard bump failed");
            }
            if since_gc + 1 >= gc_every {
                if let Err(err) = data.reducers().gc(tic16.wrapping_sub(gc_behind)) {
                    tracing::warn!(%err, "gc failed");
                }
            }
        }
        if let Some(pawn) = acquire(&pawn_up, &mut up_p, "pawn").await {
            if let Err(err) = pawn.reducers().bump(tic16) {
                tracing::warn!(%err, tic = tic16, "pawn shard bump failed");
            }
            if since_gc + 1 >= gc_every {
                if let Err(err) = pawn.reducers().gc(tic16.wrapping_sub(gc_behind)) {
                    tracing::warn!(%err, "pawn gc failed");
                }
            }
        }
        if let Some(tile) = acquire(&tile_up, &mut up_t, "tile").await {
            if let Err(err) = tile.reducers().bump(tic16) {
                tracing::warn!(%err, tic = tic16, "tile shard bump failed");
            }
        }
        if let Some(thing) = acquire(&thing_up, &mut up_h, "thing").await {
            if let Err(err) = thing.reducers().bump(tic16) {
                tracing::warn!(%err, tic = tic16, "thing shard bump failed");
            }
        }

        since_gc += 1;
        if since_gc >= gc_every {
            since_gc = 0;
        }

        tracing::debug!(tic, tic16, "fanned out");
    }
}
