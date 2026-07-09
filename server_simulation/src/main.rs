//! server_simulation — a worker in the resolution pool.
//!
//! Subscribes to a shard's `state_log`/`event_log`, and each pass:
//!   1. finds the **lowest pending tic per entity** (in-order resolution),
//!   2. `claim`s it (fenced; a claim it already holds is a no-op),
//!   3. once it owns the claim, composes the tic's events (ordered by
//!      `event_reference`) onto the entity's most-recent resolved state *below* that
//!      tic, and calls `resolve` with the result.
//!
//! Phase A2 is position-only (`move`) with same-shard reads. The read-rule /
//! priority-DAG (actor reads) and actor-consuming actions (`damage`) land in A3; the
//! claim→resolve here already goes through the fence and the subscription, so the
//! blocking-read machinery slots in without restructuring. Poll-driven for now;
//! reactive wake + eviction are A4.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_tick::{resolve_events, EntityState, Event};

mod bindings;
use bindings::object_shard::{
    claim as _, resolve as _, DbConnection, EventLogTableAccess, StateLog, StateLogTableAccess,
};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server_simulation=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let db = env_or("OBJECT_DB", "resonantdust-dev-object-0");
    let worker_id: u16 = env_or("WORKER_ID", "1").parse().unwrap_or(1);
    let poll = Duration::from_millis(env_or("POLL_MS", "50").parse().unwrap_or(50));
    assert!(worker_id != 0, "WORKER_ID must be non-zero (0 = unclaimed sentinel)");
    tracing::info!(%uri, %db, worker_id, "server_simulation starting");

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

    conn.subscription_builder()
        .on_applied(|_ctx| tracing::info!("subscription applied"))
        .subscribe(["SELECT * FROM state_log", "SELECT * FROM event_log"]);

    let mut ticker = tokio::time::interval(poll);
    loop {
        ticker.tick().await;
        work_pass(&conn, worker_id);
    }
}

/// One resolution pass over the current subscription snapshot.
fn work_pass(conn: &DbConnection, worker_id: u16) {
    // Lowest pending (dirty>0) tic per entity — resolve an entity's tics in order.
    let mut lowest: HashMap<u64, StateLog> = HashMap::new();
    for r in conn.db().state_log().iter() {
        if r.dirty == 0 {
            continue;
        }
        match lowest.get(&r.entity_key) {
            Some(cur) if cur.tic <= r.tic => {}
            _ => {
                lowest.insert(r.entity_key, r);
            }
        }
    }

    for (entity, row) in lowest {
        if row.server_id == 0 {
            // Unclaimed → claim it; we resolve on a later pass once we own it.
            if let Err(err) = conn.reducers().claim(worker_id, entity, row.tic) {
                tracing::warn!(%err, entity, tic = row.tic, "claim failed");
            }
        } else if row.server_id == worker_id {
            resolve_one(conn, worker_id, entity, &row);
        }
        // else: owned by another live worker — skip (A4 handles eviction).
    }
}

/// Compose `(entity, tic)`'s events onto the entity's prior resolved state and commit.
fn resolve_one(conn: &DbConnection, worker_id: u16, entity: u64, row: &StateLog) {
    // Base = the entity's most recent resolved state strictly below this tic (its state
    // at tic-1, idle-carried). Computed authoritatively here — the pending row's carried
    // fields can predate a just-resolved earlier tic.
    let base = conn
        .db()
        .state_log()
        .iter()
        .filter(|r| r.entity_key == entity && r.dirty == 0 && r.tic < row.tic)
        .max_by_key(|r| r.tic)
        .map(|r| EntityState {
            kind: r.kind,
            zone_id: r.zone_id,
            location: r.location,
            rotation: r.rotation,
            offset: r.offset,
            data: [r.data_0, r.data_1],
        })
        .unwrap_or_default();

    // The tic's active events targeting this entity, ordered by event_reference.
    let mut evs: Vec<_> = conn
        .db()
        .event_log()
        .iter()
        .filter(|e| e.target_key == entity && e.event_tic == row.tic && e.status == 0)
        .collect();
    evs.sort_by_key(|e| e.event_reference);
    let events: Vec<Event> = evs
        .iter()
        .map(|e| Event {
            action: e.action,
            actor_key: e.actor_key,
            data: [e.data_0, e.data_1],
        })
        .collect();

    let out = resolve_events(base, &events);
    match conn.reducers().resolve(
        worker_id,
        entity,
        row.tic,
        out.kind,
        out.zone_id,
        out.location,
        out.rotation,
        out.offset,
        out.data[0],
        out.data[1],
    ) {
        Ok(()) => tracing::info!(entity, tic = row.tic, loc = out.location, "resolved"),
        Err(err) => tracing::warn!(%err, entity, tic = row.tic, "resolve failed"),
    }
}
