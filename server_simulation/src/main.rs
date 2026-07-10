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

use resonantdust_tick::{
    action_reads_actor, actor_read_tic, resolve_events, resolved_through, EntityState, Event,
    Spatial,
};

mod bindings;
use bindings::shard::{
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
    let worker_id: u16 = env_or("WORKER_ID", "1").parse().unwrap_or(1);
    let poll = Duration::from_millis(env_or("POLL_MS", "50").parse().unwrap_or(50));
    assert!(worker_id != 0, "WORKER_ID must be non-zero (0 = unclaimed sentinel)");

    // The shard set this worker connects to, `shard_id → db_name`. It resolves work on
    // every shard here, and routes an actor read to the actor's shard by the event's
    // `actor_shard_id` (falling back to the target's own shard). One shard = the A2
    // same-shard case; two or more = the C2 cross-shard case.
    let shard_cfg = parse_shards();
    tracing::info!(%uri, worker_id, shards = shard_cfg.len(), "server_simulation starting");

    let mut shards: HashMap<u16, Arc<DbConnection>> = HashMap::new();
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
            .subscribe(["SELECT * FROM state_log", "SELECT * FROM event_log"]);
        shards.insert(*id, conn);
    }

    let mut ticker = tokio::time::interval(poll);
    loop {
        ticker.tick().await;
        for conn in shards.values() {
            work_pass(conn, &shards, worker_id);
        }
    }
}

/// Parse the shard set from `SHARDS="1=db_a,2=db_b"` (`shard_id=db_name`). Falls back to
/// a single shard `1=<SHARD_DB>` — the same-shard configuration.
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

/// One resolution pass over one home shard's snapshot. `shards` is the full set, used
/// to route cross-shard actor reads.
fn work_pass(conn: &DbConnection, shards: &HashMap<u16, Arc<DbConnection>>, worker_id: u16) {
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
            resolve_one(conn, shards, worker_id, entity, &row);
        }
        // else: owned by another live worker — skip (A4 handles eviction).
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

/// The entity's most recent resolved (`dirty==0`) state at or below `tic` — its
/// idle-carried value at that tic. `None` if it has never resolved that far down
/// (an absent actor, which reads as dead).
fn resolved_at(conn: &DbConnection, entity: u64, tic: u32) -> Option<EntityState> {
    conn.db()
        .state_log()
        .iter()
        .filter(|r| r.entity_key == entity && r.dirty == 0 && r.tic <= tic)
        .max_by_key(|r| r.tic)
        .map(|r| entity_state(&r))
}

/// The entity's lowest pending (`dirty>0`) tic, or `None` if it has none. The read
/// rule's input: an entity is resolved through `T` iff it has no pending row `≤ T`.
fn min_pending_tic(conn: &DbConnection, entity: u64) -> Option<u32> {
    conn.db()
        .state_log()
        .iter()
        .filter(|r| r.entity_key == entity && r.dirty > 0)
        .map(|r| r.tic)
        .min()
}

/// Try to resolve `(entity, tic)`. Gathers each actor-reading event's actor state via
/// the **read-rule + priority-DAG**; if any required actor isn't resolved through its
/// read tic yet, returns without committing (the claim is held; a later pass retries
/// once the actor's row flips to `dirty==0`). Returns `true` if it resolved.
fn resolve_one(
    conn: &DbConnection,
    shards: &HashMap<u16, Arc<DbConnection>>,
    worker_id: u16,
    entity: u64,
    row: &StateLog,
) -> bool {
    // Base = the entity's most recent resolved state strictly below this tic.
    let base = resolved_at(conn, entity, row.tic.saturating_sub(1)).unwrap_or_default();

    // The tic's active events targeting this entity, ordered by event_reference.
    let mut evs: Vec<_> = conn
        .db()
        .event_log()
        .iter()
        .filter(|e| e.target_key == entity && e.event_tic == row.tic && e.status == 0)
        .collect();
    evs.sort_by_key(|e| e.event_reference);

    // Pair each event with its actor's resolved state, applying the read-rule.
    let mut paired: Vec<(Event, Option<EntityState>)> = Vec::with_capacity(evs.len());
    for e in &evs {
        let ev = Event { action: e.action, actor_key: e.actor_key, data: [e.data_0, e.data_1] };
        let actor = if action_reads_actor(ev.action) {
            // Route the actor read to its shard by `actor_shard_id` (fall back to this
            // home shard for a same-shard/unset actor). THIS is the cross-shard step: a
            // different shard's connection, read + blocked-on exactly like the local one.
            let actor_conn: &DbConnection =
                shards.get(&e.actor_shard_id).map_or(conn, |c| c.as_ref());
            // priority-DAG: read the actor at `tic` if it ranks below the target, else
            // at `tic-1` (the back-edge that keeps cycles deadlock-free).
            let read_tic = actor_read_tic(ev.actor_key, entity, row.tic);
            // read-rule: block unless the actor is resolved through `read_tic`.
            if !resolved_through(min_pending_tic(actor_conn, ev.actor_key), read_tic) {
                return false; // an actor isn't ready — leave the whole resolution parked
            }
            resolved_at(actor_conn, ev.actor_key, read_tic)
        } else {
            None
        };
        paired.push((ev, actor));
    }

    let out = resolve_events::<Spatial>(base, &paired);
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
        Ok(()) => {
            tracing::info!(entity, tic = row.tic, loc = out.location, hp = out.data[0], "resolved");
            true
        }
        Err(err) => {
            tracing::warn!(%err, entity, tic = row.tic, "resolve failed");
            false
        }
    }
}
