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
//! thing:     u32 = x:4 | y:4 | rotation:2 | object_id:12 | reserved:10
//!                  (a layer field will later be carved from the reserved bits)
//! tile:      u16 = def_id:12 | reserved:4
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

// ── packed thing (cold `things` entries) ─────────────────────────────────────
//
// Layout (LSB→MSB): x:4 | y:4 | rotation:2 | object_id:12 | reserved:10.
// `object_id` is a **u12** ([`DEF_ID_MAX`]) — the thing def-id space; the 10
// reserved bits are headroom for later. There is one thing list per zone; a
// layer field carved from the reserved bits will distinguish floor-level from
// wall-level things (folding `primary_thing` + `wall_thing` into one list).
// Tiles use the same u12 def width in their
// own separate namespace (see the tile-slot helpers below). The hot `id` column
// is a u16 for all three layers (no u12 scalar exists), so the hot write path
// rejects any `id` above the u12 max rather than truncating it on fold. Rotation
// is only meaningful for the two thing layers (tiles don't rotate).

/// Largest def id the u12 packing holds (4095). The shared width for tile defs
/// and thing `object_id`s (separate namespaces, same size); the hot write path
/// validates every `id` against it.
pub const DEF_ID_MAX: u16 = 0x0FFF;

const THING_X_SHIFT: u32 = 0;
const THING_Y_SHIFT: u32 = 4;
const THING_ROT_SHIFT: u32 = 8;
const THING_OBJ_SHIFT: u32 = 10;

const NIBBLE: u32 = 0x0F;
const THING_ROT_MASK: u32 = 0x03;
const THING_OBJ_MASK: u32 = 0x0FFF;

/// Pack an in-zone thing into the cold `u32` entry. `object_id` is masked to u12;
/// callers reject out-of-range ids upstream (see the shard's hot write path).
/// `object_id == 0` is the "empty" sentinel — callers shouldn't pack it (a
/// removed thing is dropped from the vec, not stored as a 0-id entry).
pub fn pack_thing(x: u8, y: u8, rotation: u8, object_id: u16) -> u32 {
    ((x as u32 & NIBBLE) << THING_X_SHIFT)
        | ((y as u32 & NIBBLE) << THING_Y_SHIFT)
        | ((rotation as u32 & THING_ROT_MASK) << THING_ROT_SHIFT)
        | ((object_id as u32 & THING_OBJ_MASK) << THING_OBJ_SHIFT)
}

/// Pack a thing addressed by a cell index instead of `(x, y)`.
pub fn pack_thing_at(location: u8, rotation: u8, object_id: u16) -> u32 {
    pack_thing(cell_x(location), cell_y(location), rotation, object_id)
}

/// The `x` (column) of a packed thing.
pub fn thing_x(packed: u32) -> u8 {
    ((packed >> THING_X_SHIFT) & NIBBLE) as u8
}

/// The `y` (row) of a packed thing.
pub fn thing_y(packed: u32) -> u8 {
    ((packed >> THING_Y_SHIFT) & NIBBLE) as u8
}

/// The cell index of a packed thing (`y << 4 | x`).
pub fn thing_location(packed: u32) -> u8 {
    cell(thing_x(packed), thing_y(packed))
}

/// The rotation (0..3) of a packed thing.
pub fn thing_rotation(packed: u32) -> u8 {
    ((packed >> THING_ROT_SHIFT) & THING_ROT_MASK) as u8
}

/// The object/def id of a packed thing.
pub fn thing_object_id(packed: u32) -> u16 {
    ((packed >> THING_OBJ_SHIFT) & THING_OBJ_MASK) as u16
}

// ── cold tile slot (`cold_zones.tiles[location]`) ────────────────────────────
//
// Each tile cell is a u16: `def_id:12 | reserved:4`. The def id is a u12 in the
// tile namespace ([`DEF_ID_MAX`]); the top 4 bits are reserved per cell for later
// use. Tiles don't rotate. `def_id == 0` = empty (no floor/wall). The hot tile
// row carries only the def `id`, so the fold preserves a cell's existing reserved
// nibble across a def change (see the shard's `fold_hot_to_cold`).

const TILE_DEF_MASK: u16 = 0x0FFF;
const TILE_RESERVED_SHIFT: u16 = 12;
const TILE_RESERVED_MASK: u16 = 0x0F;

/// Pack a tile slot from a u12 `def_id` and its 4 reserved bits.
pub fn pack_tile(def_id: u16, reserved: u8) -> u16 {
    (def_id & TILE_DEF_MASK) | (((reserved as u16) & TILE_RESERVED_MASK) << TILE_RESERVED_SHIFT)
}

/// The u12 def id of a tile slot.
pub fn tile_def(slot: u16) -> u16 {
    slot & TILE_DEF_MASK
}

/// The 4 reserved bits of a tile slot.
pub fn tile_reserved(slot: u16) -> u8 {
    ((slot >> TILE_RESERVED_SHIFT) & TILE_RESERVED_MASK) as u8
}

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

// ── entity key (simulation pipeline) ─────────────────────────────────────────
//
// A tagged `u64` naming any simulation entity across both shard classes:
//
// ```text
// entity_key: u64 = tag:2 | payload:62
//   OBJECT (tag=01): payload = object_reference     (mobile things; object shards)
//   ZONE   (tag=10): payload = zone_id:32 << 16 | location:8 << 8 | layer:8   (cells)
// ```
//
// The tag makes object and zone payloads disjoint, so the two classes share **one**
// globally-unique key space. That is what lets `priority` (a hash of the key) be a
// total order spanning both classes — the property that breaks cross-class dependency
// cycles in the resolution DAG (see `docs/simulation.md`). Events address an actor or
// target by this key; `state_log`/`state` are identified by it. Zone cells have no id
// of their own — position *is* the identity — which is why the zone payload is just
// `(zone_id, location)` and objects carry their allocated `object_id`.

/// Reserved "no entity" tag (`0`).
pub const ENTITY_TAG_NONE: u8 = 0b00;
/// Object-shard entity: payload is an `object_id`.
pub const ENTITY_TAG_OBJECT: u8 = 0b01;
/// Zone-shard entity: payload is `(zone_id, location)`.
pub const ENTITY_TAG_ZONE: u8 = 0b10;

const ENTITY_TAG_SHIFT: u32 = 62;
/// The low 62 bits — the payload under a 2-bit tag. `object_id` must fit here.
pub const ENTITY_PAYLOAD_MAX: u64 = (1u64 << ENTITY_TAG_SHIFT) - 1;
// ZONE payload: zone_id:32 << 16 | location:8 << 8 | layer:8  (48 bits, 14 spare)
const ENTITY_ZONE_ID_SHIFT: u32 = 16;
const ENTITY_LOCATION_SHIFT: u32 = 8;

/// Zone cell layers — a `u8` lets multiple entities stack on one `(zone_id, location)`
/// (floor + wall + affixed things), each a distinct location-keyed cell. A starting
/// palette; content can define more (up to 255). (Mobile object-shard things stack
/// without a layer — they're keyed by `object_id`, positioned by `(zone_id, location)`
/// in their state — so the layer is only for location-keyed zone cells.)
pub const LAYER_FLOOR: u8 = 0;
pub const LAYER_WALL: u8 = 1;
pub const LAYER_THING: u8 = 2;

/// The entity key for an object-shard thing, from its `object_reference` (`obj_type`
/// + `object_id`). The reference is masked to the 62-bit payload.
pub fn pack_object_key(object_reference: u64) -> u64 {
    ((ENTITY_TAG_OBJECT as u64) << ENTITY_TAG_SHIFT) | (object_reference & ENTITY_PAYLOAD_MAX)
}

/// The entity key for a zone-shard cell `(zone_id, location, layer)`.
pub fn pack_zone_key(zone_id: u32, location: u8, layer: u8) -> u64 {
    ((ENTITY_TAG_ZONE as u64) << ENTITY_TAG_SHIFT)
        | ((zone_id as u64) << ENTITY_ZONE_ID_SHIFT)
        | ((location as u64) << ENTITY_LOCATION_SHIFT)
        | (layer as u64)
}

/// The 2-bit tag of an entity key (`ENTITY_TAG_*`).
pub fn entity_tag(key: u64) -> u8 {
    (key >> ENTITY_TAG_SHIFT) as u8
}

/// True if `key` names an object-shard entity.
pub fn entity_is_object(key: u64) -> bool {
    entity_tag(key) == ENTITY_TAG_OBJECT
}

/// True if `key` names a zone-shard cell.
pub fn entity_is_zone(key: u64) -> bool {
    entity_tag(key) == ENTITY_TAG_ZONE
}

/// The `object_reference` of an OBJECT key — its 62-bit payload (meaningless for other
/// tags). Decompose further with [`object_reference_type`] / [`object_reference_id`].
pub fn entity_object_reference(key: u64) -> u64 {
    key & ENTITY_PAYLOAD_MAX
}

/// The `zone_id` of a ZONE key (meaningless for other tags).
pub fn entity_zone_id(key: u64) -> u32 {
    ((key >> ENTITY_ZONE_ID_SHIFT) & 0xFFFF_FFFF) as u32
}

/// The `location` of a ZONE key (meaningless for other tags).
pub fn entity_location(key: u64) -> u8 {
    ((key >> ENTITY_LOCATION_SHIFT) & 0xFF) as u8
}

/// The `layer` of a ZONE key (meaningless for other tags).
pub fn entity_layer(key: u64) -> u8 {
    (key & 0xFF) as u8
}

// ── object reference (OBJECT payload) ────────────────────────────────────────
//
// An OBJECT entity key's 62-bit payload is an **object_reference**: a class tag plus
// the object's stable, globally-unique **object_id**.
//
// ```text
// object_reference:62 = obj_type:8 << 48 | object_id:48          (bits 56–61 spare)
// object_id:48        = shard_id:16 << 32 | count:32
// ```
//
// The `object_id` is unique across every data shard by construction: `shard_id` is the
// data shard that minted it (unique + permanent), `count` is that shard's monotonic
// per-shard counter — so no two shards, and no single shard twice, ever mint the same
// `(shard_id, count)`. `obj_type` is just a class tag (thing/pawn/demo/player) and plays
// no part in the uniqueness. Position is NOT encoded here — a mobile object keeps its
// `object_id` across every move; only its `state` position changes.

/// Demo object (a moving circle in the Phase-A sync demo).
pub const OBJ_TYPE_DEMO: u8 = 1;

/// A player-controlled object (one per logged-in player). Materialized on the player's
/// first move through the edge.
pub const OBJ_TYPE_PLAYER: u8 = 2;

/// Reserved `shard_id` for objects **not** minted by a data shard (edge/system/ephemeral
/// — e.g. demo & player objects, whose `count` is assigned directly and self-uniquely).
/// Real data shards use ids `≥ 1`.
pub const SHARD_NONE: u16 = 0;

const OBJECT_ID_SHARD_SHIFT: u64 = 32;
const OBJ_TYPE_SHIFT: u64 = 48;
/// Mask for the 48-bit `object_id` within an `object_reference`.
pub const OBJECT_ID_MAX: u64 = (1u64 << 48) - 1;

/// Compose the globally-unique 48-bit `object_id` from the minting `shard_id` and its
/// per-shard `count`.
pub fn pack_object_id(shard_id: u16, count: u32) -> u64 {
    ((shard_id as u64) << OBJECT_ID_SHARD_SHIFT) | count as u64
}

/// The minting `shard_id` of an `object_id`.
pub fn object_id_shard(object_id: u64) -> u16 {
    ((object_id >> OBJECT_ID_SHARD_SHIFT) & 0xFFFF) as u16
}

/// The per-shard `count` of an `object_id`.
pub fn object_id_count(object_id: u64) -> u32 {
    object_id as u32
}

/// Compose an `object_reference` from its class tag and 48-bit `object_id`.
pub fn pack_object_reference(obj_type: u8, object_id: u64) -> u64 {
    ((obj_type as u64) << OBJ_TYPE_SHIFT) | (object_id & OBJECT_ID_MAX)
}

/// The class tag of an `object_reference`.
pub fn object_reference_type(reference: u64) -> u8 {
    ((reference >> OBJ_TYPE_SHIFT) & 0xFF) as u8
}

/// The 48-bit `object_id` of an `object_reference`.
pub fn object_reference_id(reference: u64) -> u64 {
    reference & OBJECT_ID_MAX
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
        // 0xABC (2748) is within the u12 object_id range.
        let p = pack_thing(13, 7, 2, 0x0ABC);
        assert_eq!(thing_x(p), 13);
        assert_eq!(thing_y(p), 7);
        assert_eq!(thing_rotation(p), 2);
        assert_eq!(thing_object_id(p), 0x0ABC);
        assert_eq!(thing_location(p), cell(13, 7));
        // by-location constructor agrees
        assert_eq!(pack_thing_at(cell(13, 7), 2, 0x0ABC), p);
        // u12 cap: the max object_id round-trips, nothing above it fits.
        assert_eq!(thing_object_id(pack_thing(0, 0, 0, DEF_ID_MAX)), DEF_ID_MAX);
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
    fn tile_roundtrips() {
        let slot = pack_tile(0x0ABC, 0x5);
        assert_eq!(tile_def(slot), 0x0ABC);
        assert_eq!(tile_reserved(slot), 0x5);
        // def and reserved occupy disjoint bits — the u12 max + full reserved nibble.
        let full = pack_tile(DEF_ID_MAX, 0xF);
        assert_eq!(tile_def(full), DEF_ID_MAX);
        assert_eq!(tile_reserved(full), 0xF);
        assert_eq!(full, 0xFFFF);
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

    #[test]
    fn object_key_roundtrips() {
        for &r in &[1u64, 2, 42, 0xFFFF, 0xFFFF_FFFF, ENTITY_PAYLOAD_MAX] {
            let k = pack_object_key(r);
            assert_eq!(entity_tag(k), ENTITY_TAG_OBJECT);
            assert!(entity_is_object(k) && !entity_is_zone(k));
            assert_eq!(entity_object_reference(k), r);
        }
    }

    #[test]
    fn zone_key_roundtrips() {
        let z = pack_zone_id(0xAB, 0xCD, 0x02, 13, 7);
        for loc in [0u8, 1, 128, 255] {
            for layer in [LAYER_FLOOR, LAYER_WALL, LAYER_THING, 200] {
                let k = pack_zone_key(z, loc, layer);
                assert_eq!(entity_tag(k), ENTITY_TAG_ZONE);
                assert!(entity_is_zone(k) && !entity_is_object(k));
                assert_eq!(entity_zone_id(k), z);
                assert_eq!(entity_location(k), loc);
                assert_eq!(entity_layer(k), layer);
            }
        }
    }

    #[test]
    fn layers_stack_on_one_tile() {
        // Multiple objects on the same tile — distinct keys per layer.
        let z = pack_zone_id(1, 1, 0, 5, 3);
        let floor = pack_zone_key(z, 42, LAYER_FLOOR);
        let wall = pack_zone_key(z, 42, LAYER_WALL);
        let thing = pack_zone_key(z, 42, LAYER_THING);
        assert_ne!(floor, wall);
        assert_ne!(wall, thing);
        assert_ne!(floor, thing);
        // …all at the same cell.
        assert_eq!(entity_location(floor), entity_location(thing));
        assert_eq!(entity_zone_id(floor), entity_zone_id(thing));
    }

    #[test]
    fn object_id_and_reference_roundtrip() {
        // object_id = (shard_id, count), globally unique.
        let oid = pack_object_id(7, 12345);
        assert_eq!(object_id_shard(oid), 7);
        assert_eq!(object_id_count(oid), 12345);
        // object_reference = obj_type + object_id.
        let r = pack_object_reference(OBJ_TYPE_DEMO, oid);
        assert_eq!(object_reference_type(r), OBJ_TYPE_DEMO);
        assert_eq!(object_reference_id(r), oid);
        // Survives the round-trip through an entity key.
        let k = pack_object_key(r);
        let back = entity_object_reference(k);
        assert_eq!(object_reference_type(back), OBJ_TYPE_DEMO);
        assert_eq!(object_id_shard(object_reference_id(back)), 7);
        assert_eq!(object_id_count(object_reference_id(back)), 12345);
    }

    #[test]
    fn object_ids_are_globally_unique_by_construction() {
        // Different shards, same count → different ids. Same shard, different count →
        // different ids. So no two shards, and no shard twice, collide.
        assert_ne!(pack_object_id(1, 100), pack_object_id(2, 100));
        assert_ne!(pack_object_id(1, 100), pack_object_id(1, 101));
        // Max shard + max count fits the 48-bit object_id.
        assert!(pack_object_id(u16::MAX, u32::MAX) <= OBJECT_ID_MAX);
    }

    #[test]
    fn object_and_zone_key_spaces_are_disjoint() {
        // Same numeric payload under different tags must not collide — this is what
        // makes the priority total order span both classes.
        let obj = pack_object_key(0x1234);
        let zone = pack_zone_key(0x12, 0x34, 0); // packs to a different payload anyway
        assert_ne!(obj, zone);
        assert_ne!(entity_tag(obj), entity_tag(zone));
        // A zone key never reads as an object key with the same low bits.
        let z = pack_zone_id(0, 0, 0, 0, 0);
        assert_ne!(pack_zone_key(z, 5, 0), pack_object_key(5));
    }
}
