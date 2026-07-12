//! The simulation tick pipeline — `docs/simulation.md` / `docs/simulation-plan.md`,
//! Phase A, and `docs/pipeline-generalization.md`.
//!
//! This crate provides the **`decl_tick_pipeline!` macro** plus the payload-agnostic
//! helpers it reaches through `$crate::`. It defines no tables or reducers itself: a
//! SpacetimeDB **module** invokes the macro with its own payload, and the tables +
//! reducers register in *that* module. So one engine serves any number of pipelines with
//! divergent payloads (object, zone, …), each in its own module — see the invocation in
//! `spacetime/server/modules/shard/src/lib.rs`.
//!
//! The engine's scheduling spine never inspects the payload — it only ever copies the
//! game fields verbatim — so the whole engine is emitted by a macro taking the payload
//! field list as a parameter. The macro is `#[macro_export]` and self-contained (it
//! emits its own `::spacetimedb` prelude and reaches this crate's constants/helpers via
//! `$crate::`), so an invoking module needs only `spacetimedb` + `resonantdust_pipeline`
//! as deps — not the codec.
//!
//! Tic relationship (`event_tic = master_tic + 2`): on `bump` to `M` we generate work
//! for tic `M` (pending `state_log` rows for entities with events at `M`); a worker only
//! ever resolves a pending row whose tic `≤ master_tic`, so `resolve` always promotes
//! into `state`. The `+2` gap means every event a worker resolves is already sealed (no
//! new event for tic `M` can arrive once `master_tic ≥ M-1`).

use spacetimedb::ReducerContext;

/// Re-exported so the macro can reach it via `$crate::pack_minted_entity` from any module
/// that invokes it — without that module depending on the codec directly.
pub use resonantdust_codec::refs::pack_minted_entity;

/// An event row is live until its target resolves the tic; then it's marked complete
/// (soft-delete for debug; the GC sweep hard-deletes later — Phase A4).
pub const STATUS_ACTIVE: u8 = 0;
pub const STATUS_COMPLETE: u8 = 1;

/// How long (seconds) a claim holds before another worker may evict it. Deliberately a
/// few tics at 2 Hz; tuned with the reaper in Phase A4.
pub const CLAIM_LEASE_SECS: u32 = 5;

/// `server_id == 0` means "unclaimed".
pub const SERVER_NONE: u16 = 0;

/// Seconds since the unix epoch, from the reducer's timestamp. Payload-agnostic and
/// SDK-only, so it lives here (reached via `$crate::now_secs`) rather than in the macro.
pub fn now_secs(ctx: &ReducerContext) -> u32 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000) as u32
}

/// Emit a complete tick-pipeline into the invoking module: the six tables (`event_log`,
/// `state_log`, `state`, `tic_meta`, `shard_meta`, `object_counter`), their helpers, and
/// the reducers. The only parameter is the `payload` field list carried through
/// `state_log`/`state` and the seed/spawn/resolve reducer signatures — everything else
/// (ids, tic bookkeeping, fencing, work-gen, GC) is fixed for every pipeline.
///
/// `payload` fields are emitted, in declared order, in the position the hand-written
/// schema had them (`state_log`: after `dirty`; `state`: after `tic`; reducer args: after
/// the fixed prefix). Field values are `.clone()`d on carry-forward/promote, so a payload
/// may carry non-`Copy` fields (e.g. a `Vec<u64>` zone-cell array) as well as scalars.
///
/// Self-contained: the expansion opens with its own `use ::spacetimedb::{…}` and reaches
/// `$crate::{SERVER_NONE, STATUS_ACTIVE, …, now_secs, pack_minted_entity}`, so it resolves
/// in any module depending on `spacetimedb` + `resonantdust_pipeline`.
#[macro_export]
macro_rules! decl_tick_pipeline {
    (
        payload: { $( $pf:ident : $pt:ty ),* $(,)? } $(,)?
    ) => {
        use ::spacetimedb::{reducer, table, ReducerContext, Table};

        // ── tables ─────────────────────────────────────────────────────────────

        /// Inbound intent, appended by the edge at `event_tic`, sealed by the `+2` gap,
        /// and consumed by the worker resolving the target. Ordered within a target by
        /// the `auto_inc` `event_reference` (per-shard, sufficient for per-target order).
        #[table(accessor = event_log, public)]
        pub struct EventLog {
            /// Per-shard identity + composition order (`auto_inc`). Cross-shard, a trigger
            /// is located by the `(trigger_server_reference, trigger_event_reference)` pair,
            /// so no global minting is needed.
            #[primary_key]
            #[auto_inc]
            pub event_reference: u64,
            #[index(btree)]
            pub event_tic: u32,
            /// Who injected this event (edge / npc / worker) — audit; not read during
            /// resolution.
            pub source_server_reference: u16,
            /// The shard where `actor_key` lives — the cross-shard actor-read target.
            pub actor_server_reference: u16,
            /// The worker that generated this event (`0` if externally injected) —
            /// provenance: validated against the trigger's recorded `worker_server_reference`.
            pub requesting_server_reference: u16,
            /// The worker that RESOLVED this event's target (stamped by `resolve` — the
            /// fence winner). A follow-on reads this on its trigger to validate provenance.
            pub worker_server_reference: u16,
            /// The shard where `trigger_event_reference` lives (`0` if none) — locates the
            /// trigger for validation.
            pub trigger_server_reference: u16,
            /// The event that generated this one (`0` = none); with
            /// `trigger_server_reference`, the cross-shard pair identifying the trigger.
            pub trigger_event_reference: u64,
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
            /// `0` (unclaimed). `resolve` rejects a caller that isn't the current assignee.
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

        /// This data server's reference — bits 8–23 of every minted `entity_reference`.
        /// Single row (PK 0), `0` (unset) until `set_shard_id`. Must be globally unique
        /// and permanent.
        #[table(accessor = shard_meta, public)]
        pub struct ShardMeta {
            #[primary_key]
            pub id: u8,
            pub shard_id: u16,
        }

        /// The monotonic per-server entity counter — the `entity_id` of a minted
        /// `entity_reference`. Durable (survives restart), never reset independently of
        /// the entities it stamped.
        #[table(accessor = object_counter)]
        pub struct ObjectCounter {
            #[primary_key]
            pub id: u8,
            pub next: u32,
        }

        /// The **cold** table — a module's settled, packed objects, the static
        /// counterpart to the ticking hot `state`. One row per
        /// `(zone_id, object_type_reference)`: the shared type half plus a
        /// `Vec<object_kind_reference>` (a `ColdRow`; see `docs/object-model.md` §4,
        /// `docs/data-shards.md`). `biome-tile` rows are the dense ground, `biome-thing`
        /// rows the sparse scatter — a row's `subtype` *is* its zone's biome. Uniform
        /// across modules (object_kind_references are type-agnostic `u32`s), unlike the
        /// payload-parameterised `state`. NOT tick-managed (never `dirty`; `tick_gc`
        /// ignores it). `unpack` promotes a cold object to a hot `state` entity; `pack`
        /// folds a settled hot entity back — within this one module. `zone_id` is a
        /// routing column so the edge subscribes `WHERE zone_id`; `cold_key` packs
        /// `(zone_id, type_reference)` into the single-column PK.
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
        /// `tic` — the base a new pending row carries forward.
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
                $( $pf: r.$pf.clone(), )*
            });
        }

        /// Write an entity's initial resolved state at the current master tic (clearing
        /// any prior rows for it) and promote it. Shared by `seed_entity`/`spawn_object`.
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
            let now = $crate::now_secs(ctx);
            let row = ctx.db.state_log().insert(StateLog {
                id: 0,
                entity_key,
                tic,
                dirty: 0,
                $( $pf, )*
                server_id: $crate::SERVER_NONE,
                created_at: now,
                assigned_at: 0,
                status: $crate::STATUS_ACTIVE,
            });
            promote(ctx, &row);
        }

        fn this_shard_id(ctx: &ReducerContext) -> u16 {
            ctx.db.shard_meta().id().find(0).map_or(0, |m| m.shard_id)
        }

        /// The monotonic per-server entity counter — the `entity_id` half of a minted
        /// `entity_reference`. Returns the next id and advances it.
        fn next_entity_id(ctx: &ReducerContext) -> u32 {
            match ctx.db.object_counter().id().find(0) {
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
            }
        }

        /// Mint a fresh, globally-unique **minted** `entity_reference` of `entity_type`:
        /// `(entity_type, this server's next entity_id, this server's reference)`. Unique
        /// everywhere with no coordination.
        pub fn mint_entity(ctx: &ReducerContext, entity_type: u8) -> u64 {
            $crate::pack_minted_entity(entity_type, next_entity_id(ctx), this_shard_id(ctx))
        }

        /// The cold-row PK: `(zone_id:32 << 32) | type_reference:32` — a single-column
        /// key for the `(zone_id, object_type_reference)` a `ColdRow` is filed under.
        fn cold_key(zone_id: u32, type_reference: u32) -> u64 {
            ((zone_id as u64) << 32) | (type_reference as u64)
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

        /// Seed one **cold** row for a zone — insert-only-if-absent, so a worldgen
        /// re-run never clobbers a cold row later mutated in-world (same discipline as
        /// `seed_entity`). Sets `version = 1`. The edge calls this once per `ColdRow` of
        /// a fresh zone (`Worldgen::zone_cold_objects`).
        #[reducer]
        pub fn seed_cold_row(
            ctx: &ReducerContext,
            zone_id: u32,
            type_reference: u32,
            kinds: Vec<u32>,
        ) -> Result<(), String> {
            let key = cold_key(zone_id, type_reference);
            if ctx.db.cold().cold_key().find(key).is_none() {
                ctx.db.cold().insert(Cold { cold_key: key, zone_id, type_reference, kinds, version: 1 });
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

        /// Create a new entity with a freshly-minted globally-unique `entity_reference`,
        /// of `entity_type` (the reducer's first arg, kept named `obj_type` until the
        /// reducer-rename batch) and payload. The caller learns the key by observing the
        /// new `state` row (reducers can't return it).
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn spawn_object(
            ctx: &ReducerContext,
            obj_type: u8,
            $( $pf: $pt, )*
        ) -> Result<(), String> {
            let entity_key = mint_entity(ctx, obj_type);
            seed_write(ctx, entity_key, $( $pf, )*);
            Ok(())
        }

        /// Append validated intent. The edge/npc call this with `requesting`/`trigger`
        /// fields `0` (externally injected); a worker emitting a saga follow-on sets them
        /// so the consumer can validate provenance. The shard stamps `event_tic` so tic
        /// logic stays server-side. `worker_server_reference` starts `0`; `resolve` stamps
        /// it. AoE = one call per target.
        #[reducer]
        #[allow(clippy::too_many_arguments)]
        pub fn append_event(
            ctx: &ReducerContext,
            source_server_reference: u16,
            actor_server_reference: u16,
            requesting_server_reference: u16,
            trigger_server_reference: u16,
            trigger_event_reference: u64,
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
                source_server_reference,
                actor_server_reference,
                requesting_server_reference,
                worker_server_reference: $crate::SERVER_NONE,
                trigger_server_reference,
                trigger_event_reference,
                actor_key,
                target_key,
                action,
                data0,
                data1,
                status: $crate::STATUS_ACTIVE,
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
            let mut counts: ::std::collections::HashMap<u64, u16> =
                ::std::collections::HashMap::new();
            for e in ctx.db.event_log().event_tic().filter(to_tic) {
                if e.status == $crate::STATUS_ACTIVE {
                    *counts.entry(e.target_key).or_insert(0) += 1;
                }
            }
            let now = $crate::now_secs(ctx);
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
                    $( $pf: base.as_ref().map_or(Default::default(), |b| b.$pf.clone()), )*
                    server_id: $crate::SERVER_NONE,
                    created_at: now,
                    assigned_at: 0,
                    status: $crate::STATUS_ACTIVE,
                });
            }
            Ok(())
        }

        /// A worker claims a pending `(entity_key, tic)`. Succeeds if unclaimed or the
        /// lease has expired; otherwise a no-op. The claim is the fence token `resolve`
        /// checks.
        #[reducer]
        pub fn claim(ctx: &ReducerContext, server_id: u16, entity_key: u64, tic: u32) -> Result<(), String> {
            let Some(row) = find_state_log(ctx, entity_key, tic) else {
                return Ok(());
            };
            if row.dirty == 0 {
                return Ok(()); // already resolved
            }
            let now = $crate::now_secs(ctx);
            let free = row.server_id == $crate::SERVER_NONE
                || now.saturating_sub(row.assigned_at) > $crate::CLAIM_LEASE_SECS;
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
        /// marks the tic's events complete, and promotes to `state`.
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
                server_id: $crate::SERVER_NONE,
                ..row
            });
            // Mark this tic's events for this target complete.
            let done: Vec<u64> = ctx
                .db
                .event_log()
                .target_key()
                .filter(entity_key)
                .filter(|e| e.event_tic == tic && e.status == $crate::STATUS_ACTIVE)
                .map(|e| e.event_reference)
                .collect();
            for r in done {
                if let Some(mut e) = ctx.db.event_log().event_reference().find(r) {
                    ctx.db.event_log().event_reference().delete(r);
                    e.status = $crate::STATUS_COMPLETE;
                    // Record the fence winner so a saga follow-on triggered by this event
                    // can validate its provenance against it.
                    e.worker_server_reference = server_id;
                    ctx.db.event_log().insert(e);
                }
            }
            promote(ctx, &resolved);
            Ok(())
        }

        /// Prune the working set. Called periodically by `server_master`. Two sweeps:
        /// hard-delete completed events; and keep every pending row plus, per entity, its
        /// latest resolved row and resolved rows at `tic ≥ min_pending − 1`, dropping the
        /// accumulated deep history.
        #[reducer]
        pub fn tick_gc(ctx: &ReducerContext) -> Result<(), String> {
            // 1. Completed events.
            let done: Vec<u64> = ctx
                .db
                .event_log()
                .iter()
                .filter(|e| e.status == $crate::STATUS_COMPLETE)
                .map(|e| e.event_reference)
                .collect();
            for r in done {
                ctx.db.event_log().event_reference().delete(r);
            }

            // 2. Horizon = (lowest pending tic) − 1; if nothing is pending, only each
            //    entity's latest resolved row is needed.
            let pending_min = ctx
                .db
                .state_log()
                .iter()
                .filter(|r| r.dirty > 0)
                .map(|r| r.tic)
                .min();
            let horizon = pending_min.map_or(u32::MAX, |g| g.saturating_sub(1));

            let mut latest: ::std::collections::HashMap<u64, u32> =
                ::std::collections::HashMap::new();
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
