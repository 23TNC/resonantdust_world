//! The object-model reference layouts — the packed identity + addressing scheme
//! from `docs/object-model.md`.
//!
//! This is the **new** reference layer for the 0.2.3 pipeline/shard redesign. It
//! is added **alongside** [`refs`](crate::refs) (whose `entity_reference` /
//! `server_reference` / `action_reference` it supersedes) and
//! [`packed`](crate::packed) during migration; consumers move over, then the old
//! layers are removed. Like the rest of the codec it is **pure integer math** —
//! no naming, no I/O. The `type_id ↔ name` / `kind_id ↔ name` mapping is the
//! *registry's* job (content-derived, append-stable); this module only packs and
//! unpacks whatever numbers it is handed.
//!
//! ```text
//! object_reference : u64 = object_type_reference:32 | object_kind_reference:32
//!   object_type_reference : u32 = type_id:4  | subtype_id:12 | layer:4      | reserved:12
//!   object_kind_reference : u32 = kind_id:10 | subkind_id:4  | variant_id:4 | x:4 | y:4 | data:6
//!
//! spatial (each u8 = hi<<4 | lo, two u4 grid coords):
//!   position_reference  = x:4        | y:4
//!   zone_reference      = zone_x:4   | zone_y:4
//!   region_reference    = region_x:4 | region_y:4
//!   realm_reference     = realm_x:4  | realm_y:4
//!   region_zone_reference : u16 = region_reference:8 | zone_reference:8
//!
//! cold_reference : u32 = region_reference:8 | zone_reference:8 | position_reference:8 | layer_id:4 | type_id:4
//! ```
//!
//! `data` is a **type-decoded** u6 payload (see [`data_default`] / [`data_stack`]):
//! most types read it as `rotation:2 | count:4`; a dedicated stackable/resource
//! type reads the whole u6 as `count`. The decode is a function of `type_id` — a
//! caller that knows the type picks the decoder.
//!
//! `type_id == 0` is reserved (the null/unset sentinel); callers assign real types
//! from `1..=15` via the registry.

// ── object_type_reference ─────────────────────────────────────────────────────
//
// u32 = type_id:4 | subtype_id:12 | layer:4 | reserved:12. The "shared" half of an
// object_reference — many objects of the same (type, subtype, layer) share it, which
// is what the cold store's `Vec<object_kind_reference>` per row exploits.

const TYPE_ID_SHIFT: u32 = 28;
const SUBTYPE_ID_SHIFT: u32 = 16;
const TYPE_LAYER_SHIFT: u32 = 12;
const TYPE_ID_MASK: u32 = 0xF; // 4 bits
const SUBTYPE_ID_MASK: u32 = 0xFFF; // 12 bits
const LAYER_MASK: u32 = 0xF; // 4 bits

/// Reserved "no type" id — the null/unset sentinel. Real types are `1..=15`.
pub const TYPE_NONE: u8 = 0;

// The `type_id` palette — structural (each type implies a pipeline + a `data`
// decode), so they live in code, not content (types are few and fixed; kinds are
// content-derived). APPEND-ONLY: a new type goes last so stored zones never
// renumber. These are the go-forward ids, superseding the `ENTITY_TYPE_*` /
// `DATA_TYPE_*` palettes in [`refs`](crate::refs) as consumers migrate.
/// Biome-classified ground tiles — `subtype = biome`.
pub const TYPE_BIOME_TILE: u8 = 1;
/// Biome-scattered things (flora, rocks) — `subtype = biome`.
pub const TYPE_BIOME_THING: u8 = 2;
/// A mobile agent — `subtype = species` (animal / human).
pub const TYPE_PAWN: u8 = 3;
/// A player-controlled entity.
pub const TYPE_PLAYER: u8 = 4;
/// An event-log entry — the generalized object log.
pub const TYPE_EVENT: u8 = 5;
/// A server / shard (provenance).
pub const TYPE_SERVER: u8 = 6;

/// Compose an `object_type_reference`: `type_id:4 | subtype_id:12 | layer:4 | reserved:12`.
/// The low 12 reserved bits stay 0 (growth room — see `docs/object-model.md`).
pub fn pack_type_reference(type_id: u8, subtype_id: u16, layer: u8) -> u32 {
    (((type_id as u32) & TYPE_ID_MASK) << TYPE_ID_SHIFT)
        | (((subtype_id as u32) & SUBTYPE_ID_MASK) << SUBTYPE_ID_SHIFT)
        | (((layer as u32) & LAYER_MASK) << TYPE_LAYER_SHIFT)
}

/// The `type_id` (0..16) of an `object_type_reference`.
pub fn type_ref_type_id(r: u32) -> u8 {
    ((r >> TYPE_ID_SHIFT) & TYPE_ID_MASK) as u8
}

/// The `subtype_id` (0..4096) of an `object_type_reference`.
pub fn type_ref_subtype_id(r: u32) -> u16 {
    ((r >> SUBTYPE_ID_SHIFT) & SUBTYPE_ID_MASK) as u16
}

/// The `layer` (0..16 — the tile object-slot, NOT the texture `part`) of an
/// `object_type_reference`.
pub fn type_ref_layer(r: u32) -> u8 {
    ((r >> TYPE_LAYER_SHIFT) & LAYER_MASK) as u8
}

// ── object_kind_reference ─────────────────────────────────────────────────────
//
// u32 = kind_id:10 | subkind_id:4 | variant_id:4 | x:4 | y:4 | data:6. The
// "per-instance" half — its own kind/subkind/variant, where it sits in the zone
// (x/y = position_reference), and a type-decoded data payload.

const KIND_ID_SHIFT: u32 = 22;
const SUBKIND_ID_SHIFT: u32 = 18;
const VARIANT_ID_SHIFT: u32 = 14;
const KIND_X_SHIFT: u32 = 10;
const KIND_Y_SHIFT: u32 = 6;
const KIND_ID_MASK: u32 = 0x3FF; // 10 bits
const SUBKIND_ID_MASK: u32 = 0xF; // 4 bits
const VARIANT_ID_MASK: u32 = 0xF; // 4 bits
const NIBBLE_MASK: u32 = 0xF; // 4 bits (x, y)
const DATA_MASK: u32 = 0x3F; // 6 bits

/// Compose an `object_kind_reference`: `kind_id:10 | subkind_id:4 | variant_id:4 |
/// x:4 | y:4 | data:6`. `x`/`y` are the object's tile within its zone (the
/// `position_reference`); `data` is the type-decoded payload
/// ([`data_default`] / [`data_stack`]).
pub fn pack_kind_reference(
    kind_id: u16,
    subkind_id: u8,
    variant_id: u8,
    x: u8,
    y: u8,
    data: u8,
) -> u32 {
    (((kind_id as u32) & KIND_ID_MASK) << KIND_ID_SHIFT)
        | (((subkind_id as u32) & SUBKIND_ID_MASK) << SUBKIND_ID_SHIFT)
        | (((variant_id as u32) & VARIANT_ID_MASK) << VARIANT_ID_SHIFT)
        | (((x as u32) & NIBBLE_MASK) << KIND_X_SHIFT)
        | (((y as u32) & NIBBLE_MASK) << KIND_Y_SHIFT)
        | ((data as u32) & DATA_MASK)
}

/// The `kind_id` (0..1024) of an `object_kind_reference`.
pub fn kind_ref_kind_id(r: u32) -> u16 {
    ((r >> KIND_ID_SHIFT) & KIND_ID_MASK) as u16
}

/// The `subkind_id` (0..16) of an `object_kind_reference`.
pub fn kind_ref_subkind_id(r: u32) -> u8 {
    ((r >> SUBKIND_ID_SHIFT) & SUBKIND_ID_MASK) as u8
}

/// The `variant_id` (0..16) of an `object_kind_reference`.
pub fn kind_ref_variant_id(r: u32) -> u8 {
    ((r >> VARIANT_ID_SHIFT) & VARIANT_ID_MASK) as u8
}

/// The `x` tile coordinate (0..16) within the zone.
pub fn kind_ref_x(r: u32) -> u8 {
    ((r >> KIND_X_SHIFT) & NIBBLE_MASK) as u8
}

/// The `y` tile coordinate (0..16) within the zone.
pub fn kind_ref_y(r: u32) -> u8 {
    ((r >> KIND_Y_SHIFT) & NIBBLE_MASK) as u8
}

/// The raw type-decoded `data` payload (0..64). Decode with [`data_default`] or
/// [`data_stack`] per the object's `type_id`.
pub fn kind_ref_data(r: u32) -> u8 {
    (r & DATA_MASK) as u8
}

/// The `position_reference` (`x:4 | y:4`) of a kind reference — the same two
/// nibbles [`kind_ref_x`]/[`kind_ref_y`] read, packed as the u8 spatial ref.
pub fn kind_ref_position(r: u32) -> u8 {
    pack_position_reference(kind_ref_x(r), kind_ref_y(r))
}

// ── object_reference ──────────────────────────────────────────────────────────
//
// u64 = object_type_reference:32 (high) | object_kind_reference:32 (low).

const OBJECT_TYPE_SHIFT: u64 = 32;

/// Compose an `object_reference` from its two halves.
pub fn pack_object_reference(type_reference: u32, kind_reference: u32) -> u64 {
    ((type_reference as u64) << OBJECT_TYPE_SHIFT) | (kind_reference as u64)
}

/// The `object_type_reference` (high 32) of an `object_reference`.
pub fn object_ref_type_reference(r: u64) -> u32 {
    (r >> OBJECT_TYPE_SHIFT) as u32
}

/// The `object_kind_reference` (low 32) of an `object_reference`.
pub fn object_ref_kind_reference(r: u64) -> u32 {
    r as u32
}

// ── data payload (type-decoded u6) ────────────────────────────────────────────
//
// The default decode most types use: rotation:2 | count:4. A dedicated stackable
// type instead reads the whole u6 as a count ([`data_stack`]).

const DATA_ROTATION_SHIFT: u8 = 4;
const DATA_ROTATION_MASK: u8 = 0x3; // 2 bits
const DATA_COUNT4_MASK: u8 = 0xF; // 4 bits (default decode)
const DATA_COUNT6_MASK: u8 = 0x3F; // 6 bits (stackable decode)

/// Default `data` decode: `rotation:2 | count:4` → `(rotation, count)`.
/// `rotation` is 0..4 (facings; west is mirrored east), `count` is a magnitude 0..16.
pub fn data_default(data: u8) -> (u8, u8) {
    let rotation = (data >> DATA_ROTATION_SHIFT) & DATA_ROTATION_MASK;
    let count = data & DATA_COUNT4_MASK;
    (rotation, count)
}

/// Compose the default `data`: `rotation:2 | count:4`.
pub fn pack_data_default(rotation: u8, count: u8) -> u8 {
    ((rotation & DATA_ROTATION_MASK) << DATA_ROTATION_SHIFT) | (count & DATA_COUNT4_MASK)
}

/// Stackable `data` decode: the whole u6 is a `count` (0..64) — for a dedicated
/// stackable/resource type that does not rotate.
pub fn data_stack(data: u8) -> u8 {
    data & DATA_COUNT6_MASK
}

/// Compose a stackable `data`: `count:6` (masked to 0..64).
pub fn pack_data_stack(count: u8) -> u8 {
    count & DATA_COUNT6_MASK
}

// ── spatial references ────────────────────────────────────────────────────────
//
// Four nested 16×16 grids. Each is a u8 = hi_nibble<<4 | lo_nibble (two u4 coords).

const NIBBLE_SHIFT: u8 = 4;
const NIBBLE_U8_MASK: u8 = 0xF;

/// Pack a spatial `u8` reference from two `u4` grid coordinates: `hi:4 | lo:4`.
/// The primitive behind position / zone / region / realm references.
pub fn pack_position_reference(x: u8, y: u8) -> u8 {
    ((x & NIBBLE_U8_MASK) << NIBBLE_SHIFT) | (y & NIBBLE_U8_MASK)
}

/// The high-nibble coordinate (`x` / `zone_x` / `region_x` / `realm_x`).
pub fn ref_hi(r: u8) -> u8 {
    (r >> NIBBLE_SHIFT) & NIBBLE_U8_MASK
}

/// The low-nibble coordinate (`y` / `zone_y` / `region_y` / `realm_y`).
pub fn ref_lo(r: u8) -> u8 {
    r & NIBBLE_U8_MASK
}

const REGION_ZONE_REGION_SHIFT: u16 = 8;
const REGION_ZONE_LOW_MASK: u16 = 0xFF;

/// Pack a `region_zone_reference`: `region_reference:8 | zone_reference:8`. Stored
/// in cold rows so a subscription names region+zone directly, without a filter.
pub fn pack_region_zone(region_reference: u8, zone_reference: u8) -> u16 {
    ((region_reference as u16) << REGION_ZONE_REGION_SHIFT) | (zone_reference as u16)
}

/// The `region_reference` of a `region_zone_reference`.
pub fn region_zone_region(r: u16) -> u8 {
    (r >> REGION_ZONE_REGION_SHIFT) as u8
}

/// The `zone_reference` of a `region_zone_reference`.
pub fn region_zone_zone(r: u16) -> u8 {
    (r & REGION_ZONE_LOW_MASK) as u8
}

// ── cold_reference ────────────────────────────────────────────────────────────
//
// u32 = region_reference:8 | zone_reference:8 | position_reference:8 | layer_id:4 |
// type_id:4. Addresses a settled cold object; subtype-agnostic (the uniqueness rule
// is one object per (type, layer, tile)). Pair with a cold_server_id:u8 to name the
// shard within the realm.

const COLD_REGION_SHIFT: u32 = 24;
const COLD_ZONE_SHIFT: u32 = 16;
const COLD_POSITION_SHIFT: u32 = 8;
const COLD_LAYER_SHIFT: u32 = 4;
const COLD_BYTE_MASK: u32 = 0xFF;
const COLD_NIBBLE_MASK: u32 = 0xF;

/// Compose a `cold_reference`: `region:8 | zone:8 | position:8 | layer:4 | type:4`.
pub fn pack_cold_reference(
    region_reference: u8,
    zone_reference: u8,
    position_reference: u8,
    layer_id: u8,
    type_id: u8,
) -> u32 {
    ((region_reference as u32) << COLD_REGION_SHIFT)
        | ((zone_reference as u32) << COLD_ZONE_SHIFT)
        | ((position_reference as u32) << COLD_POSITION_SHIFT)
        | (((layer_id as u32) & COLD_NIBBLE_MASK) << COLD_LAYER_SHIFT)
        | ((type_id as u32) & COLD_NIBBLE_MASK)
}

/// The `region_reference` of a `cold_reference`.
pub fn cold_ref_region(r: u32) -> u8 {
    ((r >> COLD_REGION_SHIFT) & COLD_BYTE_MASK) as u8
}

/// The `zone_reference` of a `cold_reference`.
pub fn cold_ref_zone(r: u32) -> u8 {
    ((r >> COLD_ZONE_SHIFT) & COLD_BYTE_MASK) as u8
}

/// The `position_reference` of a `cold_reference`.
pub fn cold_ref_position(r: u32) -> u8 {
    ((r >> COLD_POSITION_SHIFT) & COLD_BYTE_MASK) as u8
}

/// The `layer_id` of a `cold_reference`.
pub fn cold_ref_layer(r: u32) -> u8 {
    ((r >> COLD_LAYER_SHIFT) & COLD_NIBBLE_MASK) as u8
}

/// The `type_id` of a `cold_reference`.
pub fn cold_ref_type_id(r: u32) -> u8 {
    (r & COLD_NIBBLE_MASK) as u8
}

/// The `region_zone_reference` of a `cold_reference` — its region+zone as the u16
/// subscription key (drops position/layer/type).
pub fn cold_ref_region_zone(r: u32) -> u16 {
    pack_region_zone(cold_ref_region(r), cold_ref_zone(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_id_palette_fits_u4_and_is_contiguous() {
        let all = [
            TYPE_NONE,
            TYPE_BIOME_TILE,
            TYPE_BIOME_THING,
            TYPE_PAWN,
            TYPE_PLAYER,
            TYPE_EVENT,
            TYPE_SERVER,
        ];
        for (i, &t) in all.iter().enumerate() {
            assert_eq!(t as usize, i, "append-only, contiguous from 0");
            assert!(t <= 0xF, "fits u4");
            assert_eq!(type_ref_type_id(pack_type_reference(t, 0, 0)), t);
        }
    }

    #[test]
    fn type_reference_roundtrips() {
        for &(t, s, l) in &[(0u8, 0u16, 0u8), (1, 1, 1), (7, 2048, 5), (0xF, 0xFFF, 0xF)] {
            let r = pack_type_reference(t, s, l);
            assert_eq!(type_ref_type_id(r), t);
            assert_eq!(type_ref_subtype_id(r), s);
            assert_eq!(type_ref_layer(r), l);
        }
        // all-ones per field, disjoint; the low 12 reserved bits stay 0.
        assert_eq!(pack_type_reference(0xF, 0xFFF, 0xF), 0xFFFF_F000);
    }

    #[test]
    fn kind_reference_roundtrips() {
        for &(k, sk, v, x, y, d) in &[
            (0u16, 0u8, 0u8, 0u8, 0u8, 0u8),
            (1, 1, 1, 1, 1, 1),
            (512, 8, 8, 3, 12, 42),
            (0x3FF, 0xF, 0xF, 0xF, 0xF, 0x3F),
        ] {
            let r = pack_kind_reference(k, sk, v, x, y, d);
            assert_eq!(kind_ref_kind_id(r), k);
            assert_eq!(kind_ref_subkind_id(r), sk);
            assert_eq!(kind_ref_variant_id(r), v);
            assert_eq!(kind_ref_x(r), x);
            assert_eq!(kind_ref_y(r), y);
            assert_eq!(kind_ref_data(r), d);
        }
        // all-ones per field, disjoint → the whole u32 is set.
        assert_eq!(pack_kind_reference(0x3FF, 0xF, 0xF, 0xF, 0xF, 0x3F), u32::MAX);
    }

    #[test]
    fn object_reference_composes_the_two_halves() {
        let t = pack_type_reference(3, 7, 2);
        let k = pack_kind_reference(100, 1, 4, 5, 6, 9);
        let o = pack_object_reference(t, k);
        assert_eq!(object_ref_type_reference(o), t);
        assert_eq!(object_ref_kind_reference(o), k);
        // high 32 = type half, low 32 = kind half.
        assert_eq!(o, ((t as u64) << 32) | k as u64);
    }

    #[test]
    fn kind_position_matches_x_y() {
        let r = pack_kind_reference(0, 0, 0, 0xA, 0x5, 0);
        assert_eq!(kind_ref_position(r), pack_position_reference(0xA, 0x5));
        assert_eq!(ref_hi(kind_ref_position(r)), 0xA);
        assert_eq!(ref_lo(kind_ref_position(r)), 0x5);
    }

    #[test]
    fn data_default_decode_and_pack() {
        for rot in 0u8..4 {
            for count in 0u8..16 {
                let d = pack_data_default(rot, count);
                assert!(d < 64, "fits in u6");
                assert_eq!(data_default(d), (rot, count));
            }
        }
        // rotation and count occupy disjoint bits: rot=3, count=15 → 0b11_1111.
        assert_eq!(pack_data_default(3, 15), 0x3F);
    }

    #[test]
    fn data_stack_uses_all_six_bits() {
        for count in 0u8..64 {
            assert_eq!(data_stack(pack_data_stack(count)), count);
        }
        assert_eq!(pack_data_stack(63), 0x3F);
        // over-range counts are masked to u6, not overflowed.
        assert_eq!(pack_data_stack(64), 0);
    }

    #[test]
    fn spatial_nibbles_roundtrip() {
        for x in 0u8..16 {
            for y in 0u8..16 {
                let r = pack_position_reference(x, y);
                assert_eq!(ref_hi(r), x);
                assert_eq!(ref_lo(r), y);
            }
        }
        assert_eq!(pack_position_reference(0xF, 0xF), 0xFF);
    }

    #[test]
    fn region_zone_composes() {
        let r = pack_region_zone(0xAB, 0xCD);
        assert_eq!(region_zone_region(r), 0xAB);
        assert_eq!(region_zone_zone(r), 0xCD);
        assert_eq!(r, 0xABCD);
    }

    #[test]
    fn cold_reference_roundtrips() {
        for &(reg, z, p, l, t) in &[
            (0u8, 0u8, 0u8, 0u8, 0u8),
            (1, 2, 3, 4, 5),
            (0xAB, 0xCD, 0xEF, 0xF, 0xF),
        ] {
            let r = pack_cold_reference(reg, z, p, l, t);
            assert_eq!(cold_ref_region(r), reg);
            assert_eq!(cold_ref_zone(r), z);
            assert_eq!(cold_ref_position(r), p);
            assert_eq!(cold_ref_layer(r), l);
            assert_eq!(cold_ref_type_id(r), t);
        }
        // all-ones per field, disjoint → whole u32 set.
        assert_eq!(pack_cold_reference(0xFF, 0xFF, 0xFF, 0xF, 0xF), u32::MAX);
    }

    #[test]
    fn cold_reference_region_zone_is_the_subscription_key() {
        let r = pack_cold_reference(0x12, 0x34, 0x56, 7, 8);
        // the u16 region_zone key drops position/layer/type.
        assert_eq!(cold_ref_region_zone(r), pack_region_zone(0x12, 0x34));
        assert_eq!(cold_ref_region_zone(r), 0x1234);
    }
}
