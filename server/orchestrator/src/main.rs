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

use resonantdust_codec::action;
use resonantdust_codec::object::{TYPE_BIOME_THING, TYPE_BIOME_TILE};
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::status::{status_phase, EVENT_QUEUED};
use resonantdust_codec::tic::{tic_add, tic_after};
use resonantdust_st_bindings::{data_shard, event_shard, index, thing, tile};
use data_shard::claim as _;
use event_shard::{assign as _, EventLogTableAccess as _};
use index::MasterClockTableAccess as _;
// Cold shards ride the same overlay, so a cold-cell mutation's slot is `claim`ed on its shard.
use thing::claim as _;
use tile::claim as _;

mod grouping;
use grouping::group;

/// Which composing shard an entity's slot lives on — routed by its `server_reference` `type_id` (top
/// nibble). A cold-cell `SET` target is `TYPE_BIOME_TILE`/`TYPE_BIOME_THING`; anything else (a pawn,
/// …) is the hot `data_shard`, unchanged.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shard {
    Data,
    Tile,
    Thing,
}

fn shard_of(entity: u32) -> Shard {
    match entity_ref_type_id(entity) {
        TYPE_BIOME_TILE => Shard::Tile,
        TYPE_BIOME_THING => Shard::Thing,
        _ => Shard::Data,
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
    let tic_hz: f64 = env_or("TIC_HZ", "2").parse().unwrap_or(2.0);
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
                // Claim the component's slots, each on the shard that owns the entity (routed by its
                // `server_reference` top byte) — a component may span shards (a cold cell + a pawn),
                // so partition and claim per shard. A pawn's `0x30` → `data_shard`, unchanged.
                if !wg.entities.is_empty() {
                    let mut by_shard: HashMap<u8, Vec<u32>> = HashMap::new();
                    for &e in &wg.entities {
                        let key = match shard_of(e) {
                            Shard::Data => 0,
                            Shard::Tile => 1,
                            Shard::Thing => 2,
                        };
                        by_shard.entry(key).or_default().push(e);
                    }
                    let mut claim_ok = true;
                    for (key, ents) in by_shard {
                        let res = match key {
                            1 => tile.reducers().claim(ents, t, worker),
                            2 => thing.reducers().claim(ents, t, worker),
                            _ => data.reducers().claim(ents, t, worker),
                        };
                        if let Err(err) = res {
                            tracing::warn!(%err, tic = t, worker, shard = key, "claim failed");
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
