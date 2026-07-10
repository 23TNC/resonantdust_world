//! Zone-cell packing — the `u64` a zone entity's `cells: Vec<u64>` (length
//! [`CELLS_PER_ZONE`]) holds per tile. One cell carries every layer of that tile at once,
//! so a fully-terrained zone is one ~2 KB row, not 256+ (`docs/pipeline-generalization.md`).
//!
//! ```text
//! cell: u64 = tile:9 | stack:17 | primary:14 | secondary:14 | tertiary:10   (LSB→MSB)
//!   stack:17     = count:5 | def:12     (up to 32 of one stackable def)
//!   primary:14   = def:12 | rotation:2  (a placeable object — `b0100` kind)
//!   secondary:14 = def:12 | rotation:2  (a SECOND placeable — same kind class, other slot)
//!   tertiary:10  = def:8  | rotation:2  (a utilities-layer object — `b001` kind)
//! ```
//!
//! `primary` and `secondary` are two slots for the *same* object class (both `b0100`
//! placeables) — that is how two objects share a tile; the kind prefix does not
//! distinguish them, the slot does. A `store` action writes one slot, leaving the rest of
//! the cell intact (the `cell_with_*` setters). The tile-def / biome resolution (which
//! actual tile a `tile:9` selects for a zone's biome) is render-time and lives elsewhere.

/// Cells per zone (`16×16`). A zone's `cells` Vec has this length; `location` indexes it.
pub const CELLS_PER_ZONE: usize = 256;

// ── cell field layout ─────────────────────────────────────────────────────────

const CELL_TILE_SHIFT: u64 = 0;
const CELL_TILE_MASK: u64 = 0x1FF; // 9 bits
const CELL_STACK_SHIFT: u64 = 9;
const CELL_STACK_MASK: u64 = 0x1_FFFF; // 17 bits
const CELL_PRIMARY_SHIFT: u64 = 26;
const CELL_SECONDARY_SHIFT: u64 = 40;
const CELL_PLACEABLE_MASK: u64 = 0x3FFF; // 14 bits (primary + secondary)
const CELL_TERTIARY_SHIFT: u64 = 54;
const CELL_TERTIARY_MASK: u64 = 0x3FF; // 10 bits

/// Pack a whole cell from its five already-encoded fields. Each is masked to its width;
/// build the sub-fields with [`pack_stack`] / [`pack_placeable`] / [`pack_tertiary`].
pub fn pack_cell(tile: u16, stack: u32, primary: u16, secondary: u16, tertiary: u16) -> u64 {
    ((tile as u64 & CELL_TILE_MASK) << CELL_TILE_SHIFT)
        | ((stack as u64 & CELL_STACK_MASK) << CELL_STACK_SHIFT)
        | ((primary as u64 & CELL_PLACEABLE_MASK) << CELL_PRIMARY_SHIFT)
        | ((secondary as u64 & CELL_PLACEABLE_MASK) << CELL_SECONDARY_SHIFT)
        | ((tertiary as u64 & CELL_TERTIARY_MASK) << CELL_TERTIARY_SHIFT)
}

/// The `tile:9` field (floor selector; resolved to a def via biome at render time).
pub fn cell_tile(c: u64) -> u16 {
    ((c >> CELL_TILE_SHIFT) & CELL_TILE_MASK) as u16
}

/// The `stack:17` field — decode with [`stack_count`] / [`stack_def`].
pub fn cell_stack(c: u64) -> u32 {
    ((c >> CELL_STACK_SHIFT) & CELL_STACK_MASK) as u32
}

/// The `primary:14` placeable slot — decode with [`placeable_def`] / [`placeable_rotation`].
pub fn cell_primary(c: u64) -> u16 {
    ((c >> CELL_PRIMARY_SHIFT) & CELL_PLACEABLE_MASK) as u16
}

/// The `secondary:14` placeable slot (the second object on the tile).
pub fn cell_secondary(c: u64) -> u16 {
    ((c >> CELL_SECONDARY_SHIFT) & CELL_PLACEABLE_MASK) as u16
}

/// The `tertiary:10` utilities slot — decode with [`tertiary_def`] / [`tertiary_rotation`].
pub fn cell_tertiary(c: u64) -> u16 {
    ((c >> CELL_TERTIARY_SHIFT) & CELL_TERTIARY_MASK) as u16
}

// ── slot setters (leave the rest of the cell intact) — the `store` write path ──

/// Replace the `tile` field, keeping every other layer.
pub fn cell_with_tile(c: u64, tile: u16) -> u64 {
    (c & !(CELL_TILE_MASK << CELL_TILE_SHIFT)) | ((tile as u64 & CELL_TILE_MASK) << CELL_TILE_SHIFT)
}

/// Replace the `stack` field, keeping every other layer.
pub fn cell_with_stack(c: u64, stack: u32) -> u64 {
    (c & !(CELL_STACK_MASK << CELL_STACK_SHIFT))
        | ((stack as u64 & CELL_STACK_MASK) << CELL_STACK_SHIFT)
}

/// Replace the `primary` placeable slot, keeping every other layer.
pub fn cell_with_primary(c: u64, primary: u16) -> u64 {
    (c & !(CELL_PLACEABLE_MASK << CELL_PRIMARY_SHIFT))
        | ((primary as u64 & CELL_PLACEABLE_MASK) << CELL_PRIMARY_SHIFT)
}

/// Replace the `secondary` placeable slot, keeping every other layer.
pub fn cell_with_secondary(c: u64, secondary: u16) -> u64 {
    (c & !(CELL_PLACEABLE_MASK << CELL_SECONDARY_SHIFT))
        | ((secondary as u64 & CELL_PLACEABLE_MASK) << CELL_SECONDARY_SHIFT)
}

/// Replace the `tertiary` slot, keeping every other layer.
pub fn cell_with_tertiary(c: u64, tertiary: u16) -> u64 {
    (c & !(CELL_TERTIARY_MASK << CELL_TERTIARY_SHIFT))
        | ((tertiary as u64 & CELL_TERTIARY_MASK) << CELL_TERTIARY_SHIFT)
}

// ── sub-field encodings ────────────────────────────────────────────────────────

const STACK_DEF_SHIFT: u32 = 5;
const STACK_COUNT_MASK: u32 = 0x1F; // 5 bits
const STACK_DEF_MASK: u32 = 0xFFF; // 12 bits
/// Max items in a stack (`count:5`).
pub const STACK_MAX: u8 = 31;

const PLACEABLE_ROT_SHIFT: u16 = 12;
const PLACEABLE_DEF_MASK: u16 = 0xFFF; // 12 bits
const TERTIARY_ROT_SHIFT: u16 = 8;
const TERTIARY_DEF_MASK: u16 = 0xFF; // 8 bits
const ROT_MASK: u16 = 0x3; // 2 bits

/// Pack the `stack` field: `count:5 | def:12` (up to [`STACK_MAX`] of one stackable def).
pub fn pack_stack(count: u8, def: u16) -> u32 {
    ((count as u32) & STACK_COUNT_MASK) | (((def as u32) & STACK_DEF_MASK) << STACK_DEF_SHIFT)
}

/// The count of a packed stack.
pub fn stack_count(stack: u32) -> u8 {
    (stack & STACK_COUNT_MASK) as u8
}

/// The def id of a packed stack.
pub fn stack_def(stack: u32) -> u16 {
    ((stack >> STACK_DEF_SHIFT) & STACK_DEF_MASK) as u16
}

/// Pack a placeable slot: `def:12 | rotation:2` (a `b0100`-class object).
pub fn pack_placeable(def: u16, rotation: u8) -> u16 {
    (def & PLACEABLE_DEF_MASK) | (((rotation as u16) & ROT_MASK) << PLACEABLE_ROT_SHIFT)
}

/// The def id of a packed placeable slot.
pub fn placeable_def(p: u16) -> u16 {
    p & PLACEABLE_DEF_MASK
}

/// The rotation (0..3) of a packed placeable slot.
pub fn placeable_rotation(p: u16) -> u8 {
    ((p >> PLACEABLE_ROT_SHIFT) & ROT_MASK) as u8
}

/// Pack a tertiary (utilities) slot: `def:8 | rotation:2`.
pub fn pack_tertiary(def: u16, rotation: u8) -> u16 {
    (def & TERTIARY_DEF_MASK) | (((rotation as u16) & ROT_MASK) << TERTIARY_ROT_SHIFT)
}

/// The def id of a packed tertiary slot.
pub fn tertiary_def(t: u16) -> u16 {
    t & TERTIARY_DEF_MASK
}

/// The rotation (0..3) of a packed tertiary slot.
pub fn tertiary_rotation(t: u16) -> u8 {
    ((t >> TERTIARY_ROT_SHIFT) & ROT_MASK) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_fields_roundtrip_and_are_disjoint() {
        // Full-width value in every field — nothing overlaps.
        let c = pack_cell(0x1FF, 0x1_FFFF, 0x3FFF, 0x3FFF, 0x3FF);
        assert_eq!(cell_tile(c), 0x1FF);
        assert_eq!(cell_stack(c), 0x1_FFFF);
        assert_eq!(cell_primary(c), 0x3FFF);
        assert_eq!(cell_secondary(c), 0x3FFF);
        assert_eq!(cell_tertiary(c), 0x3FF);
        assert_eq!(c, u64::MAX, "the five fields tile exactly the u64");
    }

    #[test]
    fn distinct_values_stay_separate() {
        let c = pack_cell(0x0AB, 0x0_1234, 0x0C1, 0x0D2, 0x1E3);
        assert_eq!(cell_tile(c), 0x0AB);
        assert_eq!(cell_stack(c), 0x0_1234);
        assert_eq!(cell_primary(c), 0x0C1);
        assert_eq!(cell_secondary(c), 0x0D2);
        assert_eq!(cell_tertiary(c), 0x1E3);
    }

    #[test]
    fn setters_touch_only_their_slot() {
        // Two placeables share a tile via the primary + secondary slots; setting one
        // leaves the other (and the floor) untouched — the `store` write path.
        let base = pack_cell(0x0AB, 0, 0, 0, 0);
        let with_p = cell_with_primary(base, pack_placeable(0x111, 1));
        let with_both = cell_with_secondary(with_p, pack_placeable(0x222, 2));
        assert_eq!(cell_tile(with_both), 0x0AB, "floor untouched");
        assert_eq!(placeable_def(cell_primary(with_both)), 0x111);
        assert_eq!(placeable_rotation(cell_primary(with_both)), 1);
        assert_eq!(placeable_def(cell_secondary(with_both)), 0x222);
        assert_eq!(placeable_rotation(cell_secondary(with_both)), 2);
        // Overwriting primary leaves secondary intact.
        let repl = cell_with_primary(with_both, pack_placeable(0x333, 0));
        assert_eq!(placeable_def(cell_primary(repl)), 0x333);
        assert_eq!(placeable_def(cell_secondary(repl)), 0x222);
    }

    #[test]
    fn stack_subfield_roundtrips() {
        let s = pack_stack(STACK_MAX, 0xABC);
        assert_eq!(stack_count(s), STACK_MAX);
        assert_eq!(stack_def(s), 0xABC);
    }

    #[test]
    fn placeable_and_tertiary_subfields_roundtrip() {
        let p = pack_placeable(0xFFF, 3);
        assert_eq!(placeable_def(p), 0xFFF);
        assert_eq!(placeable_rotation(p), 3);
        let t = pack_tertiary(0xFF, 3);
        assert_eq!(tertiary_def(t), 0xFF);
        assert_eq!(tertiary_rotation(t), 3);
    }
}
