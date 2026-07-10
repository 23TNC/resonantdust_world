//! The simulation tick pipeline (object shard) — `docs/simulation.md` /
//! `docs/simulation-plan.md`, Phase A, and `docs/pipeline-generalization.md`.
//!
//! This is the mechanical, single-shard, atomic half of the pipeline: the tables
//! plus the reducers (`append_event`, `bump`, `claim`, `resolve`, `seed_entity`,
//! `spawn_object`, `tick_gc`, `set_shard_id`). The *policy* — read rule, priority-DAG,
//! action composition — lives in the external worker (`server_simulation`, using
//! `resonantdust-tick`); the module only stores events, generates work, fences claims,
//! and commits precomputed results.
//!
//! ## The `decl_tick_pipeline!` macro
//!
//! The engine's scheduling spine never inspects an entity's *payload* — it only ever
//! copies the game fields (`kind`/`zone_id`/`location`/`rotation`/`offset`/`data0`/
//! `data1`) verbatim. So the whole engine is emitted by a macro that takes the payload
//! field list as a parameter; changing the payload (or a second shard class with a
//! different one) is a change to the macro's argument, not the engine. Phase 1
//! (`pipeline-generalization.md`) invokes it **in-crate** with today's exact payload, so
//! the generated schema is byte-identical to the hand-written one it replaces. Making
//! the macro `#[macro_export]` and invoking it per-module (for divergent payloads) is a
//! later phase; the `$crate`-path + per-module-codec-dep work is deferred until payloads
//! actually diverge.
//!
//! Tic relationship (`event_tic = master_tic + 2`): on `bump` to `M` we generate work
//! for tic `M` (pending `state_log` rows for entities with events at `M`); a worker
//! only ever resolves a pending row whose tic `≤ master_tic`, so `resolve` always
//! promotes into `state`. The `+2` gap means every event a worker resolves is already
//! sealed (no new event for tic `M` can arrive once `master_tic ≥ M-1`).

use std::collections::HashMap;

use spacetimedb::{reducer, table, ReducerContext, Table};

use resonantdust_codec::packed::{pack_object_id, pack_object_key, pack_object_reference};

/// An event row is live until its target resolves the tic; then it's marked complete
/// (soft-delete for debug; the GC sweep hard-deletes later — Phase A4).
const STATUS_ACTIVE: u8 = 0;
const STATUS_COMPLETE: u8 = 1;

/// How long (seconds) a claim holds before another worker may evict it. Deliberately
/// a few tics at 2 Hz; tuned with the reaper in Phase A4.
const CLAIM_LEASE_SECS: u32 = 5;

/// `server_id == 0` means "unclaimed".
const SERVER_NONE: u16 = 0;

/// Seconds since the unix epoch, from the reducer's timestamp. Payload-agnostic, so it
/// lives outside the pipeline macro (referenced by simple name at the in-crate
/// expansion site).
fn now_secs(ctx: &ReducerContext) -> u32 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000) as u32
}

/// Emit a complete tick-pipeline: the six tables (`event_log`, `state_log`, `state`,
/// `tic_meta`, `shard_meta`, `object_counter`), their helpers, and the reducers. The
/// only parameter is the `payload` field list carried through `state_log`/`state` and
/// the seed/spawn/resolve reducer signatures — everything else (ids, tic bookkeeping,
/// fencing, work-gen, GC) is fixed for every pipeline.
///
/// `payload` fields are emitted, in declared order, in exactly the position the
/// hand-written schema had them (`state_log`: after `dirty`, before `server_id`;
/// `state`: after `tic`; reducer args: after the fixed prefix), so the generated
/// SpacetimeDB schema — and thus the server bindings — are byte-identical.
macro_rules! decl_tick_pipeline {
    (
        payload: { $( $pf:ident : $pt:ty ),* $(,)? } $(,)?
    ) => {
        // ── tables ─────────────────────────────────────────────────────────────

        /// Inbound intent, appended by the edge at `event_tic`, sealed by the `+2` gap,
        /// and consumed by the worker resolving the target. Ordered within a target by
        /// the `auto_inc` `event_reference` (per-shard, sufficient for per-target order).
        #[table(accessor = event_log, public)]
        pub struct EventLog {
            #[primary_key]
            #[auto_inc]
            pub event_reference: u64,
            #[index(btree)]
            pub event_tic: u32,
            pub from_server_id: u16,
            pub actor_shard_id: u16,
            pub actor_key: u64,
            #[index(btree)]
            pub target_key: u64,
            pub action: u16,
            pub data0: u64,
            pub data1: u64,
            pub status: u8,
        }

        /// The sparse change log + work items + frontier, all in one. `dirty>0` = a
        /// pending work item (value = event count, informational); `dirty==0` = the
        /// resolved value at that tic. Idle entities write nothing here. Identity is
        /// `(entity_key, tic)`; the `auto_inc` `id` is just a surface PK.
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
            /// Fence token: the worker currently assigned this `(entity_key, tic)`, or
            /// `SERVER_NONE`. `resolve` rejects a caller that isn't the current assignee.
            pub server_id: u16,
            pub created_at: u32,
            pub assigned_at: u32,
            pub status: u8,
        }

        /// The client-visible latest per entity, promoted from resolved `state_log` rows.
        /// Separate from `state_log` so clients never see unresolved / lookahead rows.
        #[table(accessor = state, public)]
        pub struct State {
            #[primary_key]
            pub entity_key: u64,
            pub tic: u32,
            $( pub $pf: $pt, )*
        }

        /// The single master-tic row (`id` always `0`). Lazy-seeded.
        #[table(accessor = tic_meta, public)]
        pub struct TicMeta {
            #[primary_key]
            pub id: u8,
            pub master_tic: u32,
        }

        /// This data shard's id — bits 32–47 of every object_id it mints. Single row
        /// (PK 0), `SHARD_NONE` (0) until `set_shard_id`. Must be globally unique and
        /// permanent.
        #[table(accessor = shard_meta, public)]
        pub struct ShardMeta {
            #[primary_key]
            pub id: u8,
            pub shard_id: u16,
        }

        /// The monotonic per-shard object counter — bits 0–31 of a minted object_id.
        /// Durable (survives restart), never reset independently of the objects it
        /// stamped.
        #[table(accessor = object_counter)]
        pub struct ObjectCounter {
            #[primary_key]
            pub id: u8,
            pub next: u32,
        }

        // ── helpers ────────────────────────────────────────────────────────────

        /// Current master tic (0 before the first `bump`).
        pub fn master_tic(ctx: &ReducerContext) -> u32 {
            ctx.db.tic_meta().id().find(0).map_or(0, |m| m.master_tic)
        }

        fn set_master_tic(ctx: &ReducerContext, tic: u32) {
            ctx.db.tic_meta().id().delete(0);
            ctx.db.tic_meta().insert(TicMeta { id: 0, master_tic: tic });
        }

        /// The `state_log` row for `(entity_key, tic)`, if any.
        fn find_state_log(ctx: &ReducerContext, entity_key: u64, tic: u32) -> Option<StateLog> {
            ctx.db
                .state_log()
                .entity_key()
                .filter(entity_key)
                .find(|r| r.tic == tic)
        }

        /// The entity's most recent resolved (`dirty==0`) `state_log` row at or below
        /// `tic` — the base a new pending row carries forward (position/kind/data of an
        /// entity that had no event this tic still needs to be represented when it does
        /// get one).
        fn base_resolved(ctx: &ReducerContext, entity_key: u64, tic: u32) -> Option<StateLog> {
            ctx.db
                .state_log()
                .entity_key()
                .filter(entity_key)
                .filter(|r| r.dirty == 0 && r.tic <= tic)
                .max_by_key(|r| r.tic)
        }

        /// Copy a resolved `state_log` row into the client-visible `state` table (upsert).
        fn promote(ctx: &ReducerContext, r: &StateLog) {
            ctx.db.state().entity_key().delete(r.entity_key);
            ctx.db.state().insert(State {
                entity_key: r.entity_key,
                tic: r.tic,
                $( $pf: r.$pf, )*
            });
        }

        /// Write an entity's initial resolved state at the current master tic (clearing
        /// any prior rows for it) and promote it. Shared by `seed_entity` and
        /// `spawn_object`.
        #[allow(clippy::too_many_arguments)]
        fn seed_write(
            ctx: &ReducerContext,
            entity_key: u64,
            $( $pf: $pt, )*
        ) {
            let tic = master_tic(ctx);
            let old: Vec<u64> = ctx
                .db
                .state_log()
                .entity_key()
                .filter(entity_key)
                .map(|r| r.id)
                .collect();
            for id in old {
                ctx.db.state_log().id().delete(id);
            }
            let now = now_secs(ctx);
            let row = ctx.db.state_log().insert(StateLog {
                id: 0,
                entity_key,
                tic,
                dirty: 0,
                $( $pf, )*
                server_id: SERVER_NONE,
                created_at: now,
                assigned_at: 0,
                status: STATUS_ACTIVE,
            });
            promote(ctx, &row);
        }

        fn this_shard_id(ctx: &ReducerContext) -> u16 {
            ctx.db.shard_meta().id().find(0).map_or(0, |m| m.shard_id)
        }

        /// Mint a fresh, globally-unique object_id: `(this shard's id, its next count)`.
        /// The count never repeats on this shard and the `shard_id` never repeats across
        /// shards, so the pair is unique everywhere with no coordination. The
        /// object-create entry point (spawn / a zone→object detach) calls this.
        pub fn mint_object_id(ctx: &ReducerContext) -> u64 {
            let count = match ctx.db.object_counter().id().find(0) {
                Some(c) => {
                    ctx.db.object_counter().id().delete(0);
                    ctx.db.object_counter().insert(ObjectCounter {
                        id: 0,
                        next: c.next.saturating_add(1),
                    });
                    c.next
                }
                None => {
                    ctx.db.object_counter().insert(ObjectCounter { id: 0, next: 1 });
                    0
                }
            };
            pack_object_id(this_shard_id(ctx), count)
        }

        // ── reducers ───────────────────────────────────────────────────────────

        /// Seed an entity at an explicit `entity_key` — the bulk-load / harness entry
        /// point. Idempotent per entity_key.
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn seed_entity(
            ctx: &ReducerContext,
            entity_key: u64,
            $( $pf: $pt, )*
        ) -> Result<(), String> {
            seed_write(ctx, entity_key, $( $pf, )*);
            Ok(())
        }

        /// Set this shard's id — called once at deploy/seed. Idempotent upsert.
        #[reducer]
        pub fn set_shard_id(ctx: &ReducerContext, shard_id: u16) -> Result<(), String> {
            ctx.db.shard_meta().id().delete(0);
            ctx.db.shard_meta().insert(ShardMeta { id: 0, shard_id });
            Ok(())
        }

        /// Create a new object with a freshly-minted globally-unique object_id, of class
        /// `obj_type` and def `kind`, at the given position. The caller learns the id by
        /// observing the new `state` row (reducers can't return it).
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn spawn_object(
            ctx: &ReducerContext,
            obj_type: u8,
            $( $pf: $pt, )*
        ) -> Result<(), String> {
            let entity_key = pack_object_key(pack_object_reference(obj_type, mint_object_id(ctx)));
            seed_write(ctx, entity_key, $( $pf, )*);
            Ok(())
        }

        /// Append validated intent. The edge calls this; the shard stamps `event_tic` so
        /// tic logic stays server-side. AoE = one call per target.
        #[reducer]
        pub fn append_event(
            ctx: &ReducerContext,
            from_server_id: u16,
            actor_shard_id: u16,
            actor_key: u64,
            target_key: u64,
            action: u16,
            data0: u64,
            data1: u64,
        ) -> Result<(), String> {
            let event_tic = master_tic(ctx) + 2;
            ctx.db.event_log().insert(EventLog {
                event_reference: 0,
                event_tic,
                from_server_id,
                actor_shard_id,
                actor_key,
                target_key,
                action,
                data0,
                data1,
                status: STATUS_ACTIVE,
            });
            Ok(())
        }

        /// Advance the metronome one tic and generate work for it. Called by
        /// `server_master`. Idempotent: a repeat/stale `to_tic` (`≤ master`) is a no-op;
        /// a gap (`> master+1`) is ignored (master must not skip).
        #[reducer]
        pub fn bump(ctx: &ReducerContext, to_tic: u32) -> Result<(), String> {
            let cur = master_tic(ctx);
            if to_tic != cur + 1 {
                return Ok(());
            }
            set_master_tic(ctx, to_tic);

            // Work-gen for tic `to_tic`: group this tic's active events by target, and
            // create one pending row per target that doesn't already have one.
            let mut counts: HashMap<u64, u16> = HashMap::new();
            for e in ctx.db.event_log().event_tic().filter(to_tic) {
                if e.status == STATUS_ACTIVE {
                    *counts.entry(e.target_key).or_insert(0) += 1;
                }
            }
            let now = now_secs(ctx);
            for (target, count) in counts {
                if find_state_log(ctx, target, to_tic).is_some() {
                    continue; // idempotent
                }
                let base = base_resolved(ctx, target, to_tic);
                ctx.db.state_log().insert(StateLog {
                    id: 0,
                    entity_key: target,
                    tic: to_tic,
                    dirty: count,
                    $( $pf: base.as_ref().map_or(Default::default(), |b| b.$pf), )*
                    server_id: SERVER_NONE,
                    created_at: now,
                    assigned_at: 0,
                    status: STATUS_ACTIVE,
                });
            }
            Ok(())
        }

        /// A worker claims a pending `(entity_key, tic)`. Succeeds if unclaimed or the
        /// lease has expired; otherwise a no-op (the loser observes the row's `server_id`
        /// via its subscription and backs off). The claim is the fence token `resolve`
        /// checks.
        #[reducer]
        pub fn claim(ctx: &ReducerContext, server_id: u16, entity_key: u64, tic: u32) -> Result<(), String> {
            let Some(row) = find_state_log(ctx, entity_key, tic) else {
                return Ok(());
            };
            if row.dirty == 0 {
                return Ok(()); // already resolved
            }
            let now = now_secs(ctx);
            let free = row.server_id == SERVER_NONE || now.saturating_sub(row.assigned_at) > CLAIM_LEASE_SECS;
            if !free {
                return Ok(());
            }
            ctx.db.state_log().id().delete(row.id);
            ctx.db.state_log().insert(StateLog {
                id: 0, // fresh surface id; identity is (entity_key, tic)
                server_id,
                assigned_at: now,
                ..row
            });
            Ok(())
        }

        /// Commit a precomputed resolved state for `(entity_key, tic)`. Fenced: rejected
        /// (no-op) unless the caller is the current assignee. Writes the row `dirty=0`,
        /// marks the tic's events complete, and promotes to `state` (always — a pending
        /// row's tic is `≤ master`).
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn resolve(
            ctx: &ReducerContext,
            server_id: u16,
            entity_key: u64,
            tic: u32,
            $( $pf: $pt, )*
        ) -> Result<(), String> {
            let Some(row) = find_state_log(ctx, entity_key, tic) else {
                return Ok(());
            };
            if row.dirty == 0 || row.server_id != server_id {
                return Ok(()); // already resolved, or a stale/evicted caller — fence
            }
            ctx.db.state_log().id().delete(row.id);
            let resolved = ctx.db.state_log().insert(StateLog {
                id: 0, // fresh surface id; identity is (entity_key, tic)
                dirty: 0,
                $( $pf, )*
                server_id: SERVER_NONE,
                ..row
            });
            // Mark this tic's events for this target complete.
            let done: Vec<u64> = ctx
                .db
                .event_log()
                .target_key()
                .filter(entity_key)
                .filter(|e| e.event_tic == tic && e.status == STATUS_ACTIVE)
                .map(|e| e.event_reference)
                .collect();
            for r in done {
                if let Some(mut e) = ctx.db.event_log().event_reference().find(r) {
                    ctx.db.event_log().event_reference().delete(r);
                    e.status = STATUS_COMPLETE;
                    ctx.db.event_log().insert(e);
                }
            }
            promote(ctx, &resolved);
            Ok(())
        }

        /// Prune the working set. Called periodically by `server_master` (no `state_log`/
        /// `event_log` row is ever read again once past the horizon). Two sweeps:
        ///
        /// 1. **`event_log`**: hard-delete `status == COMPLETE` rows — a consumed event is
        ///    never resolved against again.
        /// 2. **`state_log`**: keep every pending (`dirty>0`) row and, per entity, its
        ///    latest resolved row (the base for its next tic + its idle-carry value that
        ///    other entities read). Also keep resolved rows at `tic ≥ min_pending − 1`,
        ///    since a resolution in progress at the lowest pending tic reads actors at
        ///    `tic − 1`. Drop the rest — the accumulated deep history.
        #[reducer]
        pub fn tick_gc(ctx: &ReducerContext) -> Result<(), String> {
            // 1. Completed events.
            let done: Vec<u64> = ctx
                .db
                .event_log()
                .iter()
                .filter(|e| e.status == STATUS_COMPLETE)
                .map(|e| e.event_reference)
                .collect();
            for r in done {
                ctx.db.event_log().event_reference().delete(r);
            }

            // 2. Horizon = (lowest pending tic) − 1; if nothing is pending, only each
            //    entity's latest resolved row is needed (no in-progress resolution reads
            //    history).
            let pending_min = ctx
                .db
                .state_log()
                .iter()
                .filter(|r| r.dirty > 0)
                .map(|r| r.tic)
                .min();
            let horizon = pending_min.map_or(u32::MAX, |g| g.saturating_sub(1));

            let mut latest: HashMap<u64, u32> = HashMap::new();
            for r in ctx.db.state_log().iter().filter(|r| r.dirty == 0) {
                latest
                    .entry(r.entity_key)
                    .and_modify(|m| {
                        if r.tic > *m {
                            *m = r.tic;
                        }
                    })
                    .or_insert(r.tic);
            }
            let drop: Vec<u64> = ctx
                .db
                .state_log()
                .iter()
                .filter(|r| {
                    r.dirty == 0 && r.tic < horizon && latest.get(&r.entity_key) != Some(&r.tic)
                })
                .map(|r| r.id)
                .collect();
            for id in drop {
                ctx.db.state_log().id().delete(id);
            }
            Ok(())
        }
    };
}

// ── the object/zone pipeline (Phase 1: today's exact payload) ──────────────────
//
// One in-crate instantiation carrying the spatial payload that objects and zones share
// today. `zone_id` is a routing key (the edge subscribes `WHERE zone_id`); it stays an
// ordinary column here — promoting it to an indexed routing key is a later refinement,
// kept out of the byte-identical no-op.
decl_tick_pipeline! {
    payload: {
        kind: u16,
        zone_id: u32,
        location: u8,
        rotation: u8,
        offset: u8,
        data0: u64,
        data1: u64,
    }
}
