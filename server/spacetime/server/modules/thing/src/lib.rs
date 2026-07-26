//! thing — the **cold** shard for a zone's scattered things (trees, rocks, …) (`TYPE_BIOME_THING`).
//!
//! One row per `(zone, biome, layer)`: a **sparse** `items: Vec<DenseItem>` — occupied cells only,
//! each carrying its own `tile_reference` + `kind_reference` + `data` (`rotation:2 | count:6`). Unlike
//! a tile — only *what kind*, positioned by its dense index — a thing carries *where in the zone* it
//! sits and *its data*, so its cell is a full `DenseItem` and can't be a bare dense index. A zone
//! spanning several biomes is several rows (one per `subtype`).
//!
//! Storage is the cold macros ([`resonantdust_codec::sparse_entity_tables!`] +
//! [`resonantdust_codec::overlay_tables!`]): the **baseline** is `entity_state`/`entity_state_log`
//! (sparse, `cold_row_reference`-addressed, owns the shard `clock`); the **override** tier is
//! `overlay`/`overlay_log`. The client composites `entity_state ⊕ overlay`. `seed` fills the baseline;
//! `set_thing` writes an overlay cell (`kind_reference == 0` removes the cell's thing); `fold`
//! (`PACK`) folds the overlay back into the baseline. Direct reducers for now — P4 makes them
//! `init_zone` / `SET` / `PACK` events (see `docs/work/shard-tables/`).
//!
//! Shapes: `docs/VARIABLES.md` (`cold_row_reference`, `kind_pos_reference`), `docs/TABLES.md`.
//! **`type_id` is this module** (`TYPE_BIOME_THING`), so it's off the row.

use spacetimedb::{reducer, ReducerContext, Table};

use resonantdust_codec::object::{
    kind_pos_ref_data, kind_pos_ref_kind_reference, kind_pos_ref_tile, pack_cold_row_reference,
};

// Cold storage: the sparse baseline (`entity_state`, owns the shard `clock`, `data: u8` payload) + the
// sparse overlay (`overlay`, same payload). Both `cold_row_reference`-addressed; the client reads
// `entity_state ⊕ overlay`.
resonantdust_codec::sparse_entity_tables!(data: u8);
resonantdust_codec::overlay_tables!(data: u8);

/// The cold shard's current tic (its `clock` mirror, bumped by the master).
fn now_tic(ctx: &ReducerContext) -> u16 {
    ctx.db.clock().id().find(0).map(|c| c.master_tic).unwrap_or(0)
}

/// Seed (or overwrite) a zone-layer-biome's scatter into the **baseline** `entity_state`. Trusted
/// server-side **generation** (worldgen), not a player mutation — player edits go through the overlay.
/// Takes the packed `kind_pos_reference`s (`kind:16 | tile:8 | data:8`) worldgen already produces and
/// unpacks each into a `DenseItem`. Idempotent: re-seeding a `(zone, subtype, layer)` overwrites
/// deterministically. (Sparse; entries are expected to hold distinct `tile_reference`s.)
#[reducer]
pub fn seed(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    things: Vec<u32>,
) -> Result<(), String> {
    let cold_row_reference = pack_cold_row_reference(macro_position, subtype_id, layer_id);
    let items: Vec<DenseItem> = things
        .into_iter()
        .map(|e| DenseItem {
            tile_reference: kind_pos_ref_tile(e),
            kind_reference: kind_pos_ref_kind_reference(e),
            data: kind_pos_ref_data(e),
        })
        .collect();
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

/// **Mutate/place a scatter thing** at a cell to `(kind_reference, data)` via the **overlay** — a
/// sparse per-cell override that never touches the baseline `entity_state`. `kind_reference == 0`
/// writes a **removal** override (the composite reads the cell as empty). Idempotent by
/// `(cold_row, tile_reference)`. The direct write primitive; event-driven `SET` routing is P4.
#[reducer]
pub fn set_thing(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_reference: u8,
    kind_reference: u16,
    data: u8,
) -> Result<(), String> {
    overlay_cell(ctx, macro_position, subtype_id, layer_id, tile_reference, kind_reference, data);
    Ok(())
}

/// **Place one KIND at many cells** through the overlay — the batched sibling of [`set_thing`], and the
/// primitive for trusted placement of objects into an already-generated zone (torches at world init today;
/// a build action or world editor later).
///
/// Deliberately **kind-agnostic**: it takes a `kind_reference`, never a name. The module links only
/// `spacetimedb` + `resonantdust-codec` — it has no DSL, so it *cannot* resolve `"torch"`, and hardcoding an
/// id here would create a second authority for name→id beside the corpus (kind ids are append-ordered
/// precisely so nothing renumbers). The **caller** resolves the name where the DSL lives: the edge, at the
/// point of use, from its live hot-reloadable bundle. See `docs/work/2026-07-26-torch-thing/` F2.
///
/// Idempotent per cell, exactly as [`set_thing`] is: re-running with the same arguments leaves identical
/// rows, so it is safe to call on every zone seed.
///
/// Overlay rather than [`seed`], which would REPLACE a whole `(zone, subtype, layer)` baseline row and
/// erase the zone's worldgen things. The overlay composites over the baseline, so terrain survives (F1).
#[reducer]
pub fn place_things(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_references: Vec<u8>,
    kind_reference: u16,
    data: u8,
) -> Result<(), String> {
    // Per cell, not per batch: cells in one call may sit in different biomes, and the cell's own biome is
    // what the overlay row is keyed by. Sharing one lookup across the batch would file a torch under a
    // neighbour's subtype and split it from the row the client composites.
    for tile_reference in tile_references {
        overlay_cell(ctx, macro_position, subtype_id, layer_id, tile_reference, kind_reference, data);
    }
    Ok(())
}

/// The shared per-cell overlay merge behind [`set_thing`] and [`place_things`] — one definition so the
/// batched path cannot drift from the single-cell one.
fn overlay_cell(
    ctx: &ReducerContext,
    macro_position: u16,
    subtype_id: u16,
    layer_id: u8,
    tile_reference: u8,
    kind_reference: u16,
    data: u8,
) {
    // Keep the cell's biome — find the baseline row holding a thing at this cell (if any); else the
    // passed `subtype_id` (a placement onto an empty cell must be told the biome).
    let subtype_id = ctx
        .db
        .entity_state()
        .macro_position_reference()
        .filter(macro_position)
        .find(|r| r.layer_id == layer_id && r.items.iter().any(|it| it.tile_reference == tile_reference))
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
        Some(it) => {
            it.kind_reference = kind_reference;
            it.data = data;
        }
        None => items.push(OverlayItem { tile_reference, kind_reference, data }),
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
}

/// **GC fold (`PACK`)** — fold every settled overlay cell back into the sparse baseline `entity_state`,
/// then drop the overlay row. For each override cell: drop any existing baseline entry at its
/// `tile_reference`, then (if `kind_reference != 0`) add the new one — so a `kind == 0` override packs
/// as a removal. The client sees no change (it read the override; now it reads the same from the
/// baseline). The master's GC calls it on cadence.
#[reducer]
pub fn fold(ctx: &ReducerContext) -> Result<(), String> {
    // The baseline is current as of *now* once folded — `now_tic` is ≥ every settled override's tic,
    // so the client reads the folded value from the baseline, not the (stale) override.
    let ftic = now_tic(ctx);
    let overlays: Vec<Overlay> = ctx.db.overlay().iter().collect();
    for o in overlays {
        match ctx.db.entity_state().cold_row_reference().find(o.cold_row_reference) {
            Some(mut base) => {
                for it in &o.items {
                    base.items.retain(|e| e.tile_reference != it.tile_reference);
                    if it.kind_reference != 0 {
                        base.items.push(DenseItem {
                            tile_reference: it.tile_reference,
                            kind_reference: it.kind_reference,
                            data: it.data,
                        });
                    }
                }
                base.tic = ftic;
                ctx.db.entity_state().cold_row_reference().update(base);
            }
            None => {
                // No baseline row for this biome yet — create it with the non-removal cells.
                let items: Vec<DenseItem> = o
                    .items
                    .iter()
                    .filter(|it| it.kind_reference != 0)
                    .map(|it| DenseItem {
                        tile_reference: it.tile_reference,
                        kind_reference: it.kind_reference,
                        data: it.data,
                    })
                    .collect();
                if !items.is_empty() {
                    ctx.db.entity_state().insert(EntityState {
                        cold_row_reference: o.cold_row_reference,
                        macro_position_reference: o.macro_position_reference,
                        subtype_id: o.subtype_id,
                        layer_id: o.layer_id,
                        tic: ftic,
                        items,
                    });
                }
            }
        }
        // The value now lives in the baseline the client already reads — drop the overlay.
        ctx.db.overlay().cold_row_reference().delete(o.cold_row_reference);
    }
    Ok(())
}
