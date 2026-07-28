//! orchestrator — the grouper.
//!
//! One coordinator per tic. It watches an event shard's queue (subscribed by its *own* reference —
//! `event_log WHERE orchestrator_reference = self`) and, each pass, takes every **frozen** still-
//! `QUEUED` event and partitions it into conflict-**components**: events are unioned when they share
//! a write target, so a whole transitive component lands on one worker. It then `assign`s a worker
//! per component on the event shard and `claim`s that component's entities on the data shard.
//!
//! **The completeness barrier.** A tic T's event set is frozen once `master ≥ T-2` (appends land at
//! `master + 3`, so nothing new can target T after that). The orchestrator only groups frozen tics,
//! so union-find always runs on a *complete* set — which makes the partition order-independent and
//! two orchestrators safe (§Why 2 in the intent). It never groups tic `master + 3` (still open).
//!
//! **Not one tic per pass.** Each pass drains *all* frozen unassigned events in the cache — a single
//! tic normally, a backlog of several if a pass ran long or on takeover. Workers likewise grind
//! their assigned chains up to the current tic, so a recovered worker clears multiple tics at once.
//!
//! Stateless: killed mid-tic, a fresh orchestrator recomputes the identical partition from the
//! shard-durable event set. Assign + claim are idempotent, so a doubled assignment is harmless.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_codec::action::{self, Route};
use resonantdust_codec::object::{TYPE_BIOME_THING, TYPE_BIOME_TILE};
use resonantdust_codec::status::{status_phase, EVENT_QUEUED};
use resonantdust_codec::tic::{tic_add, tic_after};
use resonantdust_st_bindings::{data_shard, event_shard, index, thing, tile};
use data_shard::claim as _;
use event_shard::{assign as _, EventLogTableAccess as _};
use index::MasterClockTableAccess as _;
// A cold slot is claimed on its shard + tier (F11): the baseline via `claim`, the `overlay` via
// `claim_overlay` — routed per target by `codec::target_routes`.
use thing::{claim as _, claim_overlay as _};
use tile::{claim as _, claim_overlay as _};

mod grouping;
use grouping::group;

/// A cold shard's claim key from its `type_id` (F11): `1` = tile, `2` = thing, `0` = the hot
/// `data_shard`. Cold routing is **action-derived** (`codec::target_routes`), not read off the target —
/// a `cold_row_reference` carries no type nibble, so the action names its shard's `type_id`.
fn shard_key_of_type(type_id: u8) -> u8 {
    match type_id {
        TYPE_BIOME_TILE => 1,
        TYPE_BIOME_THING => 2,
        _ => 0,
    }
}

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
                .unwrap_or_else(|_| "orchestrator=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let self_ref = parse_u8(&env_or("ORCHESTRATOR", "0x61"), 0x61);
    let realm = parse_u8(&env_or("REALM", "0"), 0);
    let workers: Vec<u8> = env_or("WORKERS", "0x62")
        .split(',')
        .map(|s| parse_u8(s.trim(), 0x62))
        .collect();
    let tic_hz: f64 = env_or("TIC_HZ", "6").parse().unwrap_or(6.0);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    let index_db = env_or("INDEX_DB", "resonantdust-dev-index-0");
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    // Cold shards get their slots claimed here too when a cold-cell mutation is grouped.
    let tile_db = env_or("TILE_DB", "resonantdust-dev-tile-0");
    let thing_db = env_or("THING_DB", "resonantdust-dev-thing-0");
    tracing::info!(%uri, self_ref = format!("{self_ref:#04x}"), realm, ?workers, %index_db, %event_db, %data_db, %tile_db, %thing_db, "orchestrator starting");

    // ── index: the canonical tic. Every SDK-client server reads it here, not off a shard clock. ──
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

    // ── event shard: subscribe our own queue slice (tic comes from index, above) ─────
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
        .on_applied(move |_| {
            let _ = tx_e.send(());
        })
        .subscribe([format!(
            "SELECT * FROM event_log WHERE orchestrator_reference = {self_ref}"
        )]);

    // ── data + cold shards: no subscription, just a call surface for `claim` ────────
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

    rx_e
        .recv_timeout(Duration::from_secs(5))
        .expect("event_shard subscription applied");
    rx_i
        .recv_timeout(Duration::from_secs(5))
        .expect("index master_clock subscription applied");
    let m0 = index.db().master_clock().realm().find(&realm).map(|c| c.tic as u16).unwrap_or(0);
    tracing::info!(master = m0, "subscriptions applied; grouping");

    // Refs we've already assigned this run — skips redundant (idempotent) reducer calls between the
    // assign and the cache reflecting ASSIGNED. Pruned each pass to what's still in the queue, so it
    // stays bounded by in-flight work; a fresh orchestrator starts empty and re-drives everything.
    let mut assigned: HashSet<u32> = HashSet::new();

    let mut ticker = tokio::time::interval(period);
    loop {
        ticker.tick().await;

        let master = index.db().master_clock().realm().find(&realm).map(|c| c.tic as u16).unwrap_or(0);
        let frozen_through = tic_add(master, 2); // T is frozen once master ≥ T-2, i.e. T ≤ master+2

        // Gather this pass's work: frozen, still QUEUED, not already assigned. Bucket per tic — only
        // same-tic events compose together.
        let mut in_queue: HashSet<u32> = HashSet::new();
        let mut per_tic: HashMap<u16, Vec<(u32, Vec<u32>)>> = HashMap::new();
        // Each write target's route (F11) — how to claim its slot: hot (`data_shard`) vs a cold shard's
        // baseline/overlay tier. Built alongside `write_targets`; a target's route is consistent, so
        // first-seen wins.
        let mut route_of: HashMap<u32, Route> = HashMap::new();
        for row in event.db().event_log().iter() {
            in_queue.insert(row.event_reference);
            if status_phase(row.status) != EVENT_QUEUED {
                continue;
            }
            if tic_after(row.event_tic, frozen_through) {
                continue; // tic still open — never group before it's frozen
            }
            if assigned.contains(&row.event_reference) {
                continue;
            }
            let targets = match action::write_targets(&row.actions) {
                Ok(t) => t,
                Err(e) => {
                    // Validated at `queue`, so this shouldn't happen — but never assign a program we
                    // can't frame.
                    tracing::warn!(event = format!("{:#010x}", row.event_reference), ?e, "unframable program — skipping");
                    continue;
                }
            };
            if let Ok(rs) = action::target_routes(&row.actions) {
                for (tgt, r) in rs {
                    route_of.entry(tgt).or_insert(r);
                }
            }
            per_tic.entry(row.event_tic).or_default().push((row.event_reference, targets));
        }
        assigned.retain(|r| in_queue.contains(r)); // prune settled/failed refs

        if per_tic.is_empty() {
            continue;
        }

        // Least-loaded worker selection, balanced across the whole pass (reset each pass).
        let mut load: HashMap<u8, usize> = workers.iter().map(|w| (*w, 0)).collect();

        // Deterministic tic order (serial) for stable logs; partition itself is order-independent.
        let mut tics: Vec<u16> = per_tic.keys().copied().collect();
        tics.sort_by(|a, b| if tic_after(*a, *b) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less });

        for t in tics {
            let events = &per_tic[&t];
            for wg in group(events) {
                let worker = *load
                    .iter()
                    .min_by_key(|(_, n)| **n)
                    .map(|(w, _)| w)
                    .expect("at least one worker configured");
                *load.get_mut(&worker).unwrap() += wg.entities.len().max(1);

                if let Err(err) = event.reducers().assign(wg.events.clone(), worker) {
                    tracing::warn!(%err, tic = t, worker, "assign failed");
                    continue;
                }
                // Claim the component's slots, each on the shard **and tier** its action routes to
                // (F11) — a component may span shards + tiers (a cold overlay cell + a pawn), so bucket
                // by `(shard_key, overlay?)` and claim per bucket: hot → `data_shard` baseline; cold →
                // its shard's `claim` (baseline) or `claim_overlay` (overlay).
                if !wg.entities.is_empty() {
                    let mut by_route: HashMap<(u8, bool), Vec<u32>> = HashMap::new();
                    for &e in &wg.entities {
                        let key = match route_of.get(&e) {
                            Some(Route::ColdOverlay { type_id }) => (shard_key_of_type(*type_id), true),
                            Some(Route::ColdBaseline { type_id }) => (shard_key_of_type(*type_id), false),
                            _ => (0, false), // hot → data_shard baseline
                        };
                        by_route.entry(key).or_default().push(e);
                    }
                    let mut claim_ok = true;
                    for ((shard, overlay), ents) in by_route {
                        let res = match (shard, overlay) {
                            (1, false) => tile.reducers().claim(ents, t, worker),
                            (1, true) => tile.reducers().claim_overlay(ents, t, worker),
                            (2, false) => thing.reducers().claim(ents, t, worker),
                            (2, true) => thing.reducers().claim_overlay(ents, t, worker),
                            _ => data.reducers().claim(ents, t, worker),
                        };
                        if let Err(err) = res {
                            tracing::warn!(%err, tic = t, worker, shard, overlay, "claim failed");
                            claim_ok = false;
                            break;
                        }
                    }
                    if !claim_ok {
                        continue;
                    }
                }
                for r in &wg.events {
                    assigned.insert(*r);
                }
                tracing::info!(
                    tic = t,
                    worker = format!("{worker:#04x}"),
                    events = wg.events.len(),
                    entities = wg.entities.len(),
                    "assigned work-group"
                );
            }
        }
    }
}
