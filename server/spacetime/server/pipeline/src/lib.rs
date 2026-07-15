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

pub use resonantdust_codec::object::pack_cold_row_reference;
pub use resonantdust_codec::refs::{entity_ref_zone_id, pack_hot_entity};

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
            pub event_reference: u32,
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
        ///
        /// A hold is keyed by **`(source_shard, event_reference)`**, not `event_reference` alone:
        /// a row on *another* shard can hold a row here (its target lives here — see
        /// [`stand_up_foreign`]), and each shard's `event_reference` is its own auto_inc sequence,
        /// so the numbers collide across shards. `source_shard == 0` means "this shard's own row".
        #[table(accessor = holder, public)]
        pub struct Holder {
            #[primary_key]
            #[auto_inc]
            pub id: u64,
            #[index(btree)]
            pub state_log_id: u64,
            #[index(btree)]
            pub event_reference: u32,
            /// The shard owning the holding row; `0` = local (this shard's own `event_log`).
            pub source_shard: u16,
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
            pub event_reference: u32,
            /// The shard owning the holding row; `0` = local (mirrors [`Holder::source_shard`]).
            pub source_shard: u16,
            pub state_log_id: u64,
        }

        /// Cross-shard convergence dedup. When a row on **another** shard writes a target that
        /// lives here, the worker calls [`resolve_foreign`] on this shard; the `(source_shard,
        /// event_reference)` pair is recorded so a crash-recovery re-drive is a no-op (idempotent
        /// per the row's identity — `docs/spacetime-tables/lifecycle.md` §convergent write).
        #[table(accessor = applied_foreign, public)]
        pub struct AppliedForeign {
            /// `(source_shard as u128) << 64 | event_reference` — the convergence idempotency key.
            #[primary_key]
            pub id: u128,
        }

        // ── cold side (schema now; find-or-mint / PACK land in S6) ──────────────────

        /// Settled, packed static objects — one row per **`(macro_position, type_reference,
        /// layer_id)`**, which *is* the row's identity: `cold_row_reference` is their composite
        /// (`reserved:28 | macro_position:16 | type_reference:16 | layer_id:4`), not a surrogate.
        /// `layer_id` is in the key because `layer` is a tile-slot, not a type property — rows
        /// sharing `(macro_position, type, subtype)` but differing in `layer` are distinct rows.
        /// Realm is implied by the shard, so the row carries `region|zone` only.
        #[table(accessor = cold, public)]
        pub struct Cold {
            #[primary_key]
            pub cold_row_reference: u64,
            /// `region_reference:8 | zone_reference:8` — the subscription key.
            #[index(btree)]
            pub macro_position: u16,
            /// `type_id:4 | subtype_id:12`.
            pub type_reference: u16,
            /// The tile-slot (u4).
            pub layer_id: u8,
            /// One `kind_pos_reference` per object: `kind_reference:16 | tile_reference:8 | data:8`.
            pub kinds: Vec<u32>,
            pub version: u32,
        }

        /// The removal delta — **1:1 with its `cold` row** (same `cold_row_reference`). A mint
        /// appends the object's `tile_reference` here instead of rewriting the big `cold` row; a
        /// `PACK` compacts it. Tombstones are a bare `tile_reference:u8` because macro/type/layer
        /// are already in the key, and an unpack re-sends only *this* row's small delta.
        #[table(accessor = cold_removed, public)]
        pub struct ColdRemoved {
            #[primary_key]
            pub cold_row_reference: u64,
            #[index(btree)]
            pub macro_position: u16,
            /// `tile_reference` (`x:4 | y:4`) tombstones.
            pub removed: Vec<u8>,
            pub version: u32,
        }

        // ── meta ─────────────────────────────────────────────────────────────────

        #[table(accessor = tic_meta, public)]
        pub struct TicMeta {
            #[primary_key]
            pub id: u8,
            pub master_tic: u32,
            /// Debug freeze: while `true`, `bump` no-ops so the tic (and thus the whole
            /// simulation) stops advancing. Set via [`set_paused`]; the master also skips its
            /// drop sweep while paused. Relayed to clients so tic-driven actors (npc) pause too.
            pub paused: bool,
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

        /// Whether the simulation is frozen (see [`set_paused`]).
        pub fn is_paused(ctx: &ReducerContext) -> bool {
            ctx.db.tic_meta().id().find(0).map_or(false, |m| m.paused)
        }

        fn set_master_tic(ctx: &ReducerContext, tic: u32) {
            let paused = is_paused(ctx); // preserve the freeze flag across the tic rewrite
            ctx.db.tic_meta().id().delete(0);
            ctx.db.tic_meta().insert(TicMeta { id: 0, master_tic: tic, paused });
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

        /// Mint a fresh globally-unique **hot** `entity_reference` — `REF_HOT` qualified by this
        /// shard's `server_reference`, handle = this shard's next `hot_reference`. (Game-type is
        /// no longer in the identity; it lives in the payload `kind` / `definition_reference`.)
        pub fn mint_hot(ctx: &ReducerContext) -> u64 {
            $crate::pack_hot_entity(this_shard_id(ctx), next_entity_id(ctx))
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
        fn add_hold(ctx: &ReducerContext, source_shard: u16, event_reference: u32, state_log_id: u64, kind: u8) {
            ctx.db.holder().insert(Holder { id: 0, state_log_id, event_reference, source_shard, kind });
            ctx.db.open_rows().insert(OpenRows { id: 0, event_reference, source_shard, state_log_id });
        }

        /// Release every hold + open-row for an event.
        /// Release every hold + open-row a row holds here. Keyed by `(source_shard,
        /// event_reference)` so releasing a local row never drops a same-numbered *foreign* row's
        /// holds (each shard mints its own `event_reference` sequence).
        fn release_holds(ctx: &ReducerContext, source_shard: u16, event_reference: u32) {
            let hs: Vec<u64> = ctx.db.holder().event_reference().filter(event_reference)
                .filter(|h| h.source_shard == source_shard).map(|h| h.id).collect();
            for id in hs { ctx.db.holder().id().delete(id); }
            let os: Vec<u64> = ctx.db.open_rows().event_reference().filter(event_reference)
                .filter(|o| o.source_shard == source_shard).map(|o| o.id).collect();
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

        /// Seed one **cold** row for a zone — insert-only-if-absent, so a worldgen re-run never
        /// clobbers a cold row later mutated in-world. The edge calls this once per `ColdRow` of a
        /// fresh zone. (Cold→hot `find-or-mint` on a targeted cold object is S6/S7.)
        #[reducer]
        pub fn seed_cold_row(ctx: &ReducerContext, macro_position: u16, type_reference: u16, layer_id: u8, kinds: Vec<u32>) -> Result<(), String> {
            let key = $crate::pack_cold_row_reference(macro_position, type_reference, layer_id);
            if ctx.db.cold().cold_row_reference().find(key).is_none() {
                ctx.db.cold().insert(Cold {
                    cold_row_reference: key, macro_position, type_reference, layer_id, kinds, version: 1,
                });
            }
            Ok(())
        }

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
        pub fn claim(ctx: &ReducerContext, worker_reference: u16, event_reference: u32) -> Result<(), String> {
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
                // Re-claim a stuck `running` row (its worker died — lease expired) or our own:
                // stays `running`, reassigned. Recovery for the execute phase.
                s if s == $crate::STATUS_RUNNING => $crate::STATUS_RUNNING,
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
        pub fn stand_up(ctx: &ReducerContext, worker_reference: u16, event_reference: u32, target: u64) -> Result<(), String> {
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
            add_hold(ctx, 0, event_reference, row_id, $crate::HOLD_WRITE);
            Ok(())
        }

        /// **Phase-1 hold for a cross-shard target.** The mirror of [`stand_up`] for a row owned by
        /// `source_shard` whose target lives *here*: stand up the pending `state_log` row at `tic`
        /// and register the write hold, so during the in-flight window this shard sees the target
        /// as having pending work — the **read rule** (`resolved_through`) defers readers instead of
        /// reading a soon-to-be-stale value, and **GC** won't reclaim the row (it has a holder).
        ///
        /// No local `event_log` row exists for the event (it lives on the source shard), so there's
        /// no fence and no status transition here — same shape as [`resolve_foreign`], and `tic` is
        /// carried explicitly for the same reason. Idempotent: a re-drive finds the row already
        /// stood up. The hold is keyed `(source_shard, event_reference)`; `resolve_foreign` releases
        /// it. A leaked hold (source row dropped mid-flight) is harmless — it only delays GC
        /// (`lifecycle.md`: release is tied to a definite terminal state; a crash may leak, never
        /// release early).
        #[reducer]
        pub fn stand_up_foreign(ctx: &ReducerContext, source_shard: u16, event_reference: u32, tic: u32, target: u64) -> Result<(), String> {
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
            // Don't double-hold on a re-drive: one write hold per (source_shard, event_reference, row).
            let held = ctx.db.holder().event_reference().filter(event_reference)
                .any(|h| h.source_shard == source_shard && h.state_log_id == row_id);
            if !held {
                add_hold(ctx, source_shard, event_reference, row_id, $crate::HOLD_WRITE);
            }
            Ok(())
        }

        /// Mark a fully-stood-up row ready to execute.
        #[reducer]
        pub fn ready(ctx: &ReducerContext, worker_reference: u16, event_reference: u32) -> Result<(), String> {
            let Some(row) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if row.worker_reference != worker_reference || row.status != $crate::STATUS_QUEUEING { return Ok(()); }
            set_status(ctx, row, $crate::STATUS_IN_QUEUE);
            Ok(())
        }

        /// Commit a whole row atomically: write every target's resolved state (dirty=0),
        /// promote to `state`, release the row's holds, mark `complete`. Fenced.
        #[reducer]
        pub fn resolve(ctx: &ReducerContext, worker_reference: u16, event_reference: u32, results: Vec<TargetState>) -> Result<(), String> {
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
            release_holds(ctx, 0, event_reference); // local row
            set_status(ctx, ev, $crate::STATUS_COMPLETE);
            Ok(())
        }

        /// Convergent cross-shard write: apply a row's targets that live on **this** shard when
        /// the row itself is owned by `source_shard`. Idempotent by `(source_shard,
        /// event_reference)` — a re-drive after a mid-write crash is a no-op. This shard holds no
        /// `event_log` row for the event (it lives on the source), so there's no fence and no
        /// status transition here; the **worker** completes the row on the source shard only once
        /// every target shard's `resolve_foreign` has returned (`lifecycle.md` §convergent write).
        /// `tic` is the row's `event_tic`, carried explicitly since this shard can't read it.
        #[reducer]
        pub fn resolve_foreign(ctx: &ReducerContext, source_shard: u16, event_reference: u32, tic: u32, results: Vec<TargetState>) -> Result<(), String> {
            let key = ((source_shard as u128) << 64) | (event_reference as u128);
            if ctx.db.applied_foreign().id().find(key).is_some() {
                return Ok(()); // already converged here — idempotent no-op
            }
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
            // This is the foreign row's **definite terminal** here — the write landed, so release
            // the Phase-1 holds [`stand_up_foreign`] took (keyed by the same `(source_shard,
            // event_reference)`), atomically with the write. Local holds are untouched.
            release_holds(ctx, source_shard, event_reference);
            ctx.db.applied_foreign().insert(AppliedForeign { id: key });
            Ok(())
        }

        /// Abort a `running` row (its `await` timed out): terminal-fail it + release holds.
        /// Fenced. Reuses `queue_failed` (both mean "terminal, produced nothing").
        #[reducer]
        pub fn abort(ctx: &ReducerContext, worker_reference: u16, event_reference: u32) -> Result<(), String> {
            let Some(row) = ctx.db.event_log().event_reference().find(event_reference) else { return Ok(()); };
            if row.worker_reference != worker_reference || row.status != $crate::STATUS_RUNNING { return Ok(()); }
            release_holds(ctx, 0, event_reference); // local row
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
            if is_paused(ctx) { return Ok(()); } // frozen — the tic does not advance while paused
            let cur = master_tic(ctx);
            if to_tic != cur + 1 { return Ok(()); }
            set_master_tic(ctx, to_tic);
            Ok(())
        }

        /// Freeze / unfreeze the simulation (debug). While `paused`, [`bump`] no-ops so the tic
        /// stops; the master also skips its drop sweep, and the flag is relayed to clients so
        /// tic-driven actors stop issuing commands. Idempotent; creates `tic_meta` if absent.
        #[reducer]
        pub fn set_paused(ctx: &ReducerContext, paused: bool) -> Result<(), String> {
            let tic = master_tic(ctx);
            ctx.db.tic_meta().id().delete(0);
            ctx.db.tic_meta().insert(TicMeta { id: 0, master_tic: tic, paused });
            Ok(())
        }

        /// The master's per-tic drop barrier: `queue_failed` every still-`enqueue`/`queueing`
        /// row whose window expired, backing out its holds. Causality guard (no watermark).
        #[reducer]
        pub fn drop_timed_out(ctx: &ReducerContext) -> Result<(), String> {
            let now = master_tic(ctx);
            let stale: Vec<u32> = ctx.db.event_log().iter()
                .filter(|e| (e.status == $crate::STATUS_ENQUEUE || e.status == $crate::STATUS_QUEUEING)
                    && now.saturating_sub(e.tic_state_change) > $crate::ENQUEUE_WINDOW_TICS)
                .map(|e| e.event_reference).collect();
            for r in stale {
                release_holds(ctx, 0, r); // local row
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
            let done: Vec<u32> = ctx.db.event_log().iter()
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

        /// **find-or-mint** a cold object into a hot entity keyed by its positional
        /// `entity_reference` `target` (the cold object's location). Idempotent: if a hot entity
        /// already exists at `target`, no-op. Else seed its resolved state from the (worker-
        /// decoded) payload, promote, and append the object's `tile_reference` to **its row's**
        /// `cold_removed` delta so the client omits it.
        ///
        /// `cold_row_reference` is the row the worker **selected** — `(macro_position, type_id,
        /// layer_id)` off the target's `cold_reference`, plus the row's `subtype_id` (the one field
        /// a position doesn't carry, which is why the worker resolves the row and passes it here).
        /// The tombstone is a bare `tile_reference`: macro/type/layer are already in the key.
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn mint_cold(ctx: &ReducerContext, target: u64, cold_row_reference: u64, macro_position: u16, tombstone: u8, $( $pf: $pt, )*) -> Result<(), String> {
            if ctx.db.state().entity_key().find(target).is_some() {
                return Ok(()); // already hot at this location — idempotent
            }
            let tic = master_tic(ctx);
            let row = ctx.db.state_log().insert(StateLog { id: 0, entity_key: target, tic, dirty: 0, $( $pf, )* });
            promote(ctx, &row);
            match ctx.db.cold_removed().cold_row_reference().find(cold_row_reference) {
                Some(r) => {
                    let mut removed = r.removed.clone();
                    if !removed.contains(&tombstone) { removed.push(tombstone); }
                    let version = r.version.saturating_add(1);
                    ctx.db.cold_removed().cold_row_reference().delete(cold_row_reference);
                    ctx.db.cold_removed().insert(ColdRemoved { cold_row_reference, macro_position, removed, version });
                }
                None => {
                    ctx.db.cold_removed().insert(ColdRemoved { cold_row_reference, macro_position, removed: vec![tombstone], version: 1 });
                }
            }
            Ok(())
        }

        /// **PACK** — settle an *idle* hot entity back into `cold` (the reverse of find-or-mint).
        /// Skips a busy object (any holder on its `state_log` rows), atomically with the write.
        /// Appends the worker-composed `kind_pos_reference` to **its** cold row
        /// (`cold_row_reference`), drops the object's `tile_reference` tombstone from that row's
        /// `cold_removed` (it's cold again), and deletes the hot `state`/`state_log`. The worker
        /// composes the entry + resolves the row (the module is payload-generic — issue 005).
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn pack_settle(ctx: &ReducerContext, entity_key: u64, cold_row_reference: u64, macro_position: u16, type_reference: u16, layer_id: u8, kind_pos_reference: u32, tombstone: u8) -> Result<(), String> {
            // Only settle an object at rest — one with no pending read/write holds.
            let busy = ctx.db.state_log().entity_key().filter(entity_key)
                .any(|r| ctx.db.holder().state_log_id().filter(r.id).count() > 0);
            if busy { return Ok(()); }
            match ctx.db.cold().cold_row_reference().find(cold_row_reference) {
                Some(c) => {
                    let mut kinds = c.kinds.clone();
                    if !kinds.contains(&kind_pos_reference) { kinds.push(kind_pos_reference); }
                    let version = c.version.saturating_add(1);
                    ctx.db.cold().cold_row_reference().delete(cold_row_reference);
                    ctx.db.cold().insert(Cold { cold_row_reference, macro_position, type_reference, layer_id, kinds, version });
                }
                None => {
                    ctx.db.cold().insert(Cold {
                        cold_row_reference, macro_position, type_reference, layer_id,
                        kinds: vec![kind_pos_reference], version: 1,
                    });
                }
            }
            if let Some(r) = ctx.db.cold_removed().cold_row_reference().find(cold_row_reference) {
                let removed: Vec<u8> = r.removed.iter().copied().filter(|&t| t != tombstone).collect();
                let version = r.version.saturating_add(1);
                ctx.db.cold_removed().cold_row_reference().delete(cold_row_reference);
                ctx.db.cold_removed().insert(ColdRemoved { cold_row_reference, macro_position, removed, version });
            }
            ctx.db.state().entity_key().delete(entity_key);
            let ids: Vec<u64> = ctx.db.state_log().entity_key().filter(entity_key).map(|r| r.id).collect();
            for id in ids { ctx.db.state_log().id().delete(id); }
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
