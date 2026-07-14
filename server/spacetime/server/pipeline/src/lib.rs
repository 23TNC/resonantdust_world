//! The shard engine — `decl_tick_pipeline!` emits the tables + reducers for the two-phase,
//! recoverable resolution lifecycle onto the invoking module. Design:
//! `docs/spacetime-tables/` (lifecycle.md is the correctness core) and the staged plan in
//! `docs/spacetime-implementation/`.
//!
//! **This is the 0.2.3 rewrite** — it replaces the old event_log/state_log/claim engine. The
//! new model:
//! - an event is a row carrying `actions: Vec<u64>` (a DSL program) + `targets` + a `status`
//!   state machine (`enqueue → queueing → in_queue → running → complete` / `queue_failed`);
//! - **enqueue** stands up a pending `state_log` row per target + registers holders;
//! - **execute** runs the program (in the worker) and commits all targets via `resolve`;
//! - GC is dumb (refcounted `holder` table); the master drops rows that miss their window;
//! - causality is strict staging (`event_tic = master + 3`), no watermark.
//!
//! The engine never runs the DSL — the worker does, then calls `resolve` with the results.

pub use resonantdust_codec::refs::pack_minted_entity;

// ── lifecycle status (event_log.status) ─────────────────────────────────────────
/// Written by the issuer; no worker has taken it to stand up yet.
pub const STATUS_ENQUEUE: u8 = 0;
/// A worker is standing up the target `state_log` rows.
pub const STATUS_QUEUEING: u8 = 1;
/// Stood up successfully; ready to execute.
pub const STATUS_IN_QUEUE: u8 = 2;
/// A worker is executing the action vector.
pub const STATUS_RUNNING: u8 = 3;
/// Done — all target writes committed.
pub const STATUS_COMPLETE: u8 = 4;
/// Missed its enqueue window (master drop); terminal, its action is dropped.
pub const STATUS_QUEUE_FAILED: u8 = 5;

// ── holder kind ─────────────────────────────────────────────────────────────────
/// A pending read hold on a `state_log` row.
pub const HOLD_READ: u8 = 0;
/// A pending write hold on a `state_log` row.
pub const HOLD_WRITE: u8 = 1;

/// `worker_reference == 0` means "unclaimed".
pub const WORKER_NONE: u16 = 0;

/// The right-shift: an appended event targets `master_tic + TIC_GAP` (enqueue at +2, execute
/// at +1, live at N — `docs/spacetime-tables/lifecycle.md`).
pub const TIC_GAP: u32 = 3;

/// How many tics a claim (fence) holds before another worker may evict — the lease.
pub const CLAIM_LEASE_TICS: u32 = 4;

/// How many tics a row may sit in `enqueue`/`queueing` before the master drops it (its
/// enqueue window). Dropping is the causality guard (no watermark).
pub const ENQUEUE_WINDOW_TICS: u32 = 3;

/// Emit the full tick pipeline into the invoking module. `payload` is the per-shape game
/// field list carried through `state_log` / `state` / `TargetState`.
#[macro_export]
macro_rules! decl_tick_pipeline {
    (
        payload: { $( $pf:ident : $pt:ty ),* $(,)? } $(,)?
    ) => {
        use ::spacetimedb::{reducer, table, ReducerContext, Table, SpacetimeType};

        // ── event side ───────────────────────────────────────────────────────────

        /// One event = one row: a DSL program (`actions`), its designated `targets`, and a
        /// lifecycle `status`. Lives on the event shard.
        #[table(accessor = event_log, public)]
        pub struct EventLog {
            #[primary_key]
            #[auto_inc]
            pub event_reference: u64,
            #[index(btree)]
            pub event_tic: u32,
            /// The DSL program — a flat postfix word stream (`docs/spacetime-tables/event-dsl.md`).
            pub actions: Vec<u64>,
            /// The entities this row writes (issuer-designated; `bump`/`stand_up` read this,
            /// never the program). Encoded as `entity_key`s.
            pub targets: Vec<u64>,
            /// The worker currently assigned this row (fence); `0` = unclaimed.
            pub worker_reference: u16,
            /// Lifecycle state (`STATUS_*`).
            pub status: u8,
            /// Set alongside `queue_failed`.
            pub failed: bool,
            /// The tic this row entered its current `status` — the lease / drop-window clock.
            pub tic_state_change: u32,
        }

        // ── data side ──────────────────────────────────────────────────────────────

        /// The sparse change log + frontier. `dirty > 0` = a pending write (value = event
        /// count, informational); `dirty == 0` = the resolved value at that tic.
        #[table(accessor = state_log, public)]
        pub struct StateLog {
            #[primary_key]
            #[auto_inc]
            pub id: u64,
            #[index(btree)]
            pub entity_key: u64,
            pub tic: u32,
            pub dirty: u16,
            $( pub $pf: $pt, )*
        }

        /// The client-visible latest per entity.
        #[table(accessor = state, public)]
        pub struct State {
            #[primary_key]
            pub entity_key: u64,
            pub tic: u32,
            $( pub $pf: $pt, )*
        }

        /// A resolved target's new state, passed to `resolve` — one per target the row wrote.
        /// The macro emits it with the payload so a whole row commits atomically in one call
        /// (`docs/issues/001-multi-target-resolve.md`).
        #[derive(SpacetimeType)]
        pub struct TargetState {
            pub entity_key: u64,
            $( pub $pf: $pt, )*
        }

        /// Refcount for dumb GC: which `event_reference`s hold a pending read/write on a
        /// `state_log` row. GC reclaims a row only when it has no holders (+ not-latest + old).
        #[table(accessor = holder, public)]
        pub struct Holder {
            #[primary_key]
            #[auto_inc]
            pub id: u64,
            #[index(btree)]
            pub state_log_id: u64,
            #[index(btree)]
            pub event_reference: u64,
            pub kind: u8,
        }

        /// Which `state_log` rows an enqueue stood up — so a re-drive finishes or backs out
        /// idempotently.
        #[table(accessor = open_rows, public)]
        pub struct OpenRows {
            #[primary_key]
            #[auto_inc]
            pub id: u64,
            #[index(btree)]
            pub event_reference: u64,
            pub state_log_id: u64,
        }

        // ── cold side (schema now; find-or-mint / PACK land in S6) ──────────────────

        /// Settled, packed static objects — one row per `(zone, type_reference)`.
        #[table(accessor = cold, public)]
        pub struct Cold {
            #[primary_key]
            pub cold_key: u64,
            #[index(btree)]
            pub zone_id: u32,
            pub type_reference: u32,
            pub kinds: Vec<u32>,
            pub version: u32,
        }

        /// The per-zone removal delta (`x:4|y:4|layer:4|type_id:4`) a mint appends to instead
        /// of rewriting the big `cold` row; a `PACK` action compacts it.
        #[table(accessor = cold_removed, public)]
        pub struct ColdRemoved {
            #[primary_key]
            pub zone_key: u32,
            pub removed: Vec<u16>,
            pub version: u32,
        }

        // ── meta ─────────────────────────────────────────────────────────────────

        #[table(accessor = tic_meta, public)]
        pub struct TicMeta {
            #[primary_key]
            pub id: u8,
            pub master_tic: u32,
        }

        #[table(accessor = shard_meta, public)]
        pub struct ShardMeta {
            #[primary_key]
            pub id: u8,
            pub shard_id: u16,
        }

        #[table(accessor = object_counter)]
        pub struct ObjectCounter {
            #[primary_key]
            pub id: u8,
            pub next: u32,
        }

        // ── helpers ────────────────────────────────────────────────────────────────

        pub fn master_tic(ctx: &ReducerContext) -> u32 {
            ctx.db.tic_meta().id().find(0).map_or(0, |m| m.master_tic)
        }

        fn set_master_tic(ctx: &ReducerContext, tic: u32) {
            ctx.db.tic_meta().id().delete(0);
            ctx.db.tic_meta().insert(TicMeta { id: 0, master_tic: tic });
        }

        fn this_shard_id(ctx: &ReducerContext) -> u16 {
            ctx.db.shard_meta().id().find(0).map_or(0, |m| m.shard_id)
        }

        fn next_entity_id(ctx: &ReducerContext) -> u32 {
            match ctx.db.object_counter().id().find(0) {
                Some(c) => {
                    ctx.db.object_counter().id().delete(0);
                    ctx.db.object_counter().insert(ObjectCounter { id: 0, next: c.next.saturating_add(1) });
                    c.next
                }
                None => {
                    ctx.db.object_counter().insert(ObjectCounter { id: 0, next: 1 });
                    0
                }
            }
        }

        /// Mint a fresh globally-unique minted `entity_reference` of `entity_type`.
        pub fn mint_entity(ctx: &ReducerContext, entity_type: u8) -> u64 {
            $crate::pack_minted_entity(entity_type, next_entity_id(ctx), this_shard_id(ctx))
        }

        /// The `state_log` row for `(entity_key, tic)`, if any.
        fn find_state_log(ctx: &ReducerContext, entity_key: u64, tic: u32) -> Option<StateLog> {
            ctx.db.state_log().entity_key().filter(entity_key).find(|r| r.tic == tic)
        }

        /// The entity's most recent resolved (`dirty==0`) row at or below `tic`.
        fn base_resolved(ctx: &ReducerContext, entity_key: u64, tic: u32) -> Option<StateLog> {
            ctx.db.state_log().entity_key().filter(entity_key)
                .filter(|r| r.dirty == 0 && r.tic <= tic).max_by_key(|r| r.tic)
        }

        fn promote(ctx: &ReducerContext, r: &StateLog) {
            ctx.db.state().entity_key().delete(r.entity_key);
            ctx.db.state().insert(State {
                entity_key: r.entity_key, tic: r.tic, $( $pf: r.$pf.clone(), )*
            });
        }

        /// Register that `event_reference` holds `kind` on `state_log_id`, and remember it in
        /// open-rows (so enqueue can back out).
        fn add_hold(ctx: &ReducerContext, event_reference: u64, state_log_id: u64, kind: u8) {
            ctx.db.holder().insert(Holder { id: 0, state_log_id, event_reference, kind });
            ctx.db.open_rows().insert(OpenRows { id: 0, event_reference, state_log_id });
        }

        /// Release every hold + open-row for an event.
        fn release_holds(ctx: &ReducerContext, event_reference: u64) {
            let hs: Vec<u64> = ctx.db.holder().event_reference().filter(event_reference).map(|h| h.id).collect();
            for id in hs { ctx.db.holder().id().delete(id); }
            let os: Vec<u64> = ctx.db.open_rows().event_reference().filter(event_reference).map(|o| o.id).collect();
            for id in os { ctx.db.open_rows().id().delete(id); }
        }

        fn set_status(ctx: &ReducerContext, mut row: EventLog, status: u8) {
            let tic = master_tic(ctx);
            ctx.db.event_log().event_reference().delete(row.event_reference);
            row.status = status;
            row.tic_state_change = tic;
            ctx.db.event_log().insert(row);
        }

        // ── reducers ─────────────────────────────────────────────────────────────

        /// Set this server's reference — called once at deploy/seed. Idempotent upsert.
        #[reducer]
        pub fn set_shard_id(ctx: &ReducerContext, shard_id: u16) -> Result<(), String> {
            ctx.db.shard_meta().id().delete(0);
            ctx.db.shard_meta().insert(ShardMeta { id: 0, shard_id });
            Ok(())
        }

        /// Append an event at `master + TIC_GAP`, status `enqueue`. `targets` are the
        /// entity_keys the row will write (issuer-designated). The caller mints keys for
        /// spawns and includes them here.
        #[reducer]
        pub fn append(ctx: &ReducerContext, actions: Vec<u64>, targets: Vec<u64>) -> Result<(), String> {
            let tic = master_tic(ctx) + $crate::TIC_GAP;
            ctx.db.event_log().insert(EventLog {
                event_reference: 0,
                event_tic: tic,
                actions,
                targets,
                worker_reference: $crate::WORKER_NONE,
                status: $crate::STATUS_ENQUEUE,
                failed: false,
                tic_state_change: master_tic(ctx),
            });
            Ok(())
        }

        /// A worker claims (fences) a row for its next phase: `enqueue → queueing` or
        /// `in_queue → running`. Succeeds if unclaimed or the lease expired.
        #[reducer]
        pub fn claim(ctx: &ReducerContext, worker_reference: u16, event_reference: u64) -> Result<(), String> {
            let Some(row) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            let now = master_tic(ctx);
            // Free to claim if unowned, already ours (re-claim across phases:
            // enqueue→queueing then in_queue→running), or the lease expired (eviction).
            let free = row.worker_reference == $crate::WORKER_NONE
                || row.worker_reference == worker_reference
                || now.saturating_sub(row.tic_state_change) > $crate::CLAIM_LEASE_TICS;
            let next = match row.status {
                s if s == $crate::STATUS_ENQUEUE => $crate::STATUS_QUEUEING,
                s if s == $crate::STATUS_IN_QUEUE => $crate::STATUS_RUNNING,
                _ => return Ok(()), // not a claimable phase
            };
            if !free { return Ok(()); }
            let mut row = row;
            row.worker_reference = worker_reference;
            row.status = next;
            row.tic_state_change = now;
            ctx.db.event_log().event_reference().delete(event_reference);
            ctx.db.event_log().insert(row);
            Ok(())
        }

        /// Stand up a pending `state_log` row for a hot `target` at the row's `event_tic`, and
        /// register a write hold. Idempotent (skips if the row already exists). Cold targets
        /// (`find-or-mint`) land in S6; this handles the hot path (spawn/move).
        #[reducer]
        pub fn stand_up(ctx: &ReducerContext, worker_reference: u16, event_reference: u64, target: u64) -> Result<(), String> {
            let Some(ev) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if ev.worker_reference != worker_reference || ev.status != $crate::STATUS_QUEUEING { return Ok(()); }
            let tic = ev.event_tic;
            let row_id = match find_state_log(ctx, target, tic) {
                Some(r) => r.id, // already stood up (idempotent)
                None => {
                    let base = base_resolved(ctx, target, tic);
                    let r = ctx.db.state_log().insert(StateLog {
                        id: 0, entity_key: target, tic, dirty: 1,
                        $( $pf: base.as_ref().map_or(Default::default(), |b| b.$pf.clone()), )*
                    });
                    r.id
                }
            };
            add_hold(ctx, event_reference, row_id, $crate::HOLD_WRITE);
            Ok(())
        }

        /// Mark a fully-stood-up row ready to execute.
        #[reducer]
        pub fn ready(ctx: &ReducerContext, worker_reference: u16, event_reference: u64) -> Result<(), String> {
            let Some(row) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if row.worker_reference != worker_reference || row.status != $crate::STATUS_QUEUEING { return Ok(()); }
            set_status(ctx, row, $crate::STATUS_IN_QUEUE);
            Ok(())
        }

        /// Commit a whole row atomically: write every target's resolved state (dirty=0),
        /// promote to `state`, release the row's holds, mark `complete`. Fenced.
        #[reducer]
        pub fn resolve(ctx: &ReducerContext, worker_reference: u16, event_reference: u64, results: Vec<TargetState>) -> Result<(), String> {
            let Some(ev) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if ev.worker_reference != worker_reference || ev.status != $crate::STATUS_RUNNING { return Ok(()); }
            let tic = ev.event_tic;
            for ts in results {
                if let Some(r) = find_state_log(ctx, ts.entity_key, tic) {
                    ctx.db.state_log().id().delete(r.id);
                }
                let resolved = ctx.db.state_log().insert(StateLog {
                    id: 0, entity_key: ts.entity_key, tic, dirty: 0,
                    $( $pf: ts.$pf, )*
                });
                promote(ctx, &resolved);
            }
            release_holds(ctx, event_reference);
            set_status(ctx, ev, $crate::STATUS_COMPLETE);
            Ok(())
        }

        /// Abort a `running` row (its `await` timed out): terminal-fail it + release holds.
        /// Fenced. Reuses `queue_failed` (both mean "terminal, produced nothing").
        #[reducer]
        pub fn abort(ctx: &ReducerContext, worker_reference: u16, event_reference: u64) -> Result<(), String> {
            let Some(row) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if row.worker_reference != worker_reference || row.status != $crate::STATUS_RUNNING { return Ok(()); }
            release_holds(ctx, event_reference);
            let mut row = row;
            row.status = $crate::STATUS_QUEUE_FAILED;
            row.failed = true;
            row.tic_state_change = master_tic(ctx);
            ctx.db.event_log().event_reference().delete(event_reference);
            ctx.db.event_log().insert(row);
            Ok(())
        }

        /// Advance the metronome one tic. Idempotent: a repeat/stale `to_tic` is a no-op; a
        /// gap (`> master+1`) is ignored. The master calls `drop_timed_out` *before* this.
        #[reducer]
        pub fn bump(ctx: &ReducerContext, to_tic: u32) -> Result<(), String> {
            let cur = master_tic(ctx);
            if to_tic != cur + 1 { return Ok(()); }
            set_master_tic(ctx, to_tic);
            Ok(())
        }

        /// The master's per-tic drop barrier: `queue_failed` every still-`enqueue`/`queueing`
        /// row whose window expired, backing out its holds. Causality guard (no watermark).
        #[reducer]
        pub fn drop_timed_out(ctx: &ReducerContext) -> Result<(), String> {
            let now = master_tic(ctx);
            let stale: Vec<u64> = ctx.db.event_log().iter()
                .filter(|e| (e.status == $crate::STATUS_ENQUEUE || e.status == $crate::STATUS_QUEUEING)
                    && now.saturating_sub(e.tic_state_change) > $crate::ENQUEUE_WINDOW_TICS)
                .map(|e| e.event_reference).collect();
            for r in stale {
                release_holds(ctx, r);
                if let Some(row) = ctx.db.event_log().event_reference().find(r) {
                    let mut row = row;
                    row.status = $crate::STATUS_QUEUE_FAILED;
                    row.failed = true;
                    row.tic_state_change = now;
                    ctx.db.event_log().event_reference().delete(r);
                    ctx.db.event_log().insert(row);
                }
            }
            Ok(())
        }

        /// Dumb reclamation. Drop `state_log` rows with **no holders**, **not the entity's
        /// latest resolved**, and old. Drop terminal (`complete`/`queue_failed`) event rows.
        #[reducer]
        pub fn tick_gc(ctx: &ReducerContext) -> Result<(), String> {
            // 1. terminal events.
            let done: Vec<u64> = ctx.db.event_log().iter()
                .filter(|e| e.status == $crate::STATUS_COMPLETE || e.status == $crate::STATUS_QUEUE_FAILED)
                .map(|e| e.event_reference).collect();
            for r in done { ctx.db.event_log().event_reference().delete(r); }

            // 2. per entity, keep the latest resolved row.
            let mut latest: ::std::collections::HashMap<u64, u32> = ::std::collections::HashMap::new();
            for r in ctx.db.state_log().iter().filter(|r| r.dirty == 0) {
                latest.entry(r.entity_key).and_modify(|m| { if r.tic > *m { *m = r.tic; } }).or_insert(r.tic);
            }
            let horizon = master_tic(ctx).saturating_sub($crate::CLAIM_LEASE_TICS);
            let drop: Vec<u64> = ctx.db.state_log().iter().filter(|r| {
                r.dirty == 0
                    && r.tic < horizon
                    && latest.get(&r.entity_key) != Some(&r.tic)
                    && ctx.db.holder().state_log_id().filter(r.id).count() == 0
            }).map(|r| r.id).collect();
            for id in drop { ctx.db.state_log().id().delete(id); }
            Ok(())
        }

        /// Seed an entity's resolved state directly (bulk-load / harness). Idempotent per key.
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn seed_entity(ctx: &ReducerContext, entity_key: u64, $( $pf: $pt, )*) -> Result<(), String> {
            let tic = master_tic(ctx);
            let old: Vec<u64> = ctx.db.state_log().entity_key().filter(entity_key).map(|r| r.id).collect();
            for id in old { ctx.db.state_log().id().delete(id); }
            let row = ctx.db.state_log().insert(StateLog {
                id: 0, entity_key, tic, dirty: 0, $( $pf, )*
            });
            promote(ctx, &row);
            Ok(())
        }
    };
}
