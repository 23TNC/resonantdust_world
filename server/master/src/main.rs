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

mod defs;

use std::time::Duration;

use spacetimedb_sdk::DbContext;

use resonantdust_st_bindings::{data_shard, event_shard, index, pawn, thing, tile};
// Reducer + table-access traits (method resolution keys off the connection type).
use data_shard::{bump as _, gc as _};
use event_shard::{bump as _, gc as _, set_orchestrator as _, settle as _};
use index::{bump_tic as _, MasterClockTableAccess as _};
use pawn::{bump as _, gc as _};
use thing::bump as _;
use tile::bump as _;

use resonantdust_uplink::{acquire, Uplink};

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
    // Where the corpus lives, for the definition-registry seed. Same var + default the edge uses,
    // so both read one tree.
    let content_dir = env_or("RD_CONTENT_DIR", "content");
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
    // `definitions` rides the same subscription (definition-registry P6): the master must see what
    // the registry ALREADY holds to decide whether a def is unchanged (keep its version) or changed
    // (bump). Without it `known_versions` reads an empty local cache, every def looks brand new,
    // and a data edit silently fails to bump — which is exactly what happened the first time.
    let index_up = resonantdust_uplink::subbed_uplink!(index, "index", uri, index_db,
        vec![
            format!("SELECT * FROM master_clock WHERE realm = {realm}"),
            "SELECT * FROM definitions".to_string(),
        ]);

    // ── the shard call surfaces (sub-less). ──────────────────────────────────────────────────
    let event_up: Uplink<event_shard::DbConnection> = resonantdust_uplink::uplink!(event_shard, "event_shard", uri, event_db);
    let data_up: Uplink<data_shard::DbConnection> = resonantdust_uplink::uplink!(data_shard, "data_shard", uri, data_db);
    let pawn_up: Uplink<pawn::DbConnection> = resonantdust_uplink::uplink!(pawn, "pawn", uri, pawn_db);
    let tile_up: Uplink<tile::DbConnection> = resonantdust_uplink::uplink!(tile, "tile", uri, tile_db);
    let thing_up: Uplink<thing::DbConnection> = resonantdust_uplink::uplink!(thing, "thing", uri, thing_db);

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
    // ── seed the definition registry (definition-registry P4) ───────────────────────────────
    //
    // The corpus DESCRIBES; the master NUMBERS (F11 — it is the single allocator, so there is no
    // boot race to coordinate around). Idempotent: `ensure_definition` no-ops on a row it already
    // holds, so this runs every boot and only writes what is genuinely new.
    //
    // Non-fatal by design, like every other upstream here: a down index or an unreadable corpus
    // logs and the metronome still starts. The registry is re-seeded on the next boot, and until
    // then every STORED id still decodes exactly as before (I6 — the read path never consults it).
    match resonantdust_content::content::read_content_dir(std::path::Path::new(&content_dir))
        .map_err(|e| e.to_string())
        .and_then(|srcs| {
            resonantdust_content::load(&srcs).map_err(|errs| {
                format!("{} error(s), first: {:?}", errs.len(), errs.first())
            })
        }) {
        Ok(bundle) => match index_up.get().await {
            Ok(index) => match defs::seed_registry(&index, &bundle).await {
                Ok(n) => tracing::info!(definitions = n, "definition registry seeded"),
                Err(err) => tracing::error!(%err, "definition registry NOT seeded — corpus will not expand"),
            },
            Err(err) => tracing::warn!(%err, "index down at boot; registry seed deferred to next boot"),
        },
        Err(err) => tracing::warn!(%err, dir = %content_dir, "corpus unreadable; registry seed skipped"),
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

    // ── REALTIME-paced metronome (movement-hardening P3, closing I5). The tic is a WALL-TIME
    // promise, but tokio sleeps ride CLOCK_MONOTONIC — measured running at 0.900× realtime in
    // this WSL2 VM (a sleep(60) takes 66.7 real seconds), which is exactly the 5.4-vs-6 Hz
    // deficit. So the grid lives in `SystemTime` (realtime) and each sleep is FEEDBACK-paced:
    // an overshooting sleep shortens the next one, converging on the realtime grid under any
    // monotonic skew. F4: no burst catch-up — falling behind by > 2 periods (suspend, hitch)
    // SNAPS the grid forward; the world never fast-forwards. A backwards realtime step (NTP)
    // re-bases the grid rather than stalling.
    let mut next_tick = std::time::SystemTime::now();
    // Integral controller on the MEASURED rate (the per-tick feedback alone measurably did
    // not converge in this VM stack — steady 5.44 Hz with the grid in realtime; wherever the
    // dilation hides, the achieved-Hz signal sees it): each report window scales the
    // effective period by achieved/target, clamped to [0.5×, 1.2×] authored. Converges on
    // ANY monotone dilation, adjusts smoothly (no bursts — F4).
    let mut period_eff = period;
    // Pacing report: achieved Hz in REALTIME over ~1-minute windows + server-confirmed bump
    // executions (the fire-and-forget diagnosis path that found the skew).
    let mut pace_n: u32 = 0;
    let mut pace_started = std::time::SystemTime::now();
    let bump_ok = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let bump_err = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    loop {
        next_tick += period_eff;
        let now = std::time::SystemTime::now();
        match next_tick.duration_since(now) {
            Ok(wait) => {
                if wait > period * 2 {
                    next_tick = now + period_eff; // realtime stepped BACKWARDS — re-base
                }
                tokio::time::sleep(next_tick.duration_since(std::time::SystemTime::now()).unwrap_or_default()).await;
            }
            Err(behind) => {
                if behind.duration() > period * 2 {
                    next_tick = now; // far behind — snap forward, never burst
                } else {
                    // Mildly behind: catch up at ≤ 1.2× rate (tick spacing ≥ ⅚ period) —
                    // instant no-sleep iterations were measured micro-bursting to 6.3 Hz
                    // windows, the F4 violation in miniature.
                    tokio::time::sleep(period_eff * 5 / 6).await;
                }
            }
        }

        pace_n += 1;
        if pace_n >= 360 {
            let span = std::time::SystemTime::now()
                .duration_since(pace_started)
                .map(|d| d.as_secs_f64())
                .unwrap_or(f64::NAN);
            let hz = pace_n as f64 / span;
            if hz.is_finite() && hz > 0.5 {
                let lo = 1.0 / (tic_hz * 1.2);
                let hi = 1.0 / (tic_hz * 0.5);
                // Half-gain (sqrt) — a single noisy window (a hitch, a snap) nudges rather
                // than yanks; converges in 2–3 windows either way.
                period_eff = std::time::Duration::from_secs_f64(
                    (period_eff.as_secs_f64() * (hz / tic_hz).sqrt()).clamp(lo, hi),
                );
            }
            tracing::info!(
                achieved_hz = format!("{:.3}", hz),
                period_eff_ms = format!("{:.1}", period_eff.as_secs_f64() * 1000.0),
                bump_confirmed = bump_ok.swap(0, std::sync::atomic::Ordering::Relaxed),
                bump_failed = bump_err.swap(0, std::sync::atomic::Ordering::Relaxed),
                "pacing report (realtime; target 6.000 Hz)"
            );
            pace_n = 0;
            pace_started = std::time::SystemTime::now();
        }

        // No index ⇒ no authority to advance or fan — skip the whole pass.
        let Some(index) = acquire(&index_up, &mut up_i, "index").await else { continue };

        // Advance the authority. The increment lands asynchronously; we fan out whatever the durable
        // row currently shows (≤ 1 tic behind), which keeps the shards a faithful mirror.
        // `_then`: count SERVER-CONFIRMED executions (P3/I5 — the loop provably calls at 6 Hz
        // but the durable counter advances ~5.4/s; fire-and-forget hides where calls die).
        {
            let ok = bump_ok.clone();
            let err_c = bump_err.clone();
            let sent = index.reducers().bump_tic_then(realm, move |_ctx, res| match res {
                Ok(Ok(())) => {
                    ok.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                other => {
                    err_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    tracing::warn!(err = %format!("{other:?}"), "bump_tic did not confirm");
                }
            });
            if let Err(err) = sent {
                tracing::warn!(%err, "bump_tic failed");
            }
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
            // Retention (pawn-movement I2): reap settled `event` rows past the same horizon the
            // hot shards gc on — a subscribe then replays only the recent window, never history.
            if since_gc + 1 >= gc_every {
                if let Err(err) = event.reducers().gc(tic16.wrapping_sub(gc_behind)) {
                    tracing::warn!(%err, "event gc failed");
                }
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
