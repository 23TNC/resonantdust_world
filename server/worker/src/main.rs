//! server_worker — a worker in the resolution pool, driving the two-phase recoverable
//! lifecycle (`docs/spacetime-tables/lifecycle.md`, `docs/spacetime-implementation/s3-worker.md`).
//!
//! Each pass over a shard's `event_log`:
//!   - **enqueue**: `ENQUEUE` rows → `claim` (→ `queueing`); a `QUEUEING` row we own →
//!     `stand_up` each target, then `ready` (→ `in_queue`).
//!   - **execute**: `IN_QUEUE` rows → `claim` (→ `running`); a `RUNNING` row we own → run the
//!     DSL interpreter ([`resonantdust_tick::vm`]) over its `actions` for each target,
//!     producing a `TargetState`, then `resolve` (atomic commit of the whole row).
//!
//! Hot path (spawn/move): no operand reads, no cross-shard reads. Cold `find-or-mint`, control
//! flow (await/skip/fail), and cross-shard convergence land in later phases.

use std::sync::Arc;
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_tick::{domain::EntityState, vm};

mod bindings;
use bindings::shard::{
    abort as _, claim as _, ready as _, resolve as _, stand_up as _, DbConnection,
    EventLogTableAccess, StateLog, StateLogTableAccess, TicMetaTableAccess, TargetState,
};

// event_log.status (mirror of resonantdust_pipeline::STATUS_*; kept local to avoid pulling the
// module crate + its `spacetimedb` dep into this SDK-client binary).
const STATUS_ENQUEUE: u8 = 0;
const STATUS_QUEUEING: u8 = 1;
const STATUS_IN_QUEUE: u8 = 2;
const STATUS_RUNNING: u8 = 3;
const STATUS_COMPLETE: u8 = 4;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "worker=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let worker_id: u16 = env_or("WORKER_ID", "1").parse().unwrap_or(1);
    let poll = Duration::from_millis(env_or("POLL_MS", "50").parse().unwrap_or(50));
    assert!(worker_id != 0, "WORKER_ID must be non-zero (0 = unclaimed sentinel)");

    let shard_cfg = parse_shards();
    tracing::info!(%uri, worker_id, shards = shard_cfg.len(), "worker starting");

    let mut shards: Vec<Arc<DbConnection>> = Vec::new();
    for (id, db) in &shard_cfg {
        let conn = DbConnection::builder()
            .with_uri(&uri)
            .with_database_name(db)
            .on_connect({
                let db = db.clone();
                let sid = *id;
                move |_ctx, identity, _token| tracing::info!(%identity, %db, shard = sid, "connected")
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
            .on_applied({
                let db = db.clone();
                move |_ctx| tracing::info!(%db, "subscription applied")
            })
            .subscribe([
                "SELECT * FROM event_log",
                "SELECT * FROM state_log",
                "SELECT * FROM tic_meta",
            ]);
        shards.push(conn);
    }

    let mut ticker = tokio::time::interval(poll);
    loop {
        ticker.tick().await;
        for conn in &shards {
            work_pass(conn, worker_id);
        }
    }
}

/// Parse the shard set from `SHARDS="1=db_a,2=db_b"`. Falls back to a single shard.
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

/// One pass over one shard's `event_log`, advancing every row it can through the lifecycle.
fn work_pass(conn: &DbConnection, worker_id: u16) {
    let rows: Vec<_> = conn.db().event_log().iter().collect();
    for ev in rows {
        match ev.status {
            STATUS_ENQUEUE => {
                if let Err(err) = conn.reducers().claim(worker_id, ev.event_reference) {
                    tracing::warn!(%err, ev = ev.event_reference, "claim(enqueue) failed");
                }
            }
            STATUS_QUEUEING if ev.worker_reference == worker_id => {
                for &target in &ev.targets {
                    if let Err(err) = conn.reducers().stand_up(worker_id, ev.event_reference, target) {
                        tracing::warn!(%err, ev = ev.event_reference, target, "stand_up failed");
                    }
                }
                if let Err(err) = conn.reducers().ready(worker_id, ev.event_reference) {
                    tracing::warn!(%err, ev = ev.event_reference, "ready failed");
                }
            }
            STATUS_IN_QUEUE => {
                if let Err(err) = conn.reducers().claim(worker_id, ev.event_reference) {
                    tracing::warn!(%err, ev = ev.event_reference, "claim(in_queue) failed");
                }
            }
            STATUS_RUNNING if ev.worker_reference == worker_id => {
                execute(conn, worker_id, &ev);
            }
            _ => {}
        }
    }
}

/// The current master tic (max over `tic_meta`; 0 before the first bump).
fn master_tic(conn: &DbConnection) -> u32 {
    conn.db().tic_meta().iter().map(|m| m.master_tic).max().unwrap_or(0)
}

/// Run the row's program over each target and commit the whole row atomically. If the row opens
/// with an `await` gate, resolve/defer/abort on the awaited row first.
fn execute(conn: &DbConnection, worker_id: u16, ev: &bindings::shard::EventLog) {
    // Await gate: block until the aliased row completes, or the timeout (in tics) fires → abort.
    let body: &[u64] = match vm::await_gate(&ev.actions) {
        Some((timeout, await_ref, gate_body)) => {
            let complete = conn
                .db()
                .event_log()
                .iter()
                .find(|e| e.event_reference == await_ref)
                .map(|e| e.status == STATUS_COMPLETE)
                .unwrap_or(false);
            if complete {
                gate_body
            } else {
                if master_tic(conn).saturating_sub(ev.tic_state_change) > timeout {
                    if let Err(err) = conn.reducers().abort(worker_id, ev.event_reference) {
                        tracing::warn!(%err, ev = ev.event_reference, "abort failed");
                    } else {
                        tracing::info!(ev = ev.event_reference, await_ref, "await timed out → abort");
                    }
                }
                return; // defer (leave RUNNING, revisit) — or aborted above
            }
        }
        None => &ev.actions,
    };
    let below = ev.event_tic.saturating_sub(1);
    let results: Vec<TargetState> = ev
        .targets
        .iter()
        .map(|&target| {
            let base = resolved_at(conn, target, below).unwrap_or_default();
            let out = vm::run(body, base);
            TargetState {
                entity_key: target,
                kind: out.kind,
                zone_id: out.zone_id,
                location: out.location,
                rotation: out.rotation,
                offset: out.offset,
                data_0: out.data[0],
                data_1: out.data[1],
            }
        })
        .collect();
    match conn.reducers().resolve(worker_id, ev.event_reference, results) {
        Ok(()) => tracing::info!(ev = ev.event_reference, targets = ev.targets.len(), "resolved"),
        Err(err) => tracing::warn!(%err, ev = ev.event_reference, "resolve failed"),
    }
}

/// Project a `state_log` row's game fields into an [`EntityState`].
fn entity_state(r: &StateLog) -> EntityState {
    EntityState {
        kind: r.kind,
        zone_id: r.zone_id,
        location: r.location,
        rotation: r.rotation,
        offset: r.offset,
        data: [r.data_0, r.data_1],
    }
}

/// The entity's most recent resolved (`dirty==0`) state at or below `tic`.
fn resolved_at(conn: &DbConnection, entity: u64, tic: u32) -> Option<EntityState> {
    conn.db()
        .state_log()
        .iter()
        .filter(|r| r.entity_key == entity && r.dirty == 0 && r.tic <= tic)
        .max_by_key(|r| r.tic)
        .map(|r| entity_state(&r))
}
