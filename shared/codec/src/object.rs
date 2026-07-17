//! The object-model reference layouts — the **definition / position / data** split of the 0.2.3
//! reference model. Authoritative shape: `docs/VARIABLES.md`.
//!
//! Three orthogonal references describe an object — *what* it is, *where* it is, its *state*.
//! *Which* one it is, is [`crate::refs`]'s job (`entity_reference`), not this module's:
//!
//! ```text
//! definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4   (WHAT)
//!   type_reference : u16 = type_id:4 | subtype_id:12      (shareable — the type half)
//!   kind_reference : u16 = kind_id:12 | variant_id:4      (the kind half)
//!
//! tile_reference : u8 = x:4 | y:4   (one primitive at four nested 16×16 scales:
//!                                    realm/region/zone/tile — hence realm/region/zone reuse it)
//!
//! position_reference : u32 = macro_position_reference:16 | micro_position_reference:16   (WHERE)
//!   macro_position_reference : u16 = region:8 | zone:8    (the zone-subscription key)
//!   micro_position_reference : u16 = tile_reference:8 | layer_reference:8
//!     layer_reference : u8 = type_id:4 | layer_id:4
//!
//! cold_row_reference : u64 = reserved:28 | macro_position:16 | type_reference:16 | layer_id:4
//! kind_pos_reference : u32 = kind_reference:16 | tile_reference:8 | data:8  (one per cold object)
//! data : u8 = rotation:2 | count:6                        (per-instance state; decoded by type_id)
//! ```
//!
//! **A position is not an identity.** `position_reference` used to double as `cold_reference` — a
//! second type over the same 32 bits, meaning "the settled object here, unpack it" — so an object
//! could be addressed by its coordinates. That's gone: identity is an `entity_reference`
//! (`server_reference:8 | object_reference:24`), and geography doesn't fit in 24 bits nor belong
//! there. A `cold_row_reference` addresses neither a place nor an object — it addresses the **row**
//! that holds one, and is the composite of exactly the three header fields a row has.
//!
//! `realm` is **not** in a position — a cold object lives in a realm-scoped shard, so realm rides
//! the `realm_server_reference` ([`crate::refs`]) on cross-realm traffic only. Pure integer math —
//! the `type_id ↔ name` / `kind_id ↔ name` mapping is the registry's (content-derived) job.
//!
//! `type_id == 0` is the null/unset sentinel; real types are `1..=15`.

// ── definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4 ──────────

/// The split between a `definition_reference`'s two `u16` halves.
const DEF_HALF_SHIFT: u32 = 16;
const TYPE_ID_MASK: u16 = 0xF; // 4 bits
const SUBTYPE_ID_MASK: u16 = 0xFFF; // 12 bits
const KIND_ID_MASK: u16 = 0xFFF; // 12 bits
const VARIANT_ID_MASK: u16 = 0xF; // 4 bits

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
/// A biome-invariant thing — an **item**: a resource or a piece of equipment whose appearance does
/// **not** change with biome. The counterpart to [`TYPE_BIOME_THING`] (biome-varying scenery like
/// trees); `subtype` classifies the item family (resource / equipment / …) rather than a biome.
pub const TYPE_THING: u8 = 7;

/// Compose a `definition_reference` from its two `u16` halves: `type_reference | kind_reference`.
/// This is the plan's shape — the halves *are* the definition, so it composes rather than
/// re-deriving from four ids (and it's why `type_reference` must be a `u16`: a `u32` cannot be
/// half of a `u32`).
pub fn pack_definition_reference(type_reference: u16, kind_reference: u16) -> u32 {
    ((type_reference as u32) << DEF_HALF_SHIFT) | (kind_reference as u32)
}

/// The `type_reference` (high half) of a `definition_reference`.
pub fn def_type_reference(r: u32) -> u16 {
    (r >> DEF_HALF_SHIFT) as u16
}

/// The `kind_reference` (low half) of a `definition_reference`.
pub fn def_kind_reference(r: u32) -> u16 {
    r as u16
}

/// Compose a `definition_reference` straight from its four ids — the convenience form of
/// [`pack_definition_reference`] over [`pack_type_reference`] + [`pack_kind_reference`].
pub fn pack_definition_from_ids(type_id: u8, subtype_id: u16, kind_id: u16, variant_id: u8) -> u32 {
    pack_definition_reference(
        pack_type_reference(type_id, subtype_id),
        pack_kind_reference(kind_id, variant_id),
    )
}

/// The `type_id` (0..16) of a `definition_reference`.
pub fn def_type_id(r: u32) -> u8 {
    type_ref_type_id(def_type_reference(r))
}
/// The `subtype_id` (0..4096) of a `definition_reference`.
pub fn def_subtype_id(r: u32) -> u16 {
    type_ref_subtype_id(def_type_reference(r))
}
/// The `kind_id` (0..4096) of a `definition_reference`.
pub fn def_kind_id(r: u32) -> u16 {
    kind_ref_kind_id(def_kind_reference(r))
}
/// The `variant_id` (0..16) of a `definition_reference`.
pub fn def_variant_id(r: u32) -> u8 {
    kind_ref_variant_id(def_kind_reference(r))
}

// ── type_reference : u16 = type_id:4 | subtype_id:12 (the shareable type half) ──────────────────
//
// The cold row's shared header field: many objects share `(type, subtype)`. **`u16` is
// load-bearing** — it is the high half of `definition_reference:u32` and a field of
// `cold_row_reference:u64`; a `u32` cannot be half of a `u32`.

const TYPE_REF_TYPE_SHIFT: u16 = 12;

/// Compose a `type_reference`: `type_id:4 | subtype_id:12`.
pub fn pack_type_reference(type_id: u8, subtype_id: u16) -> u16 {
    (((type_id as u16) & TYPE_ID_MASK) << TYPE_REF_TYPE_SHIFT) | (subtype_id & SUBTYPE_ID_MASK)
}
/// The `type_id` of a `type_reference`.
pub fn type_ref_type_id(r: u16) -> u8 {
    ((r >> TYPE_REF_TYPE_SHIFT) & TYPE_ID_MASK) as u8
}
/// The `subtype_id` of a `type_reference`.
pub fn type_ref_subtype_id(r: u16) -> u16 {
    r & SUBTYPE_ID_MASK
}

// ── kind_reference : u16 = kind_id:12 | variant_id:4 (the kind half of a definition) ────────────

const KIND_REF_KIND_SHIFT: u16 = 4;

/// Compose a `kind_reference`: `kind_id:12 | variant_id:4`.
pub fn pack_kind_reference(kind_id: u16, variant_id: u8) -> u16 {
    ((kind_id & KIND_ID_MASK) << KIND_REF_KIND_SHIFT) | ((variant_id as u16) & VARIANT_ID_MASK)
}
/// The `kind_id` of a `kind_reference`.
pub fn kind_ref_kind_id(r: u16) -> u16 {
    (r >> KIND_REF_KIND_SHIFT) & KIND_ID_MASK
}
/// The `variant_id` of a `kind_reference`.
pub fn kind_ref_variant_id(r: u16) -> u8 {
    (r & VARIANT_ID_MASK) as u8
}

// ── the spatial u8 primitive (four nested 16×16 grids) ──────────────────────────────────────────
//
// One u8 = hi:4 | lo:4 at four scales: realm / region / zone / tile. The plan's field list names
// the tile-level one `tile_reference`; the same packer serves every level (`docs/…/spatial-
// references.md`). NB `position_reference` is the **u32** (region|zone|tile|layer) — not this.

const NIBBLE_SHIFT: u8 = 4;
const NIBBLE_U8_MASK: u8 = 0xF;

/// Pack a spatial `u8` from two `u4` grid coordinates: `hi:4 | lo:4`. The primitive behind
/// `tile_reference` / `zone_reference` / `region_reference` / `realm_reference`.
pub fn pack_tile_reference(x: u8, y: u8) -> u8 {
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

// ── macro / micro position references (the two u16 halves of a position_reference) ──────────────

const MACRO_REGION_SHIFT: u16 = 8;
const MICRO_TILE_SHIFT: u16 = 8;
const HALF_BYTE_MASK: u16 = 0xFF;

/// Pack a `macro_position_reference`: `region_reference:8 | zone_reference:8` — the **macro half**
/// of a `position_reference`, the zone-subscription key (a client names region + zone directly, no
/// filter), and the macro field of a `cold_row_reference`. Realm is the shard's; tile is per object.
pub fn pack_macro_position(region_reference: u8, zone_reference: u8) -> u16 {
    ((region_reference as u16) << MACRO_REGION_SHIFT) | (zone_reference as u16)
}
/// The `region_reference` of a `macro_position_reference`.
pub fn macro_position_region(r: u16) -> u8 {
    (r >> MACRO_REGION_SHIFT) as u8
}
/// The `zone_reference` of a `macro_position_reference`.
pub fn macro_position_zone(r: u16) -> u8 {
    (r & HALF_BYTE_MASK) as u8
}

// ── world structure — the nested 16×16 grids (docs/VARIABLES.md §World structure) ───────────────
//
// Each geographic level is a `u4` nibble pair (`x:4 | y:4`), so every DIM is 16. A bigger world comes
// from widening a reference (`u8 → u16`, a level `4 → 8` bits, DIM `16 → 256`) or lighting up realms —
// the math reads these constants, so it doesn't change. **Authoritative here** (`packed` re-exports).

/// Tiles per zone edge — a zone is `ZONE_DIM × ZONE_DIM` cells (`tile_reference = tile_x:4 | tile_y:4`).
pub const ZONE_DIM: u8 = 16;
/// Cells per zone (`ZONE_DIM²` = 256) — the dense tile array's length; `tile_reference` indexes it.
pub const ZONE_TILES: usize = (ZONE_DIM as usize) * (ZONE_DIM as usize);
/// Zones per region edge (`zone_reference = zone_x:4 | zone_y:4`).
pub const REGION_DIM: u8 = 16;
/// Regions per realm edge (`region_reference = region_x:4 | region_y:4`).
pub const REALM_DIM: u8 = 16;
/// Tiles per region edge = `REGION_DIM · ZONE_DIM` = 256.
pub const REGION_TILES: i32 = REGION_DIM as i32 * ZONE_DIM as i32;
/// Tiles per realm edge = `REALM_DIM · REGION_TILES`.
pub const REALM_TILES: i32 = REALM_DIM as i32 * REGION_TILES;

/// The **world-tile origin** (top-left cell) of the zone named by a `macro_position_reference` —
/// `region_x·REGION_TILES + zone_x·ZONE_DIM` per axis, straight from the region/zone nibbles. The
/// go-forward replacement for the legacy `zone_id → global_tile` path (realm is 0, unused).
pub fn macro_world_origin(macro_position: u16) -> (i32, i32) {
    let region = macro_position_region(macro_position);
    let zone = macro_position_zone(macro_position);
    let ox = ref_hi(region) as i32 * REGION_TILES + ref_hi(zone) as i32 * ZONE_DIM as i32;
    let oy = ref_lo(region) as i32 * REGION_TILES + ref_lo(zone) as i32 * ZONE_DIM as i32;
    (ox, oy)
}

/// Pack a `micro_position_reference`: `tile_reference:8 | layer_reference:8` — the **micro half**
/// of a `position_reference`.
pub fn pack_micro_position(tile_reference: u8, layer_reference: u8) -> u16 {
    ((tile_reference as u16) << MICRO_TILE_SHIFT) | (layer_reference as u16)
}
/// The `tile_reference` of a `micro_position_reference`.
pub fn micro_position_tile(r: u16) -> u8 {
    (r >> MICRO_TILE_SHIFT) as u8
}
/// The `layer_reference` of a `micro_position_reference`.
pub fn micro_position_layer(r: u16) -> u8 {
    (r & HALF_BYTE_MASK) as u8
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

// ── position_reference : u32 = macro_position_reference:16 | micro_position_reference:16 ─────────
//                            = region:8 | zone:8 | tile:8 | layer_reference:8
//
// **A location, and only that.** It is *not* an identity: an `entity_reference` ([`crate::refs`])
// is `server_reference:8 | object_reference:24`, and 32 bits of geography can't fit in a 24-bit
// object_reference — nor should they. A position says where, a reference says which.
//
// This layout used to double as `cold_reference`, a *second type over the same bits* meaning "the
// settled object here, unpack it". That type is gone: addressing an object by its coordinates
// conflated place with identity, and the tagged union it needed (`reference_id`) went with it.
//
// Realm is omitted — a cold object lives in a realm-scoped shard, so realm rides the
// `realm_server_reference` ([`crate::refs`]) on cross-realm traffic, never per reference.

const POSITION_MACRO_SHIFT: u32 = 16;
const POSITION_BYTE_MASK: u32 = 0xFF;
const POSITION_REGION_SHIFT: u32 = 24;
const POSITION_ZONE_SHIFT: u32 = 16;
const POSITION_TILE_SHIFT: u32 = 8;

/// Compose a `position_reference` from its two `u16` halves — the plan's shape.
pub fn pack_position_reference(macro_position: u16, micro_position: u16) -> u32 {
    ((macro_position as u32) << POSITION_MACRO_SHIFT) | (micro_position as u32)
}
/// Compose a `position_reference` from its four `u8` levels.
pub fn pack_position_from_parts(region: u8, zone: u8, tile: u8, layer_reference: u8) -> u32 {
    pack_position_reference(
        pack_macro_position(region, zone),
        pack_micro_position(tile, layer_reference),
    )
}
/// The `macro_position_reference` (region+zone) of a `position_reference` — its subscription key
/// and the macro field of a [`pack_cold_row_reference`].
pub fn position_macro(r: u32) -> u16 {
    (r >> POSITION_MACRO_SHIFT) as u16
}
/// The `micro_position_reference` (tile+layer) of a `position_reference`.
pub fn position_micro(r: u32) -> u16 {
    r as u16
}
/// The `region_reference` of a `position_reference`.
pub fn position_region(r: u32) -> u8 {
    ((r >> POSITION_REGION_SHIFT) & POSITION_BYTE_MASK) as u8
}
/// The `zone_reference` of a `position_reference`.
pub fn position_zone(r: u32) -> u8 {
    ((r >> POSITION_ZONE_SHIFT) & POSITION_BYTE_MASK) as u8
}
/// The `tile_reference` (`x:4 | y:4`) of a `position_reference`.
pub fn position_tile(r: u32) -> u8 {
    ((r >> POSITION_TILE_SHIFT) & POSITION_BYTE_MASK) as u8
}
/// The `layer_reference` (`type_id:4 | layer_id:4`) of a `position_reference`.
pub fn position_layer_reference(r: u32) -> u8 {
    (r & POSITION_BYTE_MASK) as u8
}

// ── cold_row_reference : u64 = reserved:28 | macro_position:16 | type_reference:16 | layer_id:4 ──
//
// The **cold row's identity** — the composite of exactly the three header fields the plan gives a
// row (`macro_position_reference`, `type_reference`, `layer_id`), not an opaque surrogate. Every
// field a reader needs comes off a `position_reference`/`cold_reference`: macro + `layer_id` +
// `type_id` (the last two from its `layer_reference`). `subtype_id` is deliberately NOT needed to
// *find* a row — uniqueness is one object per `(type, layer, tile)`, **subtype-agnostic**.
//
// Distinct from `cold_reference:u32`, which addresses an **object**; this addresses a **row**.

const COLD_ROW_MACRO_SHIFT: u32 = 16;
const COLD_ROW_LAYER_SHIFT: u32 = 8;
const COLD_ROW_U16_MASK: u32 = 0xFFFF;
const COLD_ROW_U8_MASK: u32 = 0xFF;

/// Compose a `cold_row_reference`: `macro_position:16 | layer_reference:8 | reserved:8`. The server
/// is the module (one cold shard per type per zone), so it's out of the key — a `u32` is unique
/// within a module, mirroring `entity_reference` so the `(row, tic)` slot uid matches hot's.
pub fn pack_cold_row_reference(macro_position: u16, layer_reference: u8) -> u32 {
    ((macro_position as u32) << COLD_ROW_MACRO_SHIFT)
        | ((layer_reference as u32) << COLD_ROW_LAYER_SHIFT)
}
/// The `macro_position_reference` of a `cold_row_reference`.
pub fn cold_row_macro_position(r: u32) -> u16 {
    ((r >> COLD_ROW_MACRO_SHIFT) & COLD_ROW_U16_MASK) as u16
}
/// The `layer_reference` (`type_id:4 | layer_id:4`) of a `cold_row_reference`.
pub fn cold_row_layer_reference(r: u32) -> u8 {
    ((r >> COLD_ROW_LAYER_SHIFT) & COLD_ROW_U8_MASK) as u8
}

/// The `cold_row_reference` that holds the object at `position_reference` — filter a zone's rows on
/// `(macro_position, layer_reference)`, then match `tile_reference` within the row. No subtype: the
/// row header is subtype-agnostic (subtype rides the per-entry `kind_reference`).
pub fn cold_row_of(position_reference: u32) -> u32 {
    pack_cold_row_reference(
        position_macro(position_reference),
        position_layer_reference(position_reference),
    )
}

// ── kind_pos_reference : u32 = kind_reference:16 | tile_reference:8 | data:8 ─────────────────────
//
// The cold row's per-object entry (one per object in its `Vec<u32>`), carrying the object's full
// per-instance delta. The row header supplies the shared `(macro_position, type, subtype,
// layer_id)`. Readers are `kind_pos_ref_*` — they read an **entry**, not a `kind_reference`.

const KPR_KIND_SHIFT: u32 = 16;
const KPR_TILE_SHIFT: u32 = 8;
const KPR_KIND_MASK: u32 = 0xFFFF;
const KPR_BYTE_MASK: u32 = 0xFF;

/// Compose a `kind_pos_reference`: `kind_reference:16 | tile_reference:8 | data:8`.
/// `tile_reference` is the object's cell (`x:4|y:4`) within its zone; `data` its per-instance
/// state ([`pack_data`]).
pub fn pack_kind_pos_reference(kind_reference: u16, tile_reference: u8, data: u8) -> u32 {
    ((kind_reference as u32) << KPR_KIND_SHIFT)
        | ((tile_reference as u32) << KPR_TILE_SHIFT)
        | (data as u32)
}
/// The `kind_reference` (`kind_id:12 | variant_id:4`) of a `kind_pos_reference`.
pub fn kind_pos_ref_kind_reference(k: u32) -> u16 {
    ((k >> KPR_KIND_SHIFT) & KPR_KIND_MASK) as u16
}
/// The `kind_id` (0..4096) of a `kind_pos_reference`.
pub fn kind_pos_ref_kind_id(k: u32) -> u16 {
    kind_ref_kind_id(kind_pos_ref_kind_reference(k))
}
/// The `variant_id` (0..16) of a `kind_pos_reference`.
pub fn kind_pos_ref_variant_id(k: u32) -> u8 {
    kind_ref_variant_id(kind_pos_ref_kind_reference(k))
}
/// The `tile_reference` (`x:4 | y:4`) of a `kind_pos_reference` — what row-selection matches on.
pub fn kind_pos_ref_tile(k: u32) -> u8 {
    ((k >> KPR_TILE_SHIFT) & KPR_BYTE_MASK) as u8
}
/// The `x` tile coordinate (0..16) within the zone.
pub fn kind_pos_ref_x(k: u32) -> u8 {
    ref_hi(kind_pos_ref_tile(k))
}
/// The `y` tile coordinate (0..16) within the zone.
pub fn kind_pos_ref_y(k: u32) -> u8 {
    ref_lo(kind_pos_ref_tile(k))
}
/// The raw `data` byte (0..256) of a `kind_pos_reference` — decode with [`data_rotation`] /
/// [`data_count`].
pub fn kind_pos_ref_data(k: u32) -> u8 {
    (k & KPR_BYTE_MASK) as u8
}

// ── data : u8 = rotation:2 | count:6 ────────────────────────────────────────────────────────────
//
// Per-instance state, decoded by the row's `type_id` (read once per type-homogeneous row). This is
// the default (universal) decode; `type_id` selects it so it can diverge later without a layout
// change. Authoritative layout: `docs/VARIABLES.md`.
//
// Was `sub_position:3 | rotation:2 | aux:3`: `sub_position` (offsets internal to the tile) is gone
// — a cold object sits on its tile — and `aux` became `count`, named for what it holds and widened
// 3→6 bits (max 7 → 63) with the freed bits.

const DATA_ROT_SHIFT: u8 = 6;
const DATA_ROT_MASK: u8 = 0x3; // 2 bits
const DATA_COUNT_MASK: u8 = 0x3F; // 6 bits

/// Compose the default `data`: `rotation:2 | count:6`.
pub fn pack_data(rotation: u8, count: u8) -> u8 {
    ((rotation & DATA_ROT_MASK) << DATA_ROT_SHIFT) | (count & DATA_COUNT_MASK)
}
/// The `rotation` (0..4 facings; west mirrors east).
pub fn data_rotation(d: u8) -> u8 {
    (d >> DATA_ROT_SHIFT) & DATA_ROT_MASK
}
/// The `count` (0..64) — how many of this object sit on the tile.
pub fn data_count(d: u8) -> u8 {
    d & DATA_COUNT_MASK
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
            TYPE_THING,
        ];
        for (i, &t) in all.iter().enumerate() {
            assert_eq!(t as usize, i, "append-only, contiguous from 0");
            assert!(t <= 0xF, "fits u4");
        }
    }

    #[test]
    fn definition_reference_is_its_two_u16_halves() {
        for &(t, s, k, v) in &[(0u8, 0u16, 0u16, 0u8), (1, 1, 1, 1), (7, 2048, 3000, 5), (0xF, 0xFFF, 0xFFF, 0xF)] {
            let r = pack_definition_from_ids(t, s, k, v);
            assert_eq!(def_type_id(r), t);
            assert_eq!(def_subtype_id(r), s);
            assert_eq!(def_kind_id(r), k);
            assert_eq!(def_variant_id(r), v);
            // the plan's shape: definition == type_reference:16 | kind_reference:16
            assert_eq!(def_type_reference(r), pack_type_reference(t, s));
            assert_eq!(def_kind_reference(r), pack_kind_reference(k, v));
            assert_eq!(r, pack_definition_reference(pack_type_reference(t, s), pack_kind_reference(k, v)));
        }
        assert_eq!(pack_definition_from_ids(0xF, 0xFFF, 0xFFF, 0xF), u32::MAX);
    }

    #[test]
    fn type_and_kind_halves_are_u16() {
        let tr: u16 = pack_type_reference(7, 2048);
        assert_eq!(type_ref_type_id(tr), 7);
        assert_eq!(type_ref_subtype_id(tr), 2048);
        assert_eq!(pack_type_reference(0xF, 0xFFF), 0xFFFF);
        let kr: u16 = pack_kind_reference(3000, 5);
        assert_eq!(kind_ref_kind_id(kr), 3000);
        assert_eq!(kind_ref_variant_id(kr), 5);
        assert_eq!(pack_kind_reference(0xFFF, 0xF), 0xFFFF);
    }

    #[test]
    fn kind_pos_reference_roundtrips() {
        let kr = pack_kind_reference(3000, 5);
        for &(x, y, d) in &[(0u8, 0u8, 0u8), (3, 12, 42), (15, 15, 0xFF)] {
            let e = pack_kind_pos_reference(kr, pack_tile_reference(x, y), d);
            assert_eq!(kind_pos_ref_kind_reference(e), kr);
            assert_eq!(kind_pos_ref_kind_id(e), 3000);
            assert_eq!(kind_pos_ref_variant_id(e), 5);
            assert_eq!(kind_pos_ref_x(e), x);
            assert_eq!(kind_pos_ref_y(e), y);
            assert_eq!(kind_pos_ref_data(e), d);
        }
        assert_eq!(pack_kind_pos_reference(0xFFFF, 0xFF, 0xFF), u32::MAX);
    }

    #[test]
    fn spatial_nibbles_and_macro_micro_halves() {
        for x in 0u8..16 {
            for y in 0u8..16 {
                let r = pack_tile_reference(x, y);
                assert_eq!(ref_hi(r), x);
                assert_eq!(ref_lo(r), y);
            }
        }
        let m = pack_macro_position(0xAB, 0xCD);
        assert_eq!(macro_position_region(m), 0xAB);
        assert_eq!(macro_position_zone(m), 0xCD);
        assert_eq!(m, 0xABCD);
        let mi = pack_micro_position(0xEF, 0x35);
        assert_eq!(micro_position_tile(mi), 0xEF);
        assert_eq!(micro_position_layer(mi), 0x35);
    }

    #[test]
    fn position_reference_is_macro_plus_micro_and_cold_shares_it() {
        for &(reg, z, t, l) in &[(0u8, 0u8, 0u8, 0u8), (1, 2, 3, 4), (0xAB, 0xCD, 0xEF, 0x35)] {
            let p = pack_position_from_parts(reg, z, t, l);
            assert_eq!(position_region(p), reg);
            assert_eq!(position_zone(p), z);
            assert_eq!(position_tile(p), t);
            assert_eq!(position_layer_reference(p), l);
            // the plan's shape: position == macro:16 | micro:16
            assert_eq!(position_macro(p), pack_macro_position(reg, z));
            assert_eq!(position_micro(p), pack_micro_position(t, l));
            assert_eq!(p, pack_position_reference(pack_macro_position(reg, z), pack_micro_position(t, l)));
            // cold_reference is the SAME layout, a different *type* (plan: two types, one layout)
            assert_eq!(pack_position_from_parts(reg, z, t, l), p);
            assert_eq!(position_macro(p), position_macro(p));
        }
        assert_eq!(pack_position_from_parts(0xFF, 0xFF, 0xFF, 0xFF), u32::MAX);
        let lr = pack_layer_reference(3, 5);
        assert_eq!(layer_ref_type_id(lr), 3);
        assert_eq!(layer_ref_layer_id(lr), 5);
    }

    #[test]
    fn cold_row_reference_is_the_macro_layer_composite() {
        for &(m, lr) in &[(0u16, 0u8), (0xABCD, pack_layer_reference(2, 3)), (0xFFFF, 0xFF)] {
            let k = pack_cold_row_reference(m, lr);
            assert_eq!(cold_row_macro_position(k), m);
            assert_eq!(cold_row_layer_reference(k), lr);
        }
        // All-ones per field, disjoint; reserved:8 stays 0 → macro | layer occupy the high 24 bits.
        assert_eq!(pack_cold_row_reference(0xFFFF, 0xFF), 0xFFFF_FF00);
        // Distinct by macro_position, and by layer_reference (which folds type_id | layer_id).
        assert_ne!(
            pack_cold_row_reference(0x1234, pack_layer_reference(2, 0)),
            pack_cold_row_reference(0x1235, pack_layer_reference(2, 0)),
        );
        assert_ne!(
            pack_cold_row_reference(0x1234, pack_layer_reference(2, 0)),
            pack_cold_row_reference(0x1234, pack_layer_reference(2, 1)),
        );
    }

    #[test]
    fn cold_row_of_derives_the_row_from_a_position() {
        // A position carries macro + tile + layer_reference; the row key is macro + layer_reference
        // (server = the module, subtype = the per-entry kind — neither is in the key).
        let pos = pack_position_from_parts(0x12, 0x34, pack_tile_reference(7, 7), pack_layer_reference(2, 0));
        let row = cold_row_of(pos);
        assert_eq!(cold_row_macro_position(row), pack_macro_position(0x12, 0x34));
        assert_eq!(cold_row_layer_reference(row), pack_layer_reference(2, 0));
        // Same tile, different layer → a different row. Different tile (same row) → the SAME row.
        let other_layer = pack_position_from_parts(0x12, 0x34, pack_tile_reference(7, 7), pack_layer_reference(2, 1));
        assert_ne!(cold_row_of(other_layer), row);
        let same_row = pack_position_from_parts(0x12, 0x34, pack_tile_reference(3, 9), pack_layer_reference(2, 0));
        assert_eq!(cold_row_of(same_row), row);
    }

    #[test]
    fn macro_world_origin_tiles_continuously() {
        let m = |rx, ry, zx, zy| pack_macro_position(pack_tile_reference(rx, ry), pack_tile_reference(zx, zy));
        // Origin of region(0,0) zone(0,0) is the world origin.
        assert_eq!(macro_world_origin(m(0, 0, 0, 0)), (0, 0));
        // Adjacent zones are exactly ZONE_DIM apart — no gap, no overlap (the seam property).
        let a = macro_world_origin(m(0, 0, 3, 5));
        let b = macro_world_origin(m(0, 0, 4, 5));
        assert_eq!((b.0 - a.0, b.1 - a.1), (ZONE_DIM as i32, 0));
        // ...and the last zone of a region continues into the next region's zone 0.
        let z15 = macro_world_origin(m(0, 0, 15, 0));
        let r1 = macro_world_origin(m(1, 0, 0, 0));
        assert_eq!(r1.0 - z15.0, ZONE_DIM as i32);
        // y axis uses the low nibbles.
        assert_eq!(macro_world_origin(m(0, 2, 0, 7)), (0, 2 * REGION_TILES + 7 * ZONE_DIM as i32));
    }

    #[test]
    fn data_default_decode() {
        for rot in 0u8..4 {
            for count in 0u8..64 {
                let d = pack_data(rot, count);
                assert_eq!(data_rotation(d), rot);
                assert_eq!(data_count(d), count);
            }
        }
        // Both fields saturated fills the byte — proves the two spans are adjacent
        // and exhaust it (no gap, no overlap).
        assert_eq!(pack_data(3, 63), 0xFF);
        // Each field is masked to its own span, so an over-range argument can't
        // bleed into the other.
        assert_eq!(pack_data(0xFF, 0), 0xC0);
        assert_eq!(pack_data(0, 0xFF), 0x3F);
    }
}
