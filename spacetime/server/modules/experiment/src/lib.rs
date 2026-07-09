// lib.rs — experiment module.
//
// A deliberately-dumb full-stack sync probe that runs PARALLEL to the real
// bitemporal event-log sync (see docs/sync.md). It owns exactly one public
// table, `experiment_objects`, holding a flat `(object_id, x, y)` position per
// object. No `valid_at`, no history, no projection: the server overwrites the
// row, the client subscribes, pixijs tweens between whatever positions it
// receives. The point is to exercise every hop (module → server → wasm client →
// pixijs → npc) end-to-end, not to model motion correctly.
//
// `x`/`y` are GLOBAL tile coordinates. The five seeded objects live in zone
// (0,0), i.e. tiles 0..16 on each axis (ZONE_DIM = 16 in shared/codec).

use spacetimedb::{reducer, table, ReducerContext, Table};

/// One experiment object's current position. Public so browser clients can
/// subscribe with `SELECT * FROM experiment_objects`.
#[table(accessor = experiment_objects, public)]
#[derive(Debug, Clone)]
pub struct ExperimentObject {
    /// Stable identity, `0..5`. Primary key — a reposition overwrites the row
    /// with the same `object_id`.
    #[primary_key]
    pub object_id: u32,
    /// Global tile column.
    pub x: u32,
    /// Global tile row.
    pub y: u32,
}

/// The five starting tiles, scattered inside zone (0,0) (tiles 0..16).
const SEED_TILES: [(u32, u32); 5] = [(2, 2), (5, 3), (8, 8), (11, 5), (13, 12)];

/// Module init — runs once on fresh publish. Seeds the five objects at their
/// starting tiles. Idempotent against a stray double-publish: skips any
/// `object_id` that already exists.
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    for (object_id, (x, y)) in SEED_TILES.iter().enumerate() {
        let object_id = object_id as u32;
        if ctx
            .db
            .experiment_objects()
            .object_id()
            .find(object_id)
            .is_none()
        {
            ctx.db.experiment_objects().insert(ExperimentObject {
                object_id,
                x: *x,
                y: *y,
            });
        }
    }
}

/// Reposition one experiment object. The server calls this on behalf of a
/// client's `UpdateExperiment` command; the row update streams back to every
/// subscriber. Errors if `object_id` was never seeded.
#[reducer]
pub fn set_experiment_position(
    ctx: &ReducerContext,
    object_id: u32,
    x: u32,
    y: u32,
) -> Result<(), String> {
    if ctx
        .db
        .experiment_objects()
        .object_id()
        .find(object_id)
        .is_none()
    {
        return Err(format!(
            "experiment: set position of unknown object {object_id}"
        ));
    }
    // Overwrite the row: delete-by-PK then re-insert. Matches the object_shard
    // idiom (no `.update()` helper is used elsewhere in this codebase).
    ctx.db.experiment_objects().object_id().delete(object_id);
    ctx.db.experiment_objects().insert(ExperimentObject { object_id, x, y });
    Ok(())
}
