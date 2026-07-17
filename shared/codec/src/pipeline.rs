//! `tick_pipeline!` — the shared tic-composition machinery every composing shard stamps out.
//!
//! A shard that composes per tic (`data_shard`, and the cold `tile`/`thing` overlays) needs the
//! *same* `clock` / `state_log` / `state` tables and `init` / `bump` / `claim` / `write` / `gc`
//! reducers. That machinery used to be hand-copied per module (a drift hazard on string-typed
//! subscription SQL); this macro is the single source of truth. Each module invokes `tick_pipeline!()`
//! once and adds only its own extras (a cold module adds its baseline tables + mint + fold).
//!
//! The payload is the reference model's three orthogonal references (`definition_reference |
//! position_reference | data`) — identical for hot and cold, so it's fixed here, not a parameter.
//!
//! **Invoking module must** be a SpacetimeDB module with `spacetimedb` + `resonantdust-codec` deps and
//! `use spacetimedb::Table;` in scope (for `.insert` / `.iter` / `.count`). Everything else the macro
//! qualifies (`spacetimedb::` for the module API, `$crate::` for codec helpers). The emitted tables +
//! reducers are byte-identical to the shape in `docs/TABLES.md § data_shard`.

/// Stamp out the tic-composition tables (`clock`, `state_log`, `state`), the `TargetState` payload,
/// and the `init` / `bump` / `claim` / `write` / `gc` reducers into the invoking module. See the
/// module docs. Invoke once, unqualified: `resonantdust_codec::tick_pipeline!();`.
#[macro_export]
macro_rules! tick_pipeline {
    () => {
        // ── the tic clock (the master bumps it in lockstep across every shard) ──────────
        #[spacetimedb::table(accessor = clock, public)]
        pub struct Clock {
            #[primary_key]
            pub id: u8,
            pub master_tic: u16,
        }

        #[spacetimedb::reducer]
        pub fn bump(ctx: &spacetimedb::ReducerContext, master_tic: u16) -> Result<(), String> {
            ctx.db.clock().id().delete(0);
            ctx.db.clock().insert(Clock { id: 0, master_tic });
            Ok(())
        }

        #[spacetimedb::reducer(init)]
        pub fn init(ctx: &spacetimedb::ReducerContext) {
            if ctx.db.clock().id().find(0).is_none() {
                ctx.db.clock().insert(Clock { id: 0, master_tic: 0 });
            }
        }

        // ── the payload — the reference model's three orthogonal references ─────────────
        /// One target's composed value, passed to `write`. `entity_reference` selects the slot.
        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct TargetState {
            pub entity_reference: u32,
            pub definition_reference: u32,
            pub position_reference: u32,
            pub data: u8,
            /// If set, promote this target to `state` once settled (the program ran `PROMOTE_STATE`).
            pub promote: bool,
        }

        // ── state_log — per (entity, tic) composition slot ─────────────────────────────
        #[spacetimedb::table(accessor = state_log, public)]
        pub struct StateLog {
            /// `state_uid` = `reserved:16 | entity_reference:32 | tic:16`. Entity-major.
            #[primary_key]
            pub uid: u64,
            #[index(btree)]
            pub entity_reference: u32,
            #[index(btree)]
            pub tic: u16,
            /// The worker that writes this row (its component's owner). `SERVER_REF_NONE` = none.
            #[index(btree)]
            pub worker_reference: u8,
            /// The worker that reads this row as the base for its next-tic work. No lease — a
            /// re-`claim` overwrites it, which is the eviction.
            #[index(btree)]
            pub observer_reference: u8,
            /// `true` = work pending; `false` = settled (one worker per component → binary).
            pub dirty: bool,
            pub definition_reference: u32,
            pub position_reference: u32,
            pub data: u8,
            /// `state_status` — `flags:4 | status:4` (`PROMOTE` / `PROMOTED`).
            pub status: u8,
        }

        // ── state — client-visible latest ──────────────────────────────────────────────
        #[spacetimedb::table(accessor = state, public)]
        pub struct State {
            #[primary_key]
            pub entity_reference: u32,
            /// The zone-subscription key — the payload's `position_reference` high half, kept as its
            /// own column because a subscription filters on columns, not expressions.
            #[index(btree)]
            pub macro_position_reference: u16,
            pub tic: u16,
            pub definition_reference: u32,
            pub position_reference: u32,
            pub data: u8,
        }

        // ── claim — the orchestrator stands up a component's slots ──────────────────────
        /// For each entity: create its `(E, tic)` slot (`dirty`), stamp `worker_reference`; find E's
        /// most-recent row `< tic` and stamp `observer_reference` on it (the base the worker reads).
        /// No lease — a re-`claim` overwrites the stamp.
        #[spacetimedb::reducer]
        pub fn claim(
            ctx: &spacetimedb::ReducerContext,
            entities: Vec<u32>,
            tic: u16,
            worker: u8,
        ) -> Result<(), String> {
            for e in entities {
                let uid = $crate::uid::pack_state_uid(e, tic);
                if let Some(prev) = tick_pipeline_previous_row(ctx, e, tic) {
                    let mut p = prev;
                    p.observer_reference = worker;
                    ctx.db.state_log().uid().update(p);
                }
                match ctx.db.state_log().uid().find(uid) {
                    Some(mut row) => {
                        row.worker_reference = worker;
                        row.dirty = true;
                        ctx.db.state_log().uid().update(row);
                    }
                    None => {
                        ctx.db.state_log().insert(StateLog {
                            uid,
                            entity_reference: e,
                            tic,
                            worker_reference: worker,
                            observer_reference: $crate::refs::SERVER_REF_NONE,
                            dirty: true,
                            definition_reference: 0,
                            position_reference: 0,
                            data: 0,
                            status: $crate::status::pack_status(0, $crate::status::STATE_OPEN),
                        });
                    }
                }
            }
            Ok(())
        }

        /// The serially-latest existing `state_log` row for `entity` strictly before `tic`, if any.
        fn tick_pipeline_previous_row(
            ctx: &spacetimedb::ReducerContext,
            entity: u32,
            tic: u16,
        ) -> Option<StateLog> {
            ctx.db
                .state_log()
                .entity_reference()
                .filter(entity)
                .filter(|r| $crate::tic::tic_before(r.tic, tic))
                .reduce(|a, b| if $crate::tic::tic_after(b.tic, a.tic) { b } else { a })
        }

        // ── write — the worker commits absolute finals ─────────────────────────────────
        /// Write each target's **absolute** composed value for `tic`. Idempotent: a replay recomputes
        /// the same value from the immutable base, and an already-`!dirty` row is skipped. Only the
        /// assigned worker may write (the fence). A `promote` target lands in `state` once, on settle.
        #[spacetimedb::reducer]
        pub fn write(
            ctx: &spacetimedb::ReducerContext,
            worker: u8,
            tic: u16,
            results: Vec<TargetState>,
        ) -> Result<(), String> {
            for r in results {
                let uid = $crate::uid::pack_state_uid(r.entity_reference, tic);
                let mut row = ctx
                    .db
                    .state_log()
                    .uid()
                    .find(uid)
                    .ok_or_else(|| format!("no slot for entity {:#010x} tic {tic}", r.entity_reference))?;
                if row.worker_reference != worker {
                    return Err(format!("worker {worker} is not assigned slot {uid:#018x}"));
                }
                if !row.dirty {
                    continue; // already written — a replay
                }
                row.definition_reference = r.definition_reference;
                row.position_reference = r.position_reference;
                row.data = r.data;
                row.dirty = false;
                if r.promote {
                    row.status =
                        $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_OPEN);
                }
                let status = row.status;
                ctx.db.state_log().uid().update(row);

                if $crate::status::status_has_flag(status, $crate::status::STATE_FLAG_PROMOTE)
                    && $crate::status::status_phase(status) != $crate::status::STATE_PROMOTED
                {
                    tick_pipeline_upsert_state(ctx, &r, tic);
                    let mut promoted = ctx.db.state_log().uid().find(uid).expect("just wrote");
                    promoted.status =
                        $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_PROMOTED);
                    ctx.db.state_log().uid().update(promoted);
                }
            }
            Ok(())
        }

        fn tick_pipeline_upsert_state(ctx: &spacetimedb::ReducerContext, r: &TargetState, tic: u16) {
            let macro_position = $crate::object::position_macro(r.position_reference);
            let row = State {
                entity_reference: r.entity_reference,
                macro_position_reference: macro_position,
                tic,
                definition_reference: r.definition_reference,
                position_reference: r.position_reference,
                data: r.data,
            };
            if ctx.db.state().entity_reference().find(r.entity_reference).is_some() {
                ctx.db.state().entity_reference().update(row);
            } else {
                ctx.db.state().insert(row);
            }
        }

        // ── gc — the master drops old settled rows (never the latest per entity) ────────
        /// Delete `!dirty` rows that are **not** the latest for their entity and older than `horizon`.
        /// The latest per entity is never reaped — it is every future tic's base.
        #[spacetimedb::reducer]
        pub fn gc(ctx: &spacetimedb::ReducerContext, horizon: u16) -> Result<(), String> {
            let mut latest: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
            for row in ctx.db.state_log().iter() {
                latest
                    .entry(row.entity_reference)
                    .and_modify(|t| {
                        if $crate::tic::tic_after(row.tic, *t) {
                            *t = row.tic;
                        }
                    })
                    .or_insert(row.tic);
            }
            let doomed: Vec<u64> = ctx
                .db
                .state_log()
                .iter()
                .filter(|r| {
                    !r.dirty
                        && latest.get(&r.entity_reference) != Some(&r.tic)
                        && $crate::tic::tic_before(r.tic, horizon)
                })
                .map(|r| r.uid)
                .collect();
            for uid in doomed {
                ctx.db.state_log().uid().delete(uid);
            }
            Ok(())
        }
    };
}
