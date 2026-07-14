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
    abort as _, claim as _, mint_cold as _, ready as _, resolve as _, stand_up as _,
    ColdTableAccess, DbConnection, EventLogTableAccess, StateLog, StateLogTableAccess,
    StateTableAccess, TicMetaTableAccess, TargetState,
};

// event_log.status (mirror of resonantdust_pipeline::STATUS_*; kept local to avoid pulling the
// module crate + its `spacetimedb` dep into this SDK-client binary).
const STATUS_ENQUEUE: u8 = 0;
const STATUS_QUEUEING: u8 = 1;
const STATUS_IN_QUEUE: u8 = 2;
const STATUS_RUNNING: u8 = 3;
const STATUS_COMPLETE: u8 = 4;
const STATUS_QUEUE_FAILED: u8 = 5;

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
                "SELECT * FROM state",
                "SELECT * FROM cold",
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
                    find_or_mint(conn, target); // cold target ⇒ promote hot (issue 005)
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
            STATUS_RUNNING => {
                // Owned by another worker — try to take over (claim no-ops unless the lease
                // expired, i.e. that worker died mid-execute). Recovery.
                let _ = conn.reducers().claim(worker_id, ev.event_reference);
            }
            _ => {}
        }
    }
}

/// The current master tic (max over `tic_meta`; 0 before the first bump).
fn master_tic(conn: &DbConnection) -> u32 {
    conn.db().tic_meta().iter().map(|m| m.master_tic).max().unwrap_or(0)
}

/// Is an event's `event_reference` `complete`?
fn event_complete(conn: &DbConnection, event_reference: u64) -> bool {
    conn.db()
        .event_log()
        .iter()
        .find(|e| e.event_reference == event_reference)
        .map(|e| e.status == STATUS_COMPLETE)
        .unwrap_or(false)
}

/// Is an event ready to contribute to a tic's composition — no `await` gate, or its await is
/// complete? (A gated event whose await isn't satisfied must not fold in yet.)
fn applicable(conn: &DbConnection, ev: &bindings::shard::EventLog) -> bool {
    match vm::await_gate(&ev.actions) {
        Some((_, await_ref, _)) => event_complete(conn, await_ref),
        None => true,
    }
}

/// Run the row and commit it atomically. Two design mechanisms live here:
/// - **tic-gating**: resolve only a *sealed* tic (`master ≥ event_tic`) — by then no new event
///   for that tic can arrive (they'd target `master+3`), so the composition is complete.
/// - **deterministic composition**: a target's value at the tic is `base@(tic−1)` folded over
///   **all** applicable events targeting it, in `event_reference` order — never arrival order —
///   so any row triggering the resolve computes the same result (idempotent, order-free).
///
/// The row's own `await` gate defers/aborts it before it contributes.
fn execute(conn: &DbConnection, worker_id: u16, ev: &bindings::shard::EventLog) {
    // tic-gate: don't resolve until the tic is sealed (all its events are present).
    if master_tic(conn) < ev.event_tic {
        return; // defer — revisit once the master reaches this tic
    }
    // This row's await gate: defer until the aliased row completes, or timeout → abort.
    if let Some((timeout, await_ref, _)) = vm::await_gate(&ev.actions) {
        if !event_complete(conn, await_ref) {
            if master_tic(conn).saturating_sub(ev.tic_state_change) > timeout {
                if let Err(err) = conn.reducers().abort(worker_id, ev.event_reference) {
                    tracing::warn!(%err, ev = ev.event_reference, "abort failed");
                } else {
                    tracing::info!(ev = ev.event_reference, await_ref, "await timed out → abort");
                }
            }
            return; // defer or aborted
        }
    }
    let below = ev.event_tic.saturating_sub(1);
    let results: Vec<TargetState> = ev
        .targets
        .iter()
        .map(|&target| {
            let base = resolved_at(conn, target, below).unwrap_or_default();
            // Fold every applicable event targeting (target, this tic), by event_reference.
            let mut evs: Vec<_> = conn
                .db()
                .event_log()
                .iter()
                .filter(|e| {
                    e.event_tic == ev.event_tic
                        && e.targets.contains(&target)
                        && e.status != STATUS_QUEUE_FAILED
                        && applicable(conn, e)
                })
                .collect();
            evs.sort_by_key(|e| e.event_reference);
            let mut state = base;
            for e in &evs {
                let b = vm::await_gate(&e.actions).map(|(_, _, body)| body).unwrap_or(&e.actions);
                state = vm::run(b, state);
            }
            TargetState {
                entity_key: target,
                kind: state.kind,
                zone_id: state.zone_id,
                location: state.location,
                rotation: state.rotation,
                offset: state.offset,
                data_0: state.data[0],
                data_1: state.data[1],
            }
        })
        .collect();
    match conn.reducers().resolve(worker_id, ev.event_reference, results) {
        Ok(()) => tracing::info!(ev = ev.event_reference, targets = ev.targets.len(), "resolved"),
        Err(err) => tracing::warn!(%err, ev = ev.event_reference, "resolve failed"),
    }
}

/// Cold target ⇒ promote it hot (find-or-mint). A **positional** `entity_reference` names a
/// cold object by location; if no hot entity exists there yet, decode the cold object's kind and
/// call `mint_cold` to seed it (keyed by the positional ref) + tombstone the cold slot. The
/// worker does the decode because the module is payload-generic (docs/issues/005).
fn find_or_mint(conn: &DbConnection, target: u64) {
    use resonantdust_codec::object::{kind_ref_kind_id, kind_ref_x, kind_ref_y, type_ref_type_id};
    use resonantdust_codec::refs::{entity_ref_is_positional, entity_ref_location, entity_ref_zone_id};

    if !entity_ref_is_positional(target) {
        return; // a normal hot target — nothing to promote
    }
    if conn.db().state().iter().any(|s| s.entity_key == target) {
        return; // already hot at this location — idempotent
    }
    let zone = entity_ref_zone_id(target);
    let loc = entity_ref_location(target);
    let (x, y) = (loc & 0x0F, loc >> 4);
    // Find the cold object sitting at this tile and read its kind + type.
    let found = conn.db().cold().iter().filter(|c| c.zone_id == zone).find_map(|c| {
        c.kinds
            .iter()
            .find(|&&k| kind_ref_x(k) == x && kind_ref_y(k) == y)
            .map(|&k| (kind_ref_kind_id(k), type_ref_type_id(c.type_reference)))
    });
    let Some((kind_id, type_id)) = found else {
        return; // nothing cold here
    };
    // tombstone: x:4 | y:4 | layer:4(=0) | type_id:4
    let tombstone = ((x as u16) << 12) | ((y as u16) << 8) | (type_id as u16 & 0x0F);
    if let Err(err) = conn.reducers().mint_cold(target, tombstone, kind_id, zone, loc, 0, 0, 0, 0) {
        tracing::warn!(%err, target, "mint_cold failed");
    } else {
        tracing::info!(target, zone, loc, kind = kind_id, "cold → hot (find-or-mint)");
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
