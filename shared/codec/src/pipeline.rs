//! `entity_tables!` — the shared tic-composition machinery every composing shard stamps out.
//!
//! A shard that composes per tic (`data_shard`, and the cold `tile`/`thing` overlays) needs the
//! *same* `clock` / `state_log` / `state` tables and `init` / `bump` / `claim` / `write` / `gc`
//! reducers. That machinery used to be hand-copied per module (a drift hazard on string-typed
//! subscription SQL); this macro is the single source of truth. Each module invokes `entity_tables!()`
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
/// module docs.
///
/// **Payload is a parameter.** The fixed columns are `entity_reference`, `definition_reference`,
/// `macro_position_reference` + `micro_position_reference` (the split `position_reference`), `tic`; the
/// caller lists only the shard's **extra** payload fields, spliced in after them. E.g. the hot mover /
/// current overlay: `resonantdust_codec::entity_tables!(data: u8);` (a tile shard would pass none).
/// The worker splits `position_reference` → `macro`/`micro` when it builds a `TargetState`, and the
/// edge reassembles `position_reference` = `macro | micro` for the wire — so the client is untouched.
/// (First generalization step toward the `*_tables!` family — see `work/shard-tables`.)
#[macro_export]
macro_rules! entity_tables {
    ($($pf:ident : $pt:ty),* $(,)?) => {
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
            /// Position, split: `macro` (the zone-subscription key) + `micro` (the cell within it).
            pub macro_position_reference: u16,
            pub micro_position_reference: u16,
            $(pub $pf: $pt,)*
            /// If set, promote this target to `state` once settled (the program ran `PROMOTE_STATE`).
            pub promote: bool,
        }

        // ── state_log — per (entity, tic) composition slot ─────────────────────────────
        #[spacetimedb::table(accessor = entity_state_log, public)]
        pub struct EntityStateLog {
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
            pub macro_position_reference: u16,
            pub micro_position_reference: u16,
            $(pub $pf: $pt,)*
            /// `state_status` — `flags:4 | status:4` (`PROMOTE` / `PROMOTED`).
            pub status: u8,
        }

        // ── state — client-visible latest ──────────────────────────────────────────────
        #[spacetimedb::table(accessor = entity_state, public)]
        pub struct EntityState {
            #[primary_key]
            pub entity_reference: u32,
            /// The zone-subscription key — a subscription filters on columns, not expressions.
            #[index(btree)]
            pub macro_position_reference: u16,
            /// The cell within the zone (`tile_reference:8 | layer_reference:8`); `macro | micro` is the
            /// full `position_reference` the edge reassembles for the wire.
            pub micro_position_reference: u16,
            pub tic: u16,
            pub definition_reference: u32,
            $(pub $pf: $pt,)*
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
                if let Some(prev) = entity_tables_previous_row(ctx, e, tic) {
                    let mut p = prev;
                    p.observer_reference = worker;
                    ctx.db.entity_state_log().uid().update(p);
                }
                match ctx.db.entity_state_log().uid().find(uid) {
                    Some(mut row) => {
                        row.worker_reference = worker;
                        row.dirty = true;
                        ctx.db.entity_state_log().uid().update(row);
                    }
                    None => {
                        ctx.db.entity_state_log().insert(EntityStateLog {
                            uid,
                            entity_reference: e,
                            tic,
                            worker_reference: worker,
                            observer_reference: $crate::refs::SERVER_REF_NONE,
                            dirty: true,
                            definition_reference: 0,
                            macro_position_reference: 0,
                            micro_position_reference: 0,
                            $($pf: Default::default(),)*
                            status: $crate::status::pack_status(0, $crate::status::STATE_OPEN),
                        });
                    }
                }
            }
            Ok(())
        }

        /// The serially-latest existing `state_log` row for `entity` strictly before `tic`, if any.
        fn entity_tables_previous_row(
            ctx: &spacetimedb::ReducerContext,
            entity: u32,
            tic: u16,
        ) -> Option<EntityStateLog> {
            ctx.db
                .entity_state_log()
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
                    .entity_state_log()
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
                row.macro_position_reference = r.macro_position_reference;
                row.micro_position_reference = r.micro_position_reference;
                $(row.$pf = r.$pf.clone();)*
                row.dirty = false;
                if r.promote {
                    row.status =
                        $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_OPEN);
                }
                let status = row.status;
                ctx.db.entity_state_log().uid().update(row);

                if $crate::status::status_has_flag(status, $crate::status::STATE_FLAG_PROMOTE)
                    && $crate::status::status_phase(status) != $crate::status::STATE_PROMOTED
                {
                    entity_tables_upsert_state(ctx, &r, tic);
                    let mut promoted = ctx.db.entity_state_log().uid().find(uid).expect("just wrote");
                    promoted.status =
                        $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_PROMOTED);
                    ctx.db.entity_state_log().uid().update(promoted);
                }
            }
            Ok(())
        }

        fn entity_tables_upsert_state(ctx: &spacetimedb::ReducerContext, r: &TargetState, tic: u16) {
            let row = EntityState {
                entity_reference: r.entity_reference,
                macro_position_reference: r.macro_position_reference,
                micro_position_reference: r.micro_position_reference,
                tic,
                definition_reference: r.definition_reference,
                $($pf: r.$pf.clone(),)*
            };
            if ctx.db.entity_state().entity_reference().find(r.entity_reference).is_some() {
                ctx.db.entity_state().entity_reference().update(row);
            } else {
                ctx.db.entity_state().insert(row);
            }
        }

        // ── gc — the master drops old settled rows (never the latest per entity) ────────
        /// Delete `!dirty` rows that are **not** the latest for their entity and older than `horizon`.
        /// The latest per entity is never reaped — it is every future tic's base.
        #[spacetimedb::reducer]
        pub fn gc(ctx: &spacetimedb::ReducerContext, horizon: u16) -> Result<(), String> {
            let mut latest: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
            for row in ctx.db.entity_state_log().iter() {
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
                .entity_state_log()
                .iter()
                .filter(|r| {
                    !r.dirty
                        && latest.get(&r.entity_reference) != Some(&r.tic)
                        && $crate::tic::tic_before(r.tic, horizon)
                })
                .map(|r| r.uid)
                .collect();
            for uid in doomed {
                ctx.db.entity_state_log().uid().delete(uid);
            }
            Ok(())
        }
    };
}

/// The cold **dense** baseline pair — `entity_state`/`entity_state_log`, `cold_row_reference`-addressed,
/// payload a fully-allocated `Vec<DenseItem>` (`ZONE_DIM²`, index = `tile_reference`). Owns the shard's
/// `clock`; pair it with a clock-less [`overlay_tables!`]. Parallel sibling of `entity_tables!` (F7 —
/// the `#[table]` accessor can't be a macro param, so no shared inner macro; the tables + reducers are
/// inlined). `<T>` is the per-item extra payload (none for a plain tile). Invoke: `dense_entity_tables!();`.
#[macro_export]
macro_rules! dense_entity_tables {
    ($($pf:ident : $pt:ty),* $(,)?) => {
        $crate::__cold_clock!();
        /// One dense cell — `kind_reference` (what it is) + the optional extra payload.
        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct DenseItem {
            pub kind_reference: u16,
            $(pub $pf: $pt,)*
        }
        $crate::__cold_baseline_tables!(DenseItem);
    };
}

/// The cold **sparse** baseline pair — like [`dense_entity_tables!`] but `Vec<SparseItem>` (occupied
/// cells only; each `SparseItem` carries its own `tile_reference`). Owns the shard's `clock`.
#[macro_export]
macro_rules! sparse_entity_tables {
    ($($pf:ident : $pt:ty),* $(,)?) => {
        $crate::__cold_clock!();
        /// One occupied cell — carries its own `tile_reference` + `kind_reference` + extra payload.
        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct DenseItem {
            pub tile_reference: u8,
            pub kind_reference: u16,
            $(pub $pf: $pt,)*
        }
        $crate::__cold_baseline_tables!(DenseItem);
    };
}

/// The cold **overlay** pair — `overlay`/`overlay_log`, the per-cell override tier (always sparse).
/// **Clock-less** — the baseline macro owns the shard's `clock` — and its reducers are `*_overlay` so
/// they don't collide with the baseline's `claim`/`write`/`gc` (F7 coexistence constraint).
#[macro_export]
macro_rules! overlay_tables {
    ($($pf:ident : $pt:ty),* $(,)?) => {
        /// One override cell — `tile_reference` + `kind_reference` + extra payload.
        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct OverlayItem {
            pub tile_reference: u8,
            pub kind_reference: u16,
            $(pub $pf: $pt,)*
        }

        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct OverlayTarget {
            pub cold_row_reference: u32,
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub items: Vec<OverlayItem>,
            pub promote: bool,
        }

        #[spacetimedb::table(accessor = overlay_log, public)]
        pub struct OverlayLog {
            #[primary_key]
            pub uid: u64,
            #[index(btree)]
            pub cold_row_reference: u32,
            #[index(btree)]
            pub tic: u16,
            #[index(btree)]
            pub worker_reference: u8,
            #[index(btree)]
            pub observer_reference: u8,
            pub dirty: bool,
            #[index(btree)]
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub items: Vec<OverlayItem>,
            pub status: u8,
        }

        #[spacetimedb::table(accessor = overlay, public)]
        pub struct Overlay {
            #[primary_key]
            pub cold_row_reference: u32,
            #[index(btree)]
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub tic: u16,
            pub items: Vec<OverlayItem>,
        }

        #[spacetimedb::reducer]
        pub fn claim_overlay(ctx: &spacetimedb::ReducerContext, rows: Vec<u32>, tic: u16, worker: u8) -> Result<(), String> {
            for e in rows {
                let uid = $crate::uid::pack_state_uid(e, tic);
                if let Some(mut prev) = ctx.db.overlay_log().cold_row_reference().filter(e)
                    .filter(|r| $crate::tic::tic_before(r.tic, tic))
                    .reduce(|a, b| if $crate::tic::tic_after(b.tic, a.tic) { b } else { a }) {
                    prev.observer_reference = worker;
                    ctx.db.overlay_log().uid().update(prev);
                }
                match ctx.db.overlay_log().uid().find(uid) {
                    Some(mut row) => { row.worker_reference = worker; row.dirty = true; ctx.db.overlay_log().uid().update(row); }
                    None => { ctx.db.overlay_log().insert(OverlayLog {
                        uid, cold_row_reference: e, tic, worker_reference: worker,
                        observer_reference: $crate::refs::SERVER_REF_NONE, dirty: true,
                        macro_position_reference: 0, subtype_id: 0, layer_id: 0, items: Vec::new(),
                        status: $crate::status::pack_status(0, $crate::status::STATE_OPEN),
                    }); }
                }
            }
            Ok(())
        }

        #[spacetimedb::reducer]
        pub fn write_overlay(ctx: &spacetimedb::ReducerContext, worker: u8, tic: u16, results: Vec<OverlayTarget>) -> Result<(), String> {
            for r in results {
                let uid = $crate::uid::pack_state_uid(r.cold_row_reference, tic);
                let mut row = ctx.db.overlay_log().uid().find(uid)
                    .ok_or_else(|| format!("no overlay slot for row {:#010x} tic {tic}", r.cold_row_reference))?;
                if row.worker_reference != worker { return Err(format!("worker {worker} not assigned overlay slot {uid:#018x}")); }
                if !row.dirty { continue; }
                row.macro_position_reference = r.macro_position_reference;
                row.subtype_id = r.subtype_id;
                row.layer_id = r.layer_id;
                row.items = r.items.clone();
                row.dirty = false;
                if r.promote { row.status = $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_OPEN); }
                let status = row.status;
                ctx.db.overlay_log().uid().update(row);
                if $crate::status::status_has_flag(status, $crate::status::STATE_FLAG_PROMOTE)
                    && $crate::status::status_phase(status) != $crate::status::STATE_PROMOTED {
                    let vis = Overlay {
                        cold_row_reference: r.cold_row_reference,
                        macro_position_reference: r.macro_position_reference,
                        subtype_id: r.subtype_id, layer_id: r.layer_id, tic, items: r.items.clone(),
                    };
                    if ctx.db.overlay().cold_row_reference().find(r.cold_row_reference).is_some() {
                        ctx.db.overlay().cold_row_reference().update(vis);
                    } else { ctx.db.overlay().insert(vis); }
                    let mut promoted = ctx.db.overlay_log().uid().find(uid).expect("just wrote");
                    promoted.status = $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_PROMOTED);
                    ctx.db.overlay_log().uid().update(promoted);
                }
            }
            Ok(())
        }

        #[spacetimedb::reducer]
        pub fn gc_overlay(ctx: &spacetimedb::ReducerContext, horizon: u16) -> Result<(), String> {
            let mut latest: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
            for row in ctx.db.overlay_log().iter() {
                latest.entry(row.cold_row_reference)
                    .and_modify(|t| { if $crate::tic::tic_after(row.tic, *t) { *t = row.tic; } })
                    .or_insert(row.tic);
            }
            let doomed: Vec<u64> = ctx.db.overlay_log().iter()
                .filter(|r| !r.dirty && latest.get(&r.cold_row_reference) != Some(&r.tic) && $crate::tic::tic_before(r.tic, horizon))
                .map(|r| r.uid).collect();
            for uid in doomed { ctx.db.overlay_log().uid().delete(uid); }
            Ok(())
        }
    };
}

/// The shard's tic `clock` mirror + `bump`/`init` (the master bumps it). Emitted once per shard, by the
/// baseline macro (a shard has exactly one clock). Literal accessor — safe to share (F7).
#[macro_export]
macro_rules! __cold_clock {
    () => {
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
    };
}

/// The **baseline** cold pair (`entity_state`/`entity_state_log`) + its `claim`/`write`/`gc`, over an
/// `items: Vec<$item>`. Literal accessors (F7). The dense/sparse macros differ only in `$item`, so this
/// carries everything after the item struct. `cold_row_reference`-addressed; `uid` = `cold_uid`.
#[macro_export]
macro_rules! __cold_baseline_tables {
    ($item:ident) => {
        #[derive(spacetimedb::SpacetimeType, Clone)]
        pub struct TargetState {
            pub cold_row_reference: u32,
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub items: Vec<$item>,
            pub promote: bool,
        }

        #[spacetimedb::table(accessor = entity_state_log, public)]
        pub struct EntityStateLog {
            #[primary_key]
            pub uid: u64,
            #[index(btree)]
            pub cold_row_reference: u32,
            #[index(btree)]
            pub tic: u16,
            #[index(btree)]
            pub worker_reference: u8,
            #[index(btree)]
            pub observer_reference: u8,
            pub dirty: bool,
            #[index(btree)]
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub items: Vec<$item>,
            pub status: u8,
        }

        #[spacetimedb::table(accessor = entity_state, public)]
        pub struct EntityState {
            #[primary_key]
            pub cold_row_reference: u32,
            #[index(btree)]
            pub macro_position_reference: u16,
            pub subtype_id: u16,
            pub layer_id: u8,
            pub tic: u16,
            pub items: Vec<$item>,
        }

        #[spacetimedb::reducer]
        pub fn claim(ctx: &spacetimedb::ReducerContext, rows: Vec<u32>, tic: u16, worker: u8) -> Result<(), String> {
            for e in rows {
                let uid = $crate::uid::pack_state_uid(e, tic);
                if let Some(mut prev) = ctx.db.entity_state_log().cold_row_reference().filter(e)
                    .filter(|r| $crate::tic::tic_before(r.tic, tic))
                    .reduce(|a, b| if $crate::tic::tic_after(b.tic, a.tic) { b } else { a }) {
                    prev.observer_reference = worker;
                    ctx.db.entity_state_log().uid().update(prev);
                }
                match ctx.db.entity_state_log().uid().find(uid) {
                    Some(mut row) => { row.worker_reference = worker; row.dirty = true; ctx.db.entity_state_log().uid().update(row); }
                    None => { ctx.db.entity_state_log().insert(EntityStateLog {
                        uid, cold_row_reference: e, tic, worker_reference: worker,
                        observer_reference: $crate::refs::SERVER_REF_NONE, dirty: true,
                        macro_position_reference: 0, subtype_id: 0, layer_id: 0, items: Vec::new(),
                        status: $crate::status::pack_status(0, $crate::status::STATE_OPEN),
                    }); }
                }
            }
            Ok(())
        }

        #[spacetimedb::reducer]
        pub fn write(ctx: &spacetimedb::ReducerContext, worker: u8, tic: u16, results: Vec<TargetState>) -> Result<(), String> {
            for r in results {
                let uid = $crate::uid::pack_state_uid(r.cold_row_reference, tic);
                let mut row = ctx.db.entity_state_log().uid().find(uid)
                    .ok_or_else(|| format!("no slot for row {:#010x} tic {tic}", r.cold_row_reference))?;
                if row.worker_reference != worker { return Err(format!("worker {worker} not assigned slot {uid:#018x}")); }
                if !row.dirty { continue; }
                row.macro_position_reference = r.macro_position_reference;
                row.subtype_id = r.subtype_id;
                row.layer_id = r.layer_id;
                row.items = r.items.clone();
                row.dirty = false;
                if r.promote { row.status = $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_OPEN); }
                let status = row.status;
                ctx.db.entity_state_log().uid().update(row);
                if $crate::status::status_has_flag(status, $crate::status::STATE_FLAG_PROMOTE)
                    && $crate::status::status_phase(status) != $crate::status::STATE_PROMOTED {
                    let vis = EntityState {
                        cold_row_reference: r.cold_row_reference,
                        macro_position_reference: r.macro_position_reference,
                        subtype_id: r.subtype_id, layer_id: r.layer_id, tic, items: r.items.clone(),
                    };
                    if ctx.db.entity_state().cold_row_reference().find(r.cold_row_reference).is_some() {
                        ctx.db.entity_state().cold_row_reference().update(vis);
                    } else { ctx.db.entity_state().insert(vis); }
                    let mut promoted = ctx.db.entity_state_log().uid().find(uid).expect("just wrote");
                    promoted.status = $crate::status::pack_status($crate::status::STATE_FLAG_PROMOTE, $crate::status::STATE_PROMOTED);
                    ctx.db.entity_state_log().uid().update(promoted);
                }
            }
            Ok(())
        }

        #[spacetimedb::reducer]
        pub fn gc(ctx: &spacetimedb::ReducerContext, horizon: u16) -> Result<(), String> {
            let mut latest: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
            for row in ctx.db.entity_state_log().iter() {
                latest.entry(row.cold_row_reference)
                    .and_modify(|t| { if $crate::tic::tic_after(row.tic, *t) { *t = row.tic; } })
                    .or_insert(row.tic);
            }
            let doomed: Vec<u64> = ctx.db.entity_state_log().iter()
                .filter(|r| !r.dirty && latest.get(&r.cold_row_reference) != Some(&r.tic) && $crate::tic::tic_before(r.tic, horizon))
                .map(|r| r.uid).collect();
            for uid in doomed { ctx.db.entity_state_log().uid().delete(uid); }
            Ok(())
        }
    };
}
