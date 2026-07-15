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
//! Hot path (spawn/move): no operand reads. A row's targets may span shards: the worker computes
//! every target's state, then **converges** them — targets on this shard commit via `resolve`
//! (which also completes the row), targets on another shard via that shard's idempotent
//! `resolve_foreign`; the row completes only once every target shard has applied
//! (`docs/spacetime-tables/lifecycle.md` §convergent write).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_tick::{domain::EntityState, vm};

mod bindings;
use bindings::shard::{
    abort as _, append as _, claim as _, mint_cold as _, pack_settle as _, ready as _,
    resolve as _, resolve_foreign as _, stand_up as _, stand_up_foreign as _, ColdTableAccess,
    DbConnection, EventLogTableAccess, StateLog, StateLogTableAccess, StateTableAccess,
    TargetState, TicMetaTableAccess,
};

/// How many tics a minted-cold (find-or-mint) hot object may sit idle before a PACK settles it
/// back to cold. Long enough that an interacted object stays live to act on, short enough that
/// the cold store re-compacts promptly once it's left alone. (`docs/…/hot-cold.md` §PACK.)
const PACK_IDLE_TICS: u32 = 20;

/// The connected shard set, `(server_reference, connection)`. A target's home shard is its
/// `entity_key`'s `mint_server`; positional (cold) targets are always local (minted here).
type Shards = [(u16, Arc<DbConnection>)];

/// The connection for shard `id`, if the worker holds one.
fn shard_conn(shards: &Shards, id: u16) -> Option<&Arc<DbConnection>> {
    shards.iter().find(|(sid, _)| *sid == id).map(|(_, c)| c)
}

/// Which shard a target's resolved state lives on. Two cases resolve **locally** (`my_shard`):
/// a **positional** (cold) target is minted on the shard that owns its zone; and an **unminted**
/// target (`mint_server == SERVER_REF_NONE`, i.e. 0 — a fresh spawn not yet bound to a server)
/// is owned by whichever shard processes its event. Only a target explicitly minted by a
/// *different* server is foreign.
fn home_shard(target: u64, my_shard: u16) -> u16 {
    use resonantdust_codec::refs::{entity_ref_is_positional, entity_ref_server_reference, SERVER_REF_NONE};
    if entity_ref_is_positional(target) {
        return my_shard;
    }
    match entity_ref_server_reference(target) {
        SERVER_REF_NONE => my_shard, // unminted ⇒ local (0 is "no server", never a real shard)
        m => m,
    }
}

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

    let mut shards: Vec<(u16, Arc<DbConnection>)> = Vec::new();
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
                "SELECT * FROM applied_foreign",
            ]);
        shards.push((*id, conn));
    }

    let mut ticker = tokio::time::interval(poll);
    loop {
        ticker.tick().await;
        for (id, _) in &shards {
            work_pass(&shards, *id, worker_id);
            if let Some(conn) = shard_conn(&shards, *id) {
                enqueue_pack_sweep(conn); // the PACK trigger — enqueues; execute does the settling
            }
        }
    }
}

/// Is a hot `state` row a **packable, at-rest** object? — a find-or-mint object (`REF_COLD`
/// entity_key) carrying cold provenance, idle for longer than [`PACK_IDLE_TICS`]. Pawns are
/// `REF_HOT` and never match, so they never pack: the criterion `docs/…/hot-cold.md` wants (pawns
/// always hot; biome objects compact to cold). The refcount gate ("no holders") lives in the
/// `pack_settle` reducer, where it's atomic with the write.
fn packable(s: &bindings::shard::State, now: u32) -> bool {
    use resonantdust_codec::refs::{entity_ref_reference_id, REF_COLD};
    entity_ref_reference_id(s.entity_key) == REF_COLD
        && (s.data_0 >> 32) != 0 // has cold provenance (stashed at mint)
        && now.saturating_sub(s.tic) > PACK_IDLE_TICS
}

/// The **PACK trigger** (`hot-cold.md` names a periodic sweep as one of its candidate owners):
/// find zones holding at-rest packable objects and **enqueue a `PACK` event** against each — the
/// design's *execute op*, so the settle rides the fenced, recoverable tick loop rather than a
/// side-door reducer call. Skips a zone that already has a PACK in flight so passes don't pile up.
fn enqueue_pack_sweep(conn: &DbConnection) {
    use resonantdust_codec::object::pack_cold_reference;
    use resonantdust_codec::packed::{zone_realm, zone_region, zone_zone};
    use resonantdust_codec::refs::{
        entity_ref_zone_id, pack_cold_entity, pack_server_reference,
    };
    let now = master_tic(conn);
    let zones: std::collections::BTreeSet<u32> = conn
        .db()
        .state()
        .iter()
        .filter(|s| packable(s, now))
        .map(|s| entity_ref_zone_id(s.entity_key))
        .collect();
    for zone_id in zones {
        // One PACK per zone in flight: a queued/running PACK for this zone means the settle is
        // already scheduled — enqueueing another would just churn the log.
        let in_flight = conn.db().event_log().iter().any(|e| {
            e.status != STATUS_COMPLETE
                && e.status != STATUS_QUEUE_FAILED
                && vm::pack_target(&e.actions).is_some_and(|(sr, zr)| {
                    entity_ref_zone_id(pack_cold_entity(sr, zr)) == zone_id
                })
        });
        if in_flight {
            continue;
        }
        // Name the zone the way a cold object names its place: server_reference (realm+shard) +
        // a cold_reference with tile/layer zeroed.
        let server_reference = pack_server_reference(zone_realm(zone_id), 0);
        let zone_reference = pack_cold_reference(zone_region(zone_id), zone_zone(zone_id), 0, 0);
        match conn.reducers().append(vm::encode_pack(server_reference, zone_reference), Vec::new()) {
            Ok(()) => tracing::info!(zone = zone_id, "enqueued PACK"),
            Err(err) => tracing::warn!(%err, zone = zone_id, "append(PACK) failed"),
        }
    }
}

/// Settle every at-rest object in `zone_id` back to cold — the body of a resolved `PACK` execute
/// op. Each object's cold provenance (`type_reference` + the original cold entry) rides its
/// `data_0` (stashed at mint — see [`find_or_mint`]), so the restore is **exact**
/// (subtype/variant/data survive the hot round-trip). `pack_settle` skips a busy object (any
/// holder) atomically with its write, so an object that caught new activity simply stays hot.
/// Returns how many settled.
fn pack_zone(conn: &DbConnection, zone_id: u32) -> usize {
    use resonantdust_codec::object::{
        cold_ref_layer_reference, cold_ref_macro_position, cold_ref_tile, layer_ref_layer_id,
        pack_cold_row_reference,
    };
    use resonantdust_codec::refs::{entity_ref_cold_reference, entity_ref_zone_id};
    let now = master_tic(conn);
    let idle: Vec<_> = conn
        .db()
        .state()
        .iter()
        .filter(|s| packable(s, now) && entity_ref_zone_id(s.entity_key) == zone_id)
        .collect();
    let mut settled = 0;
    for s in idle {
        // Provenance stashed at mint: data_0 = type_reference:32 | kind_pos_reference:32. The
        // row is that type_reference + the macro/layer off the object's own cold_reference —
        // i.e. exactly the header composite it came from.
        let type_reference = (s.data_0 >> 32) as u16;
        let kind_pos_reference = s.data_0 as u32; // the original entry, restored verbatim
        let cold_reference = entity_ref_cold_reference(s.entity_key);
        let macro_position = cold_ref_macro_position(cold_reference);
        let layer_id = layer_ref_layer_id(cold_ref_layer_reference(cold_reference));
        let cold_row_reference = pack_cold_row_reference(macro_position, type_reference, layer_id);
        let tombstone = cold_ref_tile(cold_reference); // a bare tile_reference
        match conn.reducers().pack_settle(
            s.entity_key, cold_row_reference, macro_position, type_reference, layer_id,
            kind_pos_reference, tombstone,
        ) {
            Ok(()) => {
                settled += 1;
                tracing::info!(entity = s.entity_key, zone = zone_id, layer_id, "hot → cold (PACK settle)");
            }
            Err(err) => tracing::warn!(%err, entity = s.entity_key, "pack_settle failed"),
        }
    }
    settled
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

/// One pass over shard `my_shard`'s `event_log`, advancing every row it can through the
/// lifecycle. `shards` is the full connected set so a row whose targets span shards can converge
/// its writes onto the other shards.
fn work_pass(shards: &Shards, my_shard: u16, worker_id: u16) {
    let Some(conn) = shard_conn(shards, my_shard) else { return; };
    let rows: Vec<_> = conn.db().event_log().iter().collect();
    for ev in rows {
        match ev.status {
            STATUS_ENQUEUE => {
                if let Err(err) = conn.reducers().claim(worker_id, ev.event_reference) {
                    tracing::warn!(%err, ev = ev.event_reference, "claim(enqueue) failed");
                }
            }
            STATUS_QUEUEING if ev.worker_reference == worker_id => {
                // Stand up EVERY target — local ones here (fenced `stand_up`), foreign ones on
                // their home shard via `stand_up_foreign`. The foreign Phase-1 hold is what makes
                // the in-flight window visible over there: the target shows pending work, so that
                // shard's read rule defers readers (instead of reading a soon-to-be-stale value)
                // and its GC won't reclaim the row. `resolve_foreign` releases the hold when the
                // write lands.
                for &target in &ev.targets {
                    let home = home_shard(target, my_shard);
                    if home != my_shard {
                        match shard_conn(shards, home) {
                            Some(fconn) => {
                                if let Err(err) = fconn.reducers().stand_up_foreign(
                                    my_shard,
                                    ev.event_reference,
                                    ev.event_tic,
                                    target,
                                ) {
                                    tracing::warn!(%err, ev = ev.event_reference, target, shard = home, "stand_up_foreign failed");
                                }
                            }
                            // No connection to the home shard: skip the hold. The convergent write
                            // still can't complete without it (execute defers), so this only costs
                            // the in-flight visibility, never correctness of the write itself.
                            None => tracing::warn!(ev = ev.event_reference, target, shard = home, "no connection — skipping foreign Phase-1 hold"),
                        }
                        continue;
                    }
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
                execute(shards, my_shard, worker_id, &ev);
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

/// The interpreter's operand reader: maps an `OBJECT` operand `(mint_server, entity_id)` to the
/// actor's hp (`data_0`) **as resolved at `≤ read_tic`** (the `≤ T−1` read rule). `0` if the
/// actor is absent, dead, or not yet settled — all read as "no live actor".
struct WorkerReads<'a> {
    conn: &'a DbConnection,
    read_tic: u32,
}
impl resonantdust_tick::vm::Reads for WorkerReads<'_> {
    fn actor_hp(&self, server_reference: u16, object_reference: u32) -> u64 {
        use resonantdust_codec::refs::{entity_ref_object_reference, entity_ref_server_reference};
        self.conn
            .db()
            .state_log()
            .iter()
            .filter(|r| {
                r.dirty == 0
                    && r.tic <= self.read_tic
                    && entity_ref_server_reference(r.entity_key) == server_reference
                    && entity_ref_object_reference(r.entity_key) == object_reference
            })
            .max_by_key(|r| r.tic)
            .map(|r| r.data_0)
            .unwrap_or(0)
    }
}

/// The `OBJECT` actor operands `(server_reference, object_reference)` an event's program reads.
fn actor_operands(actions: &[u64]) -> Vec<(u16, u32)> {
    use resonantdust_codec::event_word::{word_op_code, word_payload, word_server_reference, OP_OBJECT};
    actions
        .iter()
        .filter(|&&w| word_op_code(w) == OP_OBJECT)
        .map(|&w| (word_server_reference(w), word_payload(w)))
        .collect()
}

/// The read rule: every actor an event reads must be **settled through `read_tic`** — no pending
/// (`dirty>0`) work at or below it. Until then the read would be stale, so the reader defers.
fn actors_settled(conn: &DbConnection, actions: &[u64], read_tic: u32) -> bool {
    use resonantdust_codec::refs::{entity_ref_object_reference, entity_ref_server_reference};
    actor_operands(actions).into_iter().all(|(server, obj)| {
        let min_pending = conn
            .db()
            .state_log()
            .iter()
            .filter(|r| {
                r.dirty > 0
                    && entity_ref_server_reference(r.entity_key) == server
                    && entity_ref_object_reference(r.entity_key) == obj
            })
            .map(|r| r.tic)
            .min();
        resonantdust_tick::resolved_through(min_pending, read_tic)
    })
}

/// Is an event's `event_reference` `complete`?
fn event_complete(conn: &DbConnection, event_reference: u32) -> bool {
    conn.db()
        .event_log()
        .iter()
        .find(|e| e.event_reference == event_reference)
        .map(|e| e.status == STATUS_COMPLETE)
        .unwrap_or(false)
}

/// Is an event ready to contribute to a tic's composition — its `await` (if any) is complete
/// AND every actor it reads is settled through `≤ T−1`? A gated/reading event that isn't ready
/// must not fold in yet (it would apply prematurely / read stale).
fn applicable(conn: &DbConnection, ev: &bindings::shard::EventLog) -> bool {
    let await_ok = match vm::await_gate(&ev.actions) {
        Some((_, await_ref, _)) => event_complete(conn, await_ref),
        None => true,
    };
    await_ok && actors_settled(conn, &ev.actions, ev.event_tic.saturating_sub(1))
}

/// Run the row and commit it atomically. Two design mechanisms live here:
/// - **tic-gating**: resolve only a *sealed* tic (`master ≥ event_tic`) — by then no new event
///   for that tic can arrive (they'd target `master+3`), so the composition is complete.
/// - **deterministic composition**: a target's value at the tic is `base@(tic−1)` folded over
///   **all** applicable events targeting it, in `event_reference` order — never arrival order —
///   so any row triggering the resolve computes the same result (idempotent, order-free).
///
/// The row's own `await` gate defers/aborts it before it contributes.
fn execute(shards: &Shards, my_shard: u16, worker_id: u16, ev: &bindings::shard::EventLog) {
    let Some(conn) = shard_conn(shards, my_shard) else { return; };
    // tic-gate: don't resolve until the tic is sealed (all its events are present).
    if master_tic(conn) < ev.event_tic {
        return; // defer — revisit once the master reaches this tic
    }
    // PACK is an **execute op** (hot-cold.md; divergence #2): a zone-targeted row carrying no
    // `targets` — it holds nothing, folds nothing, and reads no actors, so it short-circuits the
    // target machinery below. Settle every at-rest object in the named zone, then complete the row
    // through the normal fenced `resolve` (empty results) so it rides the lifecycle like any other.
    if let Some((server_reference, zone_reference)) = vm::pack_target(&ev.actions) {
        use resonantdust_codec::refs::{entity_ref_zone_id, pack_cold_entity};
        let zone_id = entity_ref_zone_id(pack_cold_entity(server_reference, zone_reference));
        let packed = pack_zone(conn, zone_id);
        match conn.reducers().resolve(worker_id, ev.event_reference, Vec::new()) {
            Ok(()) => tracing::info!(ev = ev.event_reference, zone = zone_id, packed, "PACK resolved"),
            Err(err) => tracing::warn!(%err, ev = ev.event_reference, "PACK resolve failed"),
        }
        return;
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
    // Read rule: defer this row until every actor it reads is settled through ≤ T−1.
    if !actors_settled(conn, &ev.actions, below) {
        return; // an actor isn't ready — revisit once it settles
    }
    let reads = WorkerReads { conn, read_tic: below };
    let results: Vec<TargetState> = ev
        .targets
        .iter()
        .map(|&target| {
            // A target's prior state lives on its **home** shard (foreign targets read their base
            // there, not on the source shard where the event row lives).
            let base_conn = shard_conn(shards, home_shard(target, my_shard)).map_or(conn, |c| c);
            let base = resolved_at(base_conn, target, below).unwrap_or_default();
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
                state = vm::run(b, state, &reads);
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
    // Convergent write: partition the row's target states by home shard. Foreign groups apply
    // first via each shard's idempotent `resolve_foreign` (keyed by (my_shard, event_reference));
    // only once every foreign shard has acked do we `resolve` the local group — which completes
    // the row + releases holds. A crash before the local resolve leaves the row RUNNING; the
    // re-drive re-applies (foreign dedups), then completes → converges. (`lifecycle.md`.)
    let mut local: Vec<TargetState> = Vec::new();
    let mut foreign: BTreeMap<u16, Vec<TargetState>> = BTreeMap::new();
    for ts in results {
        let home = home_shard(ts.entity_key, my_shard);
        if home == my_shard {
            local.push(ts);
        } else {
            foreign.entry(home).or_default().push(ts);
        }
    }
    for (shard_id, group) in &foreign {
        let Some(fconn) = shard_conn(shards, *shard_id) else {
            tracing::warn!(ev = ev.event_reference, shard = shard_id, "no connection to target shard — deferring");
            return; // can't converge without the shard; leave RUNNING, retry next pass
        };
        if let Err(err) = fconn.reducers().resolve_foreign(my_shard, ev.event_reference, ev.event_tic, group.clone()) {
            tracing::warn!(%err, ev = ev.event_reference, shard = shard_id, "resolve_foreign failed — deferring");
            return; // foreign write didn't land; don't complete the row yet
        }
        tracing::info!(ev = ev.event_reference, shard = shard_id, n = group.len(), "converged foreign targets");
    }
    match conn.reducers().resolve(worker_id, ev.event_reference, local) {
        Ok(()) => tracing::info!(ev = ev.event_reference, targets = ev.targets.len(), foreign = foreign.len(), "resolved"),
        Err(err) => tracing::warn!(%err, ev = ev.event_reference, "resolve failed"),
    }
}

/// Cold target ⇒ promote it hot (find-or-mint). A **positional** `entity_reference` names a
/// cold object by location; if no hot entity exists there yet, decode the cold object's kind and
/// call `mint_cold` to seed it (keyed by the positional ref) + tombstone the cold slot. The
/// worker does the decode because the module is payload-generic (docs/issues/005).
fn find_or_mint(conn: &DbConnection, target: u64) {
    use resonantdust_codec::object::{
        cold_ref_layer_reference, cold_ref_macro_position, cold_ref_tile, cold_row_selects,
        kind_pos_ref_kind_id, kind_pos_ref_tile, layer_ref_layer_id, pack_cold_row_reference,
        type_ref_type_id,
    };
    use resonantdust_codec::refs::{
        entity_ref_cold_reference, entity_ref_is_positional, entity_ref_location, entity_ref_zone_id,
    };

    if !entity_ref_is_positional(target) {
        return; // a normal hot target — nothing to promote
    }
    if conn.db().state().iter().any(|s| s.entity_key == target) {
        return; // already hot at this location — idempotent
    }
    // Everything needed to *select the row* comes off the target's own `cold_reference`:
    // macro_position + (type_id, layer_id) from its layer_reference + the tile to match.
    // Only `subtype_id` isn't there — and it needn't be: the uniqueness rule is one object per
    // `(type, layer, tile)` **subtype-agnostic**, so across the zone's several subtype (biome)
    // rows at this (type, layer) exactly one holds this tile.
    let cold_reference = entity_ref_cold_reference(target);
    let macro_position = cold_ref_macro_position(cold_reference);
    let layer_id = layer_ref_layer_id(cold_ref_layer_reference(cold_reference));
    let tile = cold_ref_tile(cold_reference);

    let found = conn
        .db()
        .cold()
        .iter()
        .filter(|c| cold_row_selects(cold_reference, c.macro_position, c.type_reference, c.layer_id))
        .find_map(|c| {
            c.kinds
                .iter()
                .find(|&&k| kind_pos_ref_tile(k) == tile)
                .map(|&entry| (entry, c.type_reference, c.cold_row_reference))
        });
    let Some((entry, type_reference, cold_row_reference)) = found else {
        return; // nothing cold at this (macro, type, layer, tile)
    };
    debug_assert_eq!(
        cold_row_reference,
        pack_cold_row_reference(macro_position, type_reference, layer_id),
        "the row we picked must BE the composite of its header"
    );
    let kind_id = kind_pos_ref_kind_id(entry);
    let zone = entity_ref_zone_id(target);
    let loc = entity_ref_location(target);
    // Cold provenance stashed in `data0` = `type_reference:32 | kind_pos_reference:32`, so PACK can
    // restore the exact row + entry (subtype/variant/data survive the hot round-trip).
    let provenance = ((type_reference as u64) << 32) | (entry as u64);
    // The tombstone is a bare tile_reference — macro/type/layer are already in the row key.
    if let Err(err) = conn.reducers().mint_cold(
        target, cold_row_reference, macro_position, tile, kind_id, zone, loc, 0, 0, provenance, 0,
    ) {
        tracing::warn!(%err, target, "mint_cold failed");
    } else {
        // `type_id` off the row we PICKED (not the target's request) — the two agreeing is the
        // whole point of D-3, so log the answer, not the question.
        let type_id = type_ref_type_id(type_reference);
        tracing::info!(target, zone, loc, kind = kind_id, type_id, layer_id, "cold → hot (find-or-mint)");
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
