//! The object-model reference layouts — the **definition / position / data** split of the 0.2.3
//! reference model. Canonical shape:
//! `docs/components/shared/codec/design/reference-model.md` (+ `references/spatial-references.md`).
//!
//! Three orthogonal references describe an object — *what* it is, *where* it is, its *state*:
//!
//! ```text
//! definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4   (WHAT)
//!   type_reference : u16 = type_id:4 | subtype_id:12      (shareable — the type half)
//!   kind_reference : u16 = kind_id:12 | variant_id:4      (the kind half)
//!
//! spatial u8 = hi:4 | lo:4   (one primitive at four nested 16×16 scales: realm/region/zone/tile)
//! region_zone_reference : u16 = region:8 | zone:8         (the zone-subscription key)
//! cold_reference : u32 = region:8 | zone:8 | tile:8 | layer_reference:8       (WHERE, realm-scoped)
//!   layer_reference : u8 = type_id:4 | layer_id:4
//!
//! cold entry (kind_pos_reference) : u32 = kind_reference:16 | tile:8 | data:8  (one per cold object)
//! data : u8 = sub_position:3 | rotation:2 | aux:3         (per-instance state; decoded by type_id)
//! ```
//!
//! `realm` is **not** in a `cold_reference` — a cold object lives in a realm-scoped shard, so the
//! realm rides `server_reference` ([`crate::refs`]), not every reference. Pure integer math — the
//! `type_id ↔ name` / `kind_id ↔ name` mapping is the registry's (content-derived) job.
//!
//! `type_id == 0` is the null/unset sentinel; real types are `1..=15`.

// ── definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4 ──────────

const DEF_TYPE_SHIFT: u32 = 28;
const DEF_SUBTYPE_SHIFT: u32 = 16;
const DEF_KIND_SHIFT: u32 = 4;
const TYPE_ID_MASK: u32 = 0xF; // 4 bits
const SUBTYPE_ID_MASK: u32 = 0xFFF; // 12 bits
const KIND_ID_MASK: u32 = 0xFFF; // 12 bits
const VARIANT_ID_MASK: u32 = 0xF; // 4 bits

/// Reserved "no type" id — the null/unset sentinel. Real types are `1..=15`.
pub const TYPE_NONE: u8 = 0;

// The `type_id` palette — structural (each type implies a pipeline + a `data` decode), so they
// live in code, not content (types are few and fixed; kinds are content-derived). APPEND-ONLY: a
// new type goes last so stored zones never renumber.
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

/// Compose a `definition_reference`: `type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4`.
/// Fully shareable — every instance of `pawn/human/male.fat` has the same one.
pub fn pack_definition_reference(type_id: u8, subtype_id: u16, kind_id: u16, variant_id: u8) -> u32 {
    (((type_id as u32) & TYPE_ID_MASK) << DEF_TYPE_SHIFT)
        | (((subtype_id as u32) & SUBTYPE_ID_MASK) << DEF_SUBTYPE_SHIFT)
        | (((kind_id as u32) & KIND_ID_MASK) << DEF_KIND_SHIFT)
        | ((variant_id as u32) & VARIANT_ID_MASK)
}

/// The `type_id` (0..16) of a `definition_reference`.
pub fn def_type_id(r: u32) -> u8 {
    ((r >> DEF_TYPE_SHIFT) & TYPE_ID_MASK) as u8
}
/// The `subtype_id` (0..4096) of a `definition_reference`.
pub fn def_subtype_id(r: u32) -> u16 {
    ((r >> DEF_SUBTYPE_SHIFT) & SUBTYPE_ID_MASK) as u16
}
/// The `kind_id` (0..4096) of a `definition_reference`.
pub fn def_kind_id(r: u32) -> u16 {
    ((r >> DEF_KIND_SHIFT) & KIND_ID_MASK) as u16
}
/// The `variant_id` (0..16) of a `definition_reference`.
pub fn def_variant_id(r: u32) -> u8 {
    (r & VARIANT_ID_MASK) as u8
}

// ── type_reference : u16 = type_id:4 | subtype_id:12 (the shareable type half) ──────────────────
//
// The cold row's shared header field: many objects share `(type, subtype)`. Stored in a `u32`
// column on the wire/table (high 16 bits 0), so these accept a `u32` and mask the low 16.

const TYPE_REF_TYPE_SHIFT: u32 = 12;

/// Compose a `type_reference`: `type_id:4 | subtype_id:12`.
pub fn pack_type_reference(type_id: u8, subtype_id: u16) -> u32 {
    (((type_id as u32) & TYPE_ID_MASK) << TYPE_REF_TYPE_SHIFT) | ((subtype_id as u32) & SUBTYPE_ID_MASK)
}
/// The `type_id` of a `type_reference`.
pub fn type_ref_type_id(r: u32) -> u8 {
    ((r >> TYPE_REF_TYPE_SHIFT) & TYPE_ID_MASK) as u8
}
/// The `subtype_id` of a `type_reference`.
pub fn type_ref_subtype_id(r: u32) -> u16 {
    (r & SUBTYPE_ID_MASK) as u16
}

// ── kind_reference : u16 = kind_id:12 | variant_id:4 (the kind half of a definition) ────────────

const KIND_REF_KIND_SHIFT: u16 = 4;

/// Compose a `kind_reference`: `kind_id:12 | variant_id:4`.
pub fn pack_kind_reference(kind_id: u16, variant_id: u8) -> u16 {
    (((kind_id) & KIND_ID_MASK as u16) << KIND_REF_KIND_SHIFT) | ((variant_id as u16) & VARIANT_ID_MASK as u16)
}

// ── spatial primitive + region_zone (four nested 16×16 grids) ───────────────────────────────────
//
// One u8 = hi:4 | lo:4 at four scales: realm / region / zone / tile. `pack_position_reference`
// packs any level's two nibbles; `ref_hi` / `ref_lo` read them. (`docs/…/spatial-references.md`.)

const NIBBLE_SHIFT: u8 = 4;
const NIBBLE_U8_MASK: u8 = 0xF;

/// Pack a spatial `u8` from two `u4` grid coordinates: `hi:4 | lo:4`. The primitive behind
/// tile / zone / region / realm references.
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

/// Pack a `region_zone_reference`: `region_reference:8 | zone_reference:8`. The zone-subscription
/// key — a client names region + zone directly, no filter. (Realm is the shard's; tile is per
/// object.)
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

// ── layer_reference : u8 = type_id:4 | layer_id:4 ───────────────────────────────────────────────

const LAYER_REF_TYPE_SHIFT: u8 = 4;

/// Compose a `layer_reference`: `type_id:4 | layer_id:4` (the tile-slot, interpreted per type).
pub fn pack_layer_reference(type_id: u8, layer_id: u8) -> u8 {
    ((type_id & NIBBLE_U8_MASK) << LAYER_REF_TYPE_SHIFT) | (layer_id & NIBBLE_U8_MASK)
}
/// The `type_id` of a `layer_reference`.
pub fn layer_ref_type_id(r: u8) -> u8 {
    (r >> LAYER_REF_TYPE_SHIFT) & NIBBLE_U8_MASK
}
/// The `layer_id` of a `layer_reference`.
pub fn layer_ref_layer_id(r: u8) -> u8 {
    r & NIBBLE_U8_MASK
}

// ── cold_reference : u32 = region:8 | zone:8 | tile:8 | layer_reference:8 ────────────────────────
//
// A settled object addressed by its (realm-scoped) location. Realm omitted — carried by
// `server_reference`. This is the `object_reference` a cold `entity_reference` (`REF_COLD`) holds,
// and it `unpack`s to a hot object at that tile.

const COLD_REGION_SHIFT: u32 = 24;
const COLD_ZONE_SHIFT: u32 = 16;
const COLD_TILE_SHIFT: u32 = 8;
const COLD_BYTE_MASK: u32 = 0xFF;

/// Compose a `cold_reference`: `region:8 | zone:8 | tile:8 | layer_reference:8`.
pub fn pack_cold_reference(region: u8, zone: u8, tile: u8, layer_reference: u8) -> u32 {
    ((region as u32) << COLD_REGION_SHIFT)
        | ((zone as u32) << COLD_ZONE_SHIFT)
        | ((tile as u32) << COLD_TILE_SHIFT)
        | (layer_reference as u32)
}
/// The `region_reference` of a `cold_reference`.
pub fn cold_ref_region(r: u32) -> u8 {
    ((r >> COLD_REGION_SHIFT) & COLD_BYTE_MASK) as u8
}
/// The `zone_reference` of a `cold_reference`.
pub fn cold_ref_zone(r: u32) -> u8 {
    ((r >> COLD_ZONE_SHIFT) & COLD_BYTE_MASK) as u8
}
/// The `tile_reference` (`x:4 | y:4`) of a `cold_reference`.
pub fn cold_ref_tile(r: u32) -> u8 {
    ((r >> COLD_TILE_SHIFT) & COLD_BYTE_MASK) as u8
}
/// The `layer_reference` (`type_id:4 | layer_id:4`) of a `cold_reference`.
pub fn cold_ref_layer_reference(r: u32) -> u8 {
    (r & COLD_BYTE_MASK) as u8
}
/// The `region_zone_reference` (region+zone) of a `cold_reference` — its subscription key.
pub fn cold_ref_region_zone(r: u32) -> u16 {
    pack_region_zone(cold_ref_region(r), cold_ref_zone(r))
}

// ── cold entry (kind_pos_reference) : u32 = kind_reference:16 | tile:8 | data:8 ──────────────────
//
// One per cold object in a row's `Vec<u32>`; carries the object's full per-instance delta. The
// row header supplies the shared `(type, subtype, region, zone, layer_id)`. The `kind_ref_*` /
// `entry_*` readers decode an entry; `pack_cold_entry` builds one.

const ENTRY_KIND_SHIFT: u32 = 16;
const ENTRY_TILE_SHIFT: u32 = 8;
const ENTRY_KIND_MASK: u32 = 0xFFFF;
const ENTRY_BYTE_MASK: u32 = 0xFF;

/// Compose a cold entry: `kind_reference:16 | tile:8 | data:8`. `tile` is the object's cell
/// (`x:4|y:4`) within its zone; `data` its per-instance state ([`pack_data`]).
pub fn pack_cold_entry(kind_reference: u16, tile: u8, data: u8) -> u32 {
    ((kind_reference as u32) << ENTRY_KIND_SHIFT) | ((tile as u32) << ENTRY_TILE_SHIFT) | (data as u32)
}
/// The `kind_reference` (`kind_id:12 | variant_id:4`) of a cold entry.
pub fn kind_ref_kind_reference(k: u32) -> u16 {
    ((k >> ENTRY_KIND_SHIFT) & ENTRY_KIND_MASK) as u16
}
/// The `kind_id` (0..4096) of a cold entry.
pub fn kind_ref_kind_id(k: u32) -> u16 {
    (kind_ref_kind_reference(k) >> KIND_REF_KIND_SHIFT) & KIND_ID_MASK as u16
}
/// The `variant_id` (0..16) of a cold entry.
pub fn kind_ref_variant_id(k: u32) -> u8 {
    (kind_ref_kind_reference(k) & VARIANT_ID_MASK as u16) as u8
}
/// The `tile` byte (`x:4 | y:4`) of a cold entry.
pub fn kind_ref_tile(k: u32) -> u8 {
    ((k >> ENTRY_TILE_SHIFT) & ENTRY_BYTE_MASK) as u8
}
/// The `x` tile coordinate (0..16) within the zone.
pub fn kind_ref_x(k: u32) -> u8 {
    ref_hi(kind_ref_tile(k))
}
/// The `y` tile coordinate (0..16) within the zone.
pub fn kind_ref_y(k: u32) -> u8 {
    ref_lo(kind_ref_tile(k))
}
/// The raw `data` byte (0..256) of a cold entry — decode with [`data_sub_position`] etc.
pub fn kind_ref_data(k: u32) -> u8 {
    (k & ENTRY_BYTE_MASK) as u8
}

// ── data : u8 = sub_position:3 | rotation:2 | aux:3 ─────────────────────────────────────────────
//
// Per-instance state, decoded by the row's `type_id` (read once per type-homogeneous row). This is
// the default (universal) decode; `type_id` selects it so it can diverge later without a layout
// change.

const DATA_SUB_SHIFT: u8 = 5;
const DATA_ROT_SHIFT: u8 = 3;
const DATA_TRIPLE_MASK: u8 = 0x7; // 3 bits
const DATA_ROT_MASK: u8 = 0x3; // 2 bits

/// Compose the default `data`: `sub_position:3 | rotation:2 | aux:3`.
pub fn pack_data(sub_position: u8, rotation: u8, aux: u8) -> u8 {
    ((sub_position & DATA_TRIPLE_MASK) << DATA_SUB_SHIFT)
        | ((rotation & DATA_ROT_MASK) << DATA_ROT_SHIFT)
        | (aux & DATA_TRIPLE_MASK)
}
/// The `sub_position` (0..8 offsets internal to the tile).
pub fn data_sub_position(d: u8) -> u8 {
    (d >> DATA_SUB_SHIFT) & DATA_TRIPLE_MASK
}
/// The `rotation` (0..4 facings; west mirrors east).
pub fn data_rotation(d: u8) -> u8 {
    (d >> DATA_ROT_SHIFT) & DATA_ROT_MASK
}
/// The type-decoded `aux` field (0..8 — e.g. a small count).
pub fn data_aux(d: u8) -> u8 {
    d & DATA_TRIPLE_MASK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_id_palette_fits_u4_and_is_contiguous() {
        let all = [TYPE_NONE, TYPE_BIOME_TILE, TYPE_BIOME_THING, TYPE_PAWN, TYPE_PLAYER, TYPE_EVENT, TYPE_SERVER];
        for (i, &t) in all.iter().enumerate() {
            assert_eq!(t as usize, i, "append-only, contiguous from 0");
            assert!(t <= 0xF, "fits u4");
        }
    }

    #[test]
    fn definition_reference_roundtrips() {
        for &(t, s, k, v) in &[(0u8, 0u16, 0u16, 0u8), (1, 1, 1, 1), (7, 2048, 3000, 5), (0xF, 0xFFF, 0xFFF, 0xF)] {
            let r = pack_definition_reference(t, s, k, v);
            assert_eq!(def_type_id(r), t);
            assert_eq!(def_subtype_id(r), s);
            assert_eq!(def_kind_id(r), k);
            assert_eq!(def_variant_id(r), v);
        }
        // all-ones per field, disjoint → the whole u32 is set.
        assert_eq!(pack_definition_reference(0xF, 0xFFF, 0xFFF, 0xF), u32::MAX);
    }

    #[test]
    fn type_and_kind_halves() {
        let tr = pack_type_reference(7, 2048);
        assert_eq!(type_ref_type_id(tr), 7);
        assert_eq!(type_ref_subtype_id(tr), 2048);
        assert_eq!(pack_type_reference(0xF, 0xFFF), 0xFFFF);
        let kr = pack_kind_reference(3000, 5);
        assert_eq!((kr >> 4) & 0xFFF, 3000);
        assert_eq!(kr & 0xF, 5);
    }

    #[test]
    fn cold_entry_roundtrips() {
        let kr = pack_kind_reference(3000, 5);
        for &(x, y, d) in &[(0u8, 0u8, 0u8), (3, 12, 42), (15, 15, 0xFF)] {
            let e = pack_cold_entry(kr, pack_position_reference(x, y), d);
            assert_eq!(kind_ref_kind_reference(e), kr);
            assert_eq!(kind_ref_kind_id(e), 3000);
            assert_eq!(kind_ref_variant_id(e), 5);
            assert_eq!(kind_ref_x(e), x);
            assert_eq!(kind_ref_y(e), y);
            assert_eq!(kind_ref_data(e), d);
        }
        // all-ones per field, disjoint → whole u32 set.
        assert_eq!(pack_cold_entry(0xFFFF, 0xFF, 0xFF), u32::MAX);
    }

    #[test]
    fn spatial_nibbles_and_region_zone() {
        for x in 0u8..16 {
            for y in 0u8..16 {
                let r = pack_position_reference(x, y);
                assert_eq!(ref_hi(r), x);
                assert_eq!(ref_lo(r), y);
            }
        }
        let rz = pack_region_zone(0xAB, 0xCD);
        assert_eq!(region_zone_region(rz), 0xAB);
        assert_eq!(region_zone_zone(rz), 0xCD);
        assert_eq!(rz, 0xABCD);
    }

    #[test]
    fn cold_reference_roundtrips() {
        for &(reg, z, t, l) in &[(0u8, 0u8, 0u8, 0u8), (1, 2, 3, 4), (0xAB, 0xCD, 0xEF, 0x35)] {
            let r = pack_cold_reference(reg, z, t, l);
            assert_eq!(cold_ref_region(r), reg);
            assert_eq!(cold_ref_zone(r), z);
            assert_eq!(cold_ref_tile(r), t);
            assert_eq!(cold_ref_layer_reference(r), l);
            assert_eq!(cold_ref_region_zone(r), pack_region_zone(reg, z));
        }
        assert_eq!(pack_cold_reference(0xFF, 0xFF, 0xFF, 0xFF), u32::MAX);
        // layer_reference splits into type_id | layer_id.
        let lr = pack_layer_reference(3, 5);
        assert_eq!(layer_ref_type_id(lr), 3);
        assert_eq!(layer_ref_layer_id(lr), 5);
    }

    #[test]
    fn data_default_decode() {
        for sp in 0u8..8 {
            for rot in 0u8..4 {
                for aux in 0u8..8 {
                    let d = pack_data(sp, rot, aux);
                    assert_eq!(data_sub_position(d), sp);
                    assert_eq!(data_rotation(d), rot);
                    assert_eq!(data_aux(d), aux);
                }
            }
        }
        assert_eq!(pack_data(7, 3, 7), 0xFF);
    }
}
