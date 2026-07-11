//! Bit-packing helpers — `valid_at` primary keys, `zone_id` composition, zone
//! cells, and packed things/tiles. The single source of truth for every wire and
//! storage bit-layout; the server packs and the client unpacks with the same
//! code.
//!
//! The shapes the game is built around:
//!
//! ```text
//! valid_at:  u64 = (time_ms: u48 << 16) | sequence: u16
//! zone_id:   u32 = region_x:8 | region_y:8 | surface:8 | zone_x:4 | zone_y:4
//! region_id: u32 = region_x:8 | region_y:8 | surface:8 | reserved:8
//! tile:      u8  = tile-kind (0 = empty); a zone's tiles are a dense Vec<u8>[256]
//! thing:     u64 = kind:16 | x:4 | y:4 | data:5 | layer:3 | variant:5 | reserved:27
//!                  (sparse Vec<u64>)
//! offset:    u8  = x_off:4 | y_off:4    (object-shard free things: sub-tile pos)
//! ```

// ── valid_at PK ──────────────────────────────────────────────────────────────

/// Pack `(time_ms, sequence)` into the u64 `valid_at` primary key. Rows for one
/// key (one zone, one player, one chat channel …) order by this composite —
/// `time_ms` dominates, `sequence` tie-breaks writes within the same ms.
pub fn pack_valid_at(time_ms: u64, sequence: u16) -> u64 {
    (time_ms << 16) | (sequence as u64)
}

/// Extract the `time_ms` half of a packed `valid_at`.
pub fn valid_at_time(v: u64) -> u64 {
    v >> 16
}

/// Extract the `sequence` half of a packed `valid_at`.
pub fn valid_at_sequence(v: u64) -> u16 {
    (v & 0xFFFF) as u16
}

// ── zone_id ↔ region_id ──────────────────────────────────────────────────────
//
// `zone_id` addresses a single zone within the whole world; `region_id` is the
// shard-routing key — a `zone_id` with its low byte (the in-region `zone_x|zone_y`
// nibbles) cleared. The shard treats `zone_id` as opaque; composing and
// decomposing it is the gateway's / client's concern, which is why these live in
// the shared codec rather than in any one module.

/// Mask turning a `zone_id` into its `region_id`: clears the low byte
/// (`zone_x | zone_y`), leaving `region_x | region_y | surface` and a zero
/// `reserved` byte.
pub const REGION_ID_MASK: u32 = 0xFFFF_FF00;

const ZONE_RX_SHIFT: u32 = 24;
const ZONE_RY_SHIFT: u32 = 16;
const ZONE_SURFACE_SHIFT: u32 = 8;
const ZONE_X_SHIFT: u32 = 4;
const ZONE_BYTE: u32 = 0xFF;
/// Low nibble mask — the in-region `zone_x`/`zone_y` (and, via the thing packers, cell
/// x/y) widths.
const NIBBLE: u32 = 0x0F;

/// Compose a `zone_id` from its parts. `region_x`/`region_y`/`surface` are full
/// bytes; `zone_x`/`zone_y` are the in-region nibbles (`0..REGION_DIM`), masked
/// to 4 bits each.
pub fn pack_zone_id(region_x: u8, region_y: u8, surface: u8, zone_x: u8, zone_y: u8) -> u32 {
    ((region_x as u32) << ZONE_RX_SHIFT)
        | ((region_y as u32) << ZONE_RY_SHIFT)
        | ((surface as u32) << ZONE_SURFACE_SHIFT)
        | ((zone_x as u32 & NIBBLE) << ZONE_X_SHIFT)
        | (zone_y as u32 & NIBBLE)
}

/// The `region_id` owning a `zone_id` (the zone_id with its low byte zeroed).
pub fn region_of(zone_id: u32) -> u32 {
    zone_id & REGION_ID_MASK
}

/// The `region_x` byte of a `zone_id` (or `region_id`).
pub fn zone_region_x(zone_id: u32) -> u8 {
    ((zone_id >> ZONE_RX_SHIFT) & ZONE_BYTE) as u8
}

/// The `region_y` byte of a `zone_id` (or `region_id`).
pub fn zone_region_y(zone_id: u32) -> u8 {
    ((zone_id >> ZONE_RY_SHIFT) & ZONE_BYTE) as u8
}

/// The `surface` byte of a `zone_id` (or `region_id`).
pub fn zone_surface(zone_id: u32) -> u8 {
    ((zone_id >> ZONE_SURFACE_SHIFT) & ZONE_BYTE) as u8
}

/// The in-region `zone_x` nibble of a `zone_id`.
pub fn zone_x(zone_id: u32) -> u8 {
    ((zone_id >> ZONE_X_SHIFT) & NIBBLE) as u8
}

/// The in-region `zone_y` nibble of a `zone_id`.
pub fn zone_y(zone_id: u32) -> u8 {
    (zone_id & NIBBLE) as u8
}

// ── zone geometry ────────────────────────────────────────────────────────────

/// Tiles per zone edge. A zone is `ZONE_DIM × ZONE_DIM` cells.
pub const ZONE_DIM: u8 = 16;
/// Cells per zone (`ZONE_DIM²` = 256). The `location` byte indexes `0..ZONE_TILES`.
pub const ZONE_TILES: usize = (ZONE_DIM as usize) * (ZONE_DIM as usize);
/// Zones per region edge. A region is `REGION_DIM × REGION_DIM` zones
/// (= 256×256 tiles).
pub const REGION_DIM: u8 = 16;

/// Cell index `0..256` from in-zone `(x, y)` — row-major, `y << 4 | x`. Matches
/// the `x`/`y` nibble positions of a [packed thing](pack_thing).
pub fn cell(x: u8, y: u8) -> u8 {
    ((y & 0x0F) << 4) | (x & 0x0F)
}

/// The `x` (column) of a cell index.
pub fn cell_x(location: u8) -> u8 {
    location & 0x0F
}

/// The `y` (row) of a cell index.
pub fn cell_y(location: u8) -> u8 {
    location >> 4
}

/// Tiles per region edge (`REGION_DIM` zones × `ZONE_DIM` tiles = 256). A region
/// spans `REGION_TILES × REGION_TILES` tiles.
pub const REGION_TILES: i32 = REGION_DIM as i32 * ZONE_DIM as i32;

/// Global tile `(gx, gy)` on `surface` → its `(zone_id, location)`. Floor-division
/// throughout so off-origin (negative) tiles map correctly. Matches the client's
/// anchor→zone convention (`client::zones::{zone_axis, zone_at}`), so a row written
/// at this `zone_id` lands in exactly the zone the client subscribes for that tile.
pub fn zone_and_location(gx: i32, gy: i32, surface: u8) -> (u32, u8) {
    let zgx = gx.div_euclid(ZONE_DIM as i32); // global zone coordinate
    let zgy = gy.div_euclid(ZONE_DIM as i32);
    let region_x = zgx.div_euclid(REGION_DIM as i32) as u8;
    let region_y = zgy.div_euclid(REGION_DIM as i32) as u8;
    let zone_x = zgx.rem_euclid(REGION_DIM as i32) as u8;
    let zone_y = zgy.rem_euclid(REGION_DIM as i32) as u8;
    let cx = gx.rem_euclid(ZONE_DIM as i32) as u8;
    let cy = gy.rem_euclid(ZONE_DIM as i32) as u8;
    (pack_zone_id(region_x, region_y, surface, zone_x, zone_y), cell(cx, cy))
}

/// `(zone_id, location)` → its global tile `(gx, gy)`. Inverse of
/// [`zone_and_location`].
pub fn global_tile(zone_id: u32, location: u8) -> (i32, i32) {
    let gx = zone_region_x(zone_id) as i32 * REGION_TILES
        + zone_x(zone_id) as i32 * ZONE_DIM as i32
        + cell_x(location) as i32;
    let gy = zone_region_y(zone_id) as i32 * REGION_TILES
        + zone_y(zone_id) as i32 * ZONE_DIM as i32
        + cell_y(location) as i32;
    (gx, gy)
}

// ── packed thing (sparse zone `things` entries) ──────────────────────────────
//
// Layout (LSB→MSB): layer:3 | data:5 | y:4 | x:4 | kind:16 | variant:5 | reserved:27.
//   kind:16     — the thing's what-kind, its own def-id namespace (SEPARATE from
//                 tiles). Room to partition into content categories later
//                 (utilities / secondary / primary / stacks) so a cell holds more
//                 than the 8 the layer alone gives.
//   x:4,y:4     — the thing's cell within its 16×16 zone.
//   data:5      — kind-interpreted: rotation for placeables, count for stacks, …
//   layer:3     — up to 8 things per cell (floor / wall / affixed / …).
//   variant:5   — which of up to 32 sprite variants of `kind` to render; the corpus
//                 may define fewer, so the renderer takes it modulo the kind's count.
//   reserved:27 — headroom (per-thing hp/state/flags as things grow richer).
// A zone's things are a SPARSE `Vec<u64>` — only occupied (cell, layer) slots cost
// storage, so a mostly-empty zone is a few bytes rather than 256 fixed slots.

/// Largest thing `kind` the u16 field holds.
pub const THING_KIND_MAX: u16 = u16::MAX;
/// Largest `data` value (5 bits).
pub const THING_DATA_MAX: u8 = 0x1F;
/// Largest `variant` value (5 bits → 32 sprite variants per kind).
pub const THING_VARIANT_MAX: u8 = 0x1F;
/// Layers per cell (3 bits → 8 things stackable on one tile per kind-category).
pub const THING_LAYERS: u8 = 8;

const THING_LAYER_SHIFT: u64 = 0;
const THING_DATA_SHIFT: u64 = 3;
const THING_Y_SHIFT: u64 = 8;
const THING_X_SHIFT: u64 = 12;
const THING_KIND_SHIFT: u64 = 16;
const THING_VARIANT_SHIFT: u64 = 32;

const THING_NIBBLE: u64 = 0x0F;
const THING_LAYER_MASK: u64 = 0x07;
const THING_DATA_MASK: u64 = 0x1F;
const THING_KIND_MASK: u64 = 0xFFFF;
const THING_VARIANT_MASK: u64 = 0x1F;

/// Pack a sparse zone thing into its `u64` entry: `kind:16 | x:4 | y:4 | data:5 |
/// layer:3 | variant:5` (+ 27 reserved). Each field is masked to its width.
pub fn pack_thing(kind: u16, x: u8, y: u8, data: u8, layer: u8, variant: u8) -> u64 {
    ((kind as u64 & THING_KIND_MASK) << THING_KIND_SHIFT)
        | ((x as u64 & THING_NIBBLE) << THING_X_SHIFT)
        | ((y as u64 & THING_NIBBLE) << THING_Y_SHIFT)
        | ((data as u64 & THING_DATA_MASK) << THING_DATA_SHIFT)
        | ((layer as u64 & THING_LAYER_MASK) << THING_LAYER_SHIFT)
        | ((variant as u64 & THING_VARIANT_MASK) << THING_VARIANT_SHIFT)
}

/// Pack a thing addressed by a cell index instead of `(x, y)`.
pub fn pack_thing_at(location: u8, kind: u16, data: u8, layer: u8, variant: u8) -> u64 {
    pack_thing(kind, cell_x(location), cell_y(location), data, layer, variant)
}

/// The what-kind of a packed thing (its own namespace, separate from tiles).
pub fn thing_kind(packed: u64) -> u16 {
    ((packed >> THING_KIND_SHIFT) & THING_KIND_MASK) as u16
}

/// The `x` (column) of a packed thing.
pub fn thing_x(packed: u64) -> u8 {
    ((packed >> THING_X_SHIFT) & THING_NIBBLE) as u8
}

/// The `y` (row) of a packed thing.
pub fn thing_y(packed: u64) -> u8 {
    ((packed >> THING_Y_SHIFT) & THING_NIBBLE) as u8
}

/// The cell index of a packed thing (`y << 4 | x`).
pub fn thing_location(packed: u64) -> u8 {
    cell(thing_x(packed), thing_y(packed))
}

/// The kind-interpreted `data` (5 bits) of a packed thing — rotation, count, ….
pub fn thing_data(packed: u64) -> u8 {
    ((packed >> THING_DATA_SHIFT) & THING_DATA_MASK) as u8
}

/// The layer (0..8) of a packed thing.
pub fn thing_layer(packed: u64) -> u8 {
    ((packed >> THING_LAYER_SHIFT) & THING_LAYER_MASK) as u8
}

/// The sprite `variant` (0..32) of a packed thing — the renderer takes it modulo the
/// kind's actual variant count (the corpus may define fewer than 32).
pub fn thing_variant(packed: u64) -> u8 {
    ((packed >> THING_VARIANT_SHIFT) & THING_VARIANT_MASK) as u8
}

// ── zone tiles ───────────────────────────────────────────────────────────────
//
// A zone's ground is a dense `Vec<u8>` of length [`ZONE_TILES`] (256), one
// **tile-kind** per cell in row-major `cell(x, y)` order; `0` = empty. Tile-kind
// is a plain `u8` in its own namespace (separate from thing `kind`) — the byte IS
// the kind, no bit-packing. 256 tile kinds; the corpus assigns them in content
// order.

// ── sub-tile offset (object-shard free things) ───────────────────────────────
//
// A `u8` holding a free thing's position *within* its tile: `x_off:4 | y_off:4`
// (x in the low nibble, mirroring the packed-thing x/y order). Each axis is a
// 1/16-of-a-tile step (`0..OFFSET_STEPS`), so on a 64px tile one step is 4px.
// Affixed things are tile-snapped and have no offset; only the object shard's
// `free_things` rows carry one — deliberately a separate byte rather than
// spending the packed thing's reserved bits, which stay as headroom.

/// Sub-tile steps per axis (`OFFSET_STEPS × OFFSET_STEPS` positions in a tile).
pub const OFFSET_STEPS: u8 = 16;

const OFFSET_Y_SHIFT: u8 = 4;

/// Pack a sub-tile `(x_off, y_off)` into the `u8` offset. Each is masked to a
/// nibble (`0..OFFSET_STEPS`).
pub fn pack_offset(x_off: u8, y_off: u8) -> u8 {
    (x_off & 0x0F) | ((y_off & 0x0F) << OFFSET_Y_SHIFT)
}

/// The `x_off` nibble of a packed offset.
pub fn offset_x(offset: u8) -> u8 {
    offset & 0x0F
}

/// The `y_off` nibble of a packed offset.
pub fn offset_y(offset: u8) -> u8 {
    offset >> OFFSET_Y_SHIFT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_at_roundtrips() {
        let v = pack_valid_at(1_700_000_000_000, 42);
        assert_eq!(valid_at_time(v), 1_700_000_000_000);
        assert_eq!(valid_at_sequence(v), 42);
    }

    #[test]
    fn zone_id_roundtrips() {
        let z = pack_zone_id(0xAB, 0xCD, 0x02, 13, 7);
        assert_eq!(zone_region_x(z), 0xAB);
        assert_eq!(zone_region_y(z), 0xCD);
        assert_eq!(zone_surface(z), 0x02);
        assert_eq!(zone_x(z), 13);
        assert_eq!(zone_y(z), 7);
        // region_of clears exactly the zone_x|zone_y nibbles, nothing above.
        let r = region_of(z);
        assert_eq!(zone_region_x(r), 0xAB);
        assert_eq!(zone_region_y(r), 0xCD);
        assert_eq!(zone_surface(r), 0x02);
        assert_eq!(zone_x(r), 0);
        assert_eq!(zone_y(r), 0);
        // two zones in the same region resolve to one region_id.
        assert_eq!(region_of(pack_zone_id(0xAB, 0xCD, 0x02, 0, 0)), r);
        assert_eq!(region_of(pack_zone_id(0xAB, 0xCD, 0x02, 15, 15)), r);
    }

    #[test]
    fn cell_roundtrips() {
        for x in 0..16u8 {
            for y in 0..16u8 {
                let loc = cell(x, y);
                assert_eq!(cell_x(loc), x);
                assert_eq!(cell_y(loc), y);
            }
        }
        assert_eq!(cell(15, 15), 255);
    }

    #[test]
    fn thing_roundtrips() {
        let p = pack_thing(0xBEEF, 13, 7, 0x1A, 5, 0x0C);
        assert_eq!(thing_kind(p), 0xBEEF);
        assert_eq!(thing_x(p), 13);
        assert_eq!(thing_y(p), 7);
        assert_eq!(thing_data(p), 0x1A);
        assert_eq!(thing_layer(p), 5);
        assert_eq!(thing_variant(p), 0x0C);
        assert_eq!(thing_location(p), cell(13, 7));
        // by-location constructor agrees
        assert_eq!(pack_thing_at(cell(13, 7), 0xBEEF, 0x1A, 5, 0x0C), p);
        // every field disjoint: 37 bits used (kind..variant), reserved:27 stays 0.
        let full =
            pack_thing(THING_KIND_MAX, 15, 15, THING_DATA_MAX, THING_LAYERS - 1, THING_VARIANT_MAX);
        assert_eq!(thing_kind(full), THING_KIND_MAX);
        assert_eq!(thing_data(full), THING_DATA_MAX);
        assert_eq!(thing_layer(full), THING_LAYERS - 1);
        assert_eq!(thing_variant(full), THING_VARIANT_MAX);
        assert_eq!(full, 0x0000_001F_FFFF_FFFF);
    }

    #[test]
    fn offset_roundtrips() {
        for x in 0..OFFSET_STEPS {
            for y in 0..OFFSET_STEPS {
                let o = pack_offset(x, y);
                assert_eq!(offset_x(o), x);
                assert_eq!(offset_y(o), y);
            }
        }
        // both nibbles full → 0xFF; x and y occupy disjoint bits.
        assert_eq!(pack_offset(0xF, 0xF), 0xFF);
        assert_eq!(pack_offset(0x3, 0xC), 0xC3);
    }

    #[test]
    fn global_tile_roundtrips() {
        for &(gx, gy) in &[(0, 0), (15, 0), (16, 3), (255, 255), (256, 256), (1000, 42)] {
            let (z, loc) = zone_and_location(gx, gy, 0);
            assert_eq!(global_tile(z, loc), (gx, gy), "roundtrip ({gx},{gy})");
        }
    }

    #[test]
    fn global_tile_matches_zone_layout() {
        // (8,8) → region 0, zone 0, cell (8,8).
        let (z, loc) = zone_and_location(8, 8, 0);
        assert_eq!(z, pack_zone_id(0, 0, 0, 0, 0));
        assert_eq!((cell_x(loc), cell_y(loc)), (8, 8));
        // Crossing the zone edge bumps zone_x; crossing the region edge bumps region_x.
        assert_eq!(zone_x(zone_and_location(16, 0, 0).0), 1);
        assert_eq!(zone_region_x(zone_and_location(256, 0, 0).0), 1);
    }

}
