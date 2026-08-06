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

// The masks' bounds, named so an allocator can REFUSE at the ceiling instead of wrapping into it
// (definition-registry). Derived from the masks above, so they cannot drift from the layout —
// these add no field and change no layout, they only state what the existing one already permits.
/// One past the largest `kind_id` / `subtype_id` — both u12, so 4096.
pub const KIND_ID_LIMIT: u32 = KIND_ID_MASK as u32 + 1;
/// One past the largest `variant_id` — u4, so **16 variants per (type, subType, kind)**.
pub const VARIANT_ID_LIMIT: u32 = VARIANT_ID_MASK as u32 + 1;

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
/// A GAMEPLAY definition — needs, conditions, traits, interactions, affordances (work
/// `2026-08-06-interactions` F1): `subtype` = the category, `kind` = the def, `variant` its
/// variation (`default` = 0). These ids ride payload words and event inputs, never placements.
pub const TYPE_GAMEPLAY: u8 = 8;

// The gameplay CATEGORY palette — `subtype_id` values under [`TYPE_GAMEPLAY`]. Code-owned like
// the type palette (a category implies a loader schema + an executor, so an unknown one is a
// corpus error, never a number invented on the spot). APPEND-ONLY.
/// `subtype_id` of `gameplay/need`.
pub const GAMEPLAY_NEED: u16 = 1;
/// `subtype_id` of `gameplay/condition`.
pub const GAMEPLAY_CONDITION: u16 = 2;
/// `subtype_id` of `gameplay/trait`.
pub const GAMEPLAY_TRAIT: u16 = 3;
/// `subtype_id` of `gameplay/interaction`.
pub const GAMEPLAY_INTERACTION: u16 = 4;
/// `subtype_id` of `gameplay/affordance`.
pub const GAMEPLAY_AFFORDANCE: u16 = 5;

/// The gameplay category names in `subtype_id` order (index 0 → id 1) — ONE spelling of the
/// palette, shared by the loader's derived taxonomy (interactions F9) and the master's
/// allocator so the two can never drift.
pub const GAMEPLAY_CATEGORIES: [&str; 5] = ["need", "condition", "trait", "interaction", "affordance"];

/// A gameplay category name's `subtype_id`, or `None` for a name outside the palette.
pub fn gameplay_subtype_id(category: &str) -> Option<u16> {
    GAMEPLAY_CATEGORIES.iter().position(|c| *c == category).map(|i| i as u16 + 1)
}

/// The reverse: a `subtype_id`'s gameplay category name.
pub fn gameplay_category(subtype_id: u16) -> Option<&'static str> {
    (subtype_id != 0).then(|| GAMEPLAY_CATEGORIES.get(subtype_id as usize - 1).copied()).flatten()
}

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
/// A `definition_reference` with its variant nibble zeroed — the IDENTITY compare for "the
/// same type/subtype/kind, any variant" (e.g. the npc adoption filter: a wolf is a wolf
/// whichever coat it wears).
pub fn def_sans_variant(r: u32) -> u32 {
    r & !(VARIANT_ID_MASK as u32)
}

// ── pawn species — RETIRED (definition-registry P5) ────────────────────────────────────────────
//
// `PAWN_SPECIES_ANIMAL/HUMAN` and `pawn_species_subtype_id` lived here because the manifest's
// subcategories were alphabetical, so a new species inserting mid-list would renumber stored defs:
// "a content registry can own these only once it guarantees append-only numbering."
//
// It does now. `index.definitions` numbers every `(type, subType, kind, variant)` tuple, never
// renumbers an existing row, and refuses to hand one id to two definitions — so a species is a
// corpus edit (`subType = ["reptile"]`) with no code change, and its number is allocated once and
// kept. Git holds the palette.

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

/// A cold cell's **deterministic** `entity_reference` — a pure function of position, because a cold
/// cell's identity *is* its position (one entity per cell). `object_reference` = `macro_position:16 |
/// tile_reference:8` (fits the `u24` exactly; the layer is the shard's). The caller computes this and
/// spells it as a `SET` write operand, so the event pipeline groups/claims/composes it with **no**
/// mint (no deferred spawn-id claim, no position→id watch). `server_reference` is the cold shard's
/// (`TYPE_BIOME_TILE`/`TYPE_BIOME_THING` | `server_id`). Idempotent: re-issuing for a cell reuses the
/// id. See `docs/work/cold-rework/deviations.md` D-1.
pub fn cold_entity_reference(server_reference: u8, position_reference: u32) -> u32 {
    let object_reference =
        ((position_macro(position_reference) as u32) << 8) | (position_tile(position_reference) as u32);
    crate::refs::pack_entity_reference(server_reference, object_reference)
}

// ── cold_row_reference : u32 = macro_position:16 | subtype_id:12 | layer_id:4 ────────────────────
//
// The **cold row's identity** — the composite of the three header fields that identify a row:
// `macro_position_reference` (the zone), `subtype_id` (the biome), and `layer_id`. The **`type_id`
// is the shard** (the `tile` module *is* `TYPE_BIOME_TILE`, `thing` *is* `TYPE_BIOME_THING`), so it
// is out of the key and off the row — dropping its `u4` (plus the old `reserved:8`) freed the `u12`
// `subtype_id`. Reconstruction sources `type_id` from the shard: `type_reference = shard.type_id |
// row.subtype_id`, `layer_reference = shard.type_id | row.layer_id`.
//
// A zone with N biomes is N rows (same `macro`, different `subtype`) — a position alone does NOT
// name a row (it lacks the biome), so there is no `position → row` pure function.

const COLD_ROW_MACRO_SHIFT: u32 = 16;
const COLD_ROW_SUBTYPE_SHIFT: u32 = 4;
const COLD_ROW_U16_MASK: u32 = 0xFFFF;
const COLD_ROW_SUBTYPE_MASK: u32 = 0xFFF; // 12 bits
const COLD_ROW_LAYER_ID_MASK: u32 = 0xF; // 4 bits

/// Compose a `cold_row_reference`: `macro_position:16 | subtype_id:12 | layer_id:4`. The `type_id` is
/// the module (one cold shard per type), so it's out of the key — a `u32` is unique within a module,
/// mirroring `entity_reference` so the `(row, tic)` slot uid matches hot's.
pub fn pack_cold_row_reference(macro_position: u16, subtype_id: u16, layer_id: u8) -> u32 {
    ((macro_position as u32) << COLD_ROW_MACRO_SHIFT)
        | ((subtype_id as u32 & COLD_ROW_SUBTYPE_MASK) << COLD_ROW_SUBTYPE_SHIFT)
        | (layer_id as u32 & COLD_ROW_LAYER_ID_MASK)
}
/// The `macro_position_reference` of a `cold_row_reference`.
pub fn cold_row_macro_position(r: u32) -> u16 {
    ((r >> COLD_ROW_MACRO_SHIFT) & COLD_ROW_U16_MASK) as u16
}
/// The `subtype_id` (the biome; 0..4096) of a `cold_row_reference`.
pub fn cold_row_subtype(r: u32) -> u16 {
    ((r >> COLD_ROW_SUBTYPE_SHIFT) & COLD_ROW_SUBTYPE_MASK) as u16
}
/// The `layer_id` (0..16) of a `cold_row_reference`.
pub fn cold_row_layer_id(r: u32) -> u8 {
    (r & COLD_ROW_LAYER_ID_MASK) as u8
}

// ── kind_pos_reference : u32 = kind_reference:16 | tile_reference:8 | data:8 ─────────────────────
//
// The cold row's per-object entry (one per object in its `Vec<u32>`), carrying the object's full
// per-instance delta. The row header supplies the shared `(macro_position, subtype, layer_id)` and
// the shard supplies `type_id`. Readers are `kind_pos_ref_*` — they read an **entry**, not a
// `kind_reference`.

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
/// A PAWN's `data`: `facing:2 | trip_serial:6` — the same bit split as the cold
/// `rotation | count` (`data` is decoded by `type_id`). Facing rides the rotation bits;
/// the live movement chain's identity rides the low six (`ACTIONS.md` §Movement chain
/// identity, `TABLES.md` § pawn). One layout, one owner: the worker stamps it, hops check it.
pub fn pack_pawn_data(facing: u8, trip_serial: u8) -> u8 {
    pack_data(facing, trip_serial)
}

/// The trip-serial in a pawn's `data` (see [`pack_pawn_data`]).
pub fn pawn_trip_serial(d: u8) -> u8 {
    data_count(d)
}

/// The `rotation` (0..4 facings; west mirrors east).
pub fn data_rotation(d: u8) -> u8 {
    (d >> DATA_ROT_SHIFT) & DATA_ROT_MASK
}
/// The `count` (0..64) — how many of this object sit on the tile.
pub fn data_count(d: u8) -> u8 {
    d & DATA_COUNT_MASK
}

// ── global tile ⇄ position (first-pawns: lifted from client/core so the WORKER shares the
// exact conversion the clients use — movement steps on global tiles) ──────────────────────
//
// A global tile axis is `region_nibble * 256 + zone_nibble * 16 + tile_nibble` (0..4096).

/// Decode a `position_reference` to global tile `(x, y)` (layer dropped).
pub fn position_to_tile(position_reference: u32) -> (i32, i32) {
    let region = position_region(position_reference);
    let zone = position_zone(position_reference);
    let tile = micro_position_tile(position_micro(position_reference));
    let x = ref_hi(region) as i32 * 256 + ref_hi(zone) as i32 * 16 + ref_hi(tile) as i32;
    let y = ref_lo(region) as i32 * 256 + ref_lo(zone) as i32 * 16 + ref_lo(tile) as i32;
    (x, y)
}

/// Compose a `position_reference` for global tile `(x, y)` on layer `0` — the inverse of
/// [`position_to_tile`]. Out-of-range axes are masked into the 12-bit tile space.
pub fn tile_to_position(tile_x: i32, tile_y: i32) -> u32 {
    let (rx, zx, tx) = split_global_axis(tile_x);
    let (ry, zy, ty) = split_global_axis(tile_y);
    let region = pack_tile_reference(rx, ry);
    let zone = pack_tile_reference(zx, zy);
    let tile = pack_tile_reference(tx, ty);
    pack_position_from_parts(region, zone, tile, 0)
}

/// Split a global tile axis into its (region, zone, tile) nibbles.
fn split_global_axis(t: i32) -> (u8, u8, u8) {
    let t = t.rem_euclid(4096) as u32;
    (((t >> 8) & 0xF) as u8, ((t >> 4) & 0xF) as u8, (t & 0xF) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pawn_data_packs_facing_and_trip_serial() {
        // facing 3 (w) + serial 13: facing in the top 2 bits, serial in the low 6 — same
        // split as rotation|count, so both decoders agree on one byte.
        let d = pack_pawn_data(3, 13);
        assert_eq!(d, (3 << 6) | 13);
        assert_eq!(data_rotation(d), 3);
        assert_eq!(pawn_trip_serial(d), 13);
        // A serial re-stamp preserves facing; a facing re-stamp preserves the serial.
        let restamped = pack_pawn_data(data_rotation(d), 62);
        assert_eq!(data_rotation(restamped), 3);
        assert_eq!(pawn_trip_serial(restamped), 62);
    }

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
    fn cold_row_reference_is_the_macro_subtype_layer_composite() {
        for &(m, sub, lid) in &[(0u16, 0u16, 0u8), (0xABCD, 0x678, 0x3), (0xFFFF, 0xFFF, 0xF)] {
            let k = pack_cold_row_reference(m, sub, lid);
            assert_eq!(cold_row_macro_position(k), m);
            assert_eq!(cold_row_subtype(k), sub);
            assert_eq!(cold_row_layer_id(k), lid);
        }
        // All-ones per field, disjoint, exactly fill the u32 (no type_id, no reserved).
        assert_eq!(pack_cold_row_reference(0xFFFF, 0xFFF, 0xF), u32::MAX);
        // Distinct by macro, by subtype (the biome), and by layer_id — each an independent axis.
        assert_ne!(pack_cold_row_reference(0x1234, 6, 0), pack_cold_row_reference(0x1235, 6, 0));
        assert_ne!(pack_cold_row_reference(0x1234, 6, 0), pack_cold_row_reference(0x1234, 7, 0));
        assert_ne!(pack_cold_row_reference(0x1234, 6, 0), pack_cold_row_reference(0x1234, 6, 1));
        // subtype/layer_id over-wide inputs mask, never bleed into a neighbour field.
        assert_eq!(pack_cold_row_reference(0, 0x1000, 0), 0); // subtype bit 12 dropped
        assert_eq!(pack_cold_row_reference(0, 0, 0x10), 0); // layer_id bit 4 dropped
    }

    #[test]
    fn cold_entity_reference_is_deterministic_from_position() {
        use crate::refs::{entity_ref_server_reference, pack_server_reference};
        let srv = pack_server_reference(TYPE_BIOME_TILE, 0);
        for &(reg, z, t, l) in &[(0u8, 0u8, 0u8, 0u8), (1, 2, 3, 4), (0xAB, 0xCD, 0xEF, 0x35)] {
            let p = pack_position_from_parts(reg, z, t, l);
            let e = cold_entity_reference(srv, p);
            // The server byte routes it; the object_reference is macro:16 | tile:8.
            assert_eq!(entity_ref_server_reference(e), srv);
            assert_eq!(e & 0x00FF_FFFF, ((position_macro(p) as u32) << 8) | position_tile(p) as u32);
            // Deterministic + idempotent: same position → same id; the layer does NOT change it
            // (one cold entity per cell, layer is the shard's).
            assert_eq!(e, cold_entity_reference(srv, p));
            assert_eq!(e, cold_entity_reference(srv, pack_position_from_parts(reg, z, t, l ^ 0x0F)));
        }
        // Distinct cells (by macro or by tile) get distinct ids.
        let a = cold_entity_reference(srv, pack_position_from_parts(0, 0, 0x11, 0));
        let b = cold_entity_reference(srv, pack_position_from_parts(0, 0, 0x12, 0));
        let c = cold_entity_reference(srv, pack_position_from_parts(0, 1, 0x11, 0));
        assert_ne!(a, b);
        assert_ne!(a, c);
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
