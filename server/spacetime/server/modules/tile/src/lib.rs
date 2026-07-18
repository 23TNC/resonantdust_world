//! tile — the **cold** shard for a zone's ground tiles (`TYPE_BIOME_TILE`).
//!
//! One row per `(zone, biome, layer)`: the dense 16×16 = 256 `kind_reference`s, one per
//! `tile_reference` (0..256), held as `entity_state.items` (`Vec<DenseItem>`). A tile carries only
//! **what kind** it is (`DenseItem.kind_reference`) — no per-entry `tile_reference` (that's the
//! index), which is what lets a dense cell be cheaper than a thing. A zone spanning several biomes is
//! several rows (one per `subtype`); cells outside a row's biome are `0`.
//!
//! Storage is the cold macros ([`resonantdust_codec::dense_entity_tables!`] +
//! [`resonantdust_codec::overlay_tables!`]): the **baseline** is `entity_state`/`entity_state_log`
//! (dense, `cold_row_reference`-addressed, owns the shard `clock`); the **override** tier is
//! `overlay`/`overlay_log` (sparse — occupied cells only). The client composites `entity_state ⊕
//! overlay` (an overlay cell shadows the baseline). `seed` fills the baseline; `set_tile` writes an
//! overlay cell; `fold` (`PACK`) folds the overlay back into the baseline. These are **direct**
//! reducers for now — P4 replaces them with `init_zone` / `SET` / `PACK` events driven through the
//! worker + `PROMOTE` (see `docs/work/shard-tables/`).
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference` = `macro_position:16 | subtype_id:12 |
//! layer_id:4`), `docs/TABLES.md`. **`type_id` is this module** (`TYPE_BIOME_TILE`), so it's off the
//! row — reconstruct with `type_reference = TYPE_BIOME_TILE | subtype_id`.

use spacetimedb::{reducer, ReducerContext, Table};

use resonantdust_codec::object::pack_cold_row_reference;

// Cold storage: the dense baseline (`entity_state`, owns the shard `clock`) + the sparse overlay
// (`overlay`). Both `cold_row_reference`-addressed; the client reads `entity_state ⊕ overlay`.
resonantdust_codec::dense_entity_tables!();
resonantdust_codec::overlay_tables!();

/// A zone is 16×16 tiles.
const TILES_PER_ZONE: usize = 256;

/// The cold shard's current tic (its `clock` mirror, bumped by the master).
fn now_tic(ctx: &ReducerContext) -> u16 {
    ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0)
}

/// Seed (or overwrite) a zone-layer-biome's ground into the **baseline** `entity_state`. This is
/// trusted server-side **generation** (worldgen), not a player mutation — player edits go through the
/// overlay. `type_id` is the module. Idempotent: re-seeding a `(zone, subtype, layer)` overwrites
/// deterministically.
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tiles: Vec<u16>,
) -> Result<(), String> {
    if tiles.len() != TILES_PER_ZONE {
        return Err(format!("a zone has {TILES_PER_ZONE} tiles, got {}", tiles.len()));
    }
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    let items: Vec<DenseItem> = tiles.into_iter().map(|kind_reference| DenseItem { kind_reference }).collect();
    let row = EntityState {
        cold_row_reference,
        macro_position_reference: macro_position,
        subtype_id,
        layer_id,
        tic: now_tic(ctx),
        items,
    };
    if ctx.db.entity_state().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.entity_state().cold_row_reference().update(row);
    } else {
        ctx.db.entity_state().insert(row);
    }
    Ok(())
}

/// **Mutate a ground cell** to `kind_reference` via the **overlay** — a sparse per-cell override that
/// never touches the baseline `entity_state`. The client composites the overlay cell over the
/// baseline. Idempotent by `(cold_row, tile_reference)`: a re-issue rewrites the same cell.
///
/// This is the direct write primitive that proves the composite path. The **event-driven** `SET`
/// routing (edge → orchestrator → worker) and the `PACK` fold back into the baseline are the
/// remaining P4 steps — see `docs/work/shard-tables/`.
#[reducer]
pub fn set_tile(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_reference: u8,
    kind_reference: u16,
) -> Result<(), String> {
    // A mutation keeps the cell's biome — find the baseline row that actually holds this cell (its
    // non-empty `subtype`), so the eventual `fold` writes back to the *right* row. `subtype_id` is a
    // fallback when no baseline row claims the cell.
    let subtype_id = ctx
        .db
        .entity_state()
        .macro_position_reference()
        .filter(macro_position)
        .find(|r| {
            r.layer_id == layer_id
                && r.items.get(tile_reference as usize).map(|it| it.kind_reference).unwrap_or(0) != 0
        })
        .map(|r| r.subtype_id)
        .unwrap_or(subtype_id);
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    // Merge the cell into the row's existing overlay (replace if already overridden, else append).
    let mut items = ctx
        .db
        .overlay()
        .cold_row_reference()
        .find(cold_row_reference)
        .map(|o| o.items)
        .unwrap_or_default();
    match items.iter_mut().find(|it| it.tile_reference == tile_reference) {
        Some(it) => it.kind_reference = kind_reference,
        None => items.push(OverlayItem { tile_reference, kind_reference }),
    }
    let row = Overlay {
        cold_row_reference,
        macro_position_reference: macro_position,
        subtype_id,
        layer_id,
        tic: now_tic(ctx),
        items,
    };
    if ctx.db.overlay().cold_row_reference().find(cold_row_reference).is_some() {
        ctx.db.overlay().cold_row_reference().update(row);
    } else {
        ctx.db.overlay().insert(row);
    }
    Ok(())
}

/// **GC fold (`PACK`)** — fold every settled overlay cell back into the compressed baseline
/// `entity_state`, then drop the overlay row. The client sees no change (it read the value from the
/// overlay; now it reads the same value from the baseline). This is the write-back half of the
/// mutation lifecycle — `set_tile` puts a cell hot, this returns it to cold. The master's GC calls it
/// on a cadence.
///
/// The overlay shares the baseline's `cold_row_reference`, so each overlay row folds straight into its
/// baseline sibling (no def-reconstruction). Tombstone-not-drop (for a slow reader / takeover) is a
/// refinement — this drops it.
#[reducer]
pub fn fold(ctx: &ReducerContext) -> Result<(), String> {
    // The baseline is current as of *now* once folded — `now_tic` is ≥ every settled override's tic,
    // so the client reads the folded value from the baseline, not the (stale) override.
    let ftic = now_tic(ctx);
    let overlays: Vec<Overlay> = ctx.db.overlay().iter().collect();
    for o in overlays {
        if let Some(mut base) = ctx.db.entity_state().cold_row_reference().find(o.cold_row_reference) {
            for it in &o.items {
                if let Some(slot) = base.items.get_mut(it.tile_reference as usize) {
                    slot.kind_reference = it.kind_reference;
                }
            }
            base.tic = ftic;
            ctx.db.entity_state().cold_row_reference().update(base);
        }
        // The value now lives in the baseline the client already reads — drop the overlay.
        ctx.db.overlay().cold_row_reference().delete(o.cold_row_reference);
    }
    Ok(())
}
