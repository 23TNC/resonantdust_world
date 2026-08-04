//! The definition ALLOCATOR — the master's half of the registry (work
//! `2026-08-04-definition-registry`, [F11]).
//!
//! The corpus describes; this composes the numbers and the `index.definitions` table records them.
//! Composition lives here rather than in the reducer because three of the four coordinates are not
//! the module's to know ([I8]): `subtype_id` is AUTHORED in `biomes.toml`, and `variant_id` is
//! chosen at placement rather than per def. Both are reachable from the loaded corpus; neither is
//! reachable from inside a WASM reducer.
//!
//! **The packed layout is FROZEN** ([F13]). Nothing here widens, narrows or moves a field — it
//! composes ids in the layout `docs/VARIABLES.md` already specifies, and refuses anything that
//! would not fit rather than wrapping. Wrapping would alias two definitions onto one number, which
//! is the single thing the registry exists to prevent.

use resonantdust_codec::object::{pack_definition_from_ids, KIND_ID_LIMIT, VARIANT_ID_LIMIT};

/// One tuple of the cross-product, with the id it composes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub id: u32,
    pub version: u32,
    pub type_name: String,
    pub sub_type: String,
    pub kind: String,
    pub variant: String,
}

/// Why a tuple could not be numbered. Every arm is a LOAD ERROR — never a warning, never a silent
/// truncation. An unnumberable definition means the corpus and the world would disagree about what
/// a stored id means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocError {
    /// `kind_id` past `u12`. 4096 kinds INCLUDING every retired version (F5 burns kind space).
    KindOverflow { kind: String, kind_id: u32 },
    /// A 17th variant of one `(type, sub_type, kind)`. The art tree may hold more folders than the
    /// id can address — those truncate out of the manifest, by design — but an AUTHORED variant
    /// past the ceiling is a mistake, not overflow to absorb.
    VariantOverflow { kind: String, variant: String, slot: u32 },
    /// `subtype_id` past `u12`.
    SubtypeOverflow { sub_type: String, subtype_id: u32 },
}

impl std::fmt::Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KindOverflow { kind, kind_id } => write!(
                f,
                "kind `{kind}` allocated id {kind_id}, past the u12 ceiling ({KIND_ID_LIMIT}) — \
                 4096 kinds including every retired version"
            ),
            Self::VariantOverflow { kind, variant, slot } => write!(
                f,
                "kind `{kind}` variant `{variant}` wants slot {slot}, past the u4 ceiling \
                 ({VARIANT_ID_LIMIT}) — 16 variants per (type, subType, kind); split the kind"
            ),
            Self::SubtypeOverflow { sub_type, subtype_id } => write!(
                f,
                "subType `{sub_type}` has id {subtype_id}, past the u12 ceiling ({KIND_ID_LIMIT})"
            ),
        }
    }
}

/// Compose one tuple's `definition_reference`, refusing anything that would not fit.
///
/// `variant_slot` is the u4 SLOT, not the label: the label is what the art tree spells (`"wall"`,
/// `"124"`) and is free-form, while the slot is what the id carries. Numeric labels are their own
/// slot; a named form takes the slot its kind assigns.
pub fn compose(
    type_id: u8,
    subtype_id: u16,
    kind_id: u16,
    variant_slot: u32,
    kind: &str,
    variant: &str,
    sub_type: &str,
) -> Result<u32, AllocError> {
    if u32::from(subtype_id) >= KIND_ID_LIMIT {
        return Err(AllocError::SubtypeOverflow {
            sub_type: sub_type.to_string(),
            subtype_id: u32::from(subtype_id),
        });
    }
    if u32::from(kind_id) >= KIND_ID_LIMIT {
        return Err(AllocError::KindOverflow { kind: kind.to_string(), kind_id: u32::from(kind_id) });
    }
    if variant_slot >= VARIANT_ID_LIMIT {
        return Err(AllocError::VariantOverflow {
            kind: kind.to_string(),
            variant: variant.to_string(),
            slot: variant_slot,
        });
    }
    Ok(pack_definition_from_ids(type_id, subtype_id, kind_id, variant_slot as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use resonantdust_codec::object::{def_kind_id, def_subtype_id, def_type_id, def_variant_id, TYPE_BIOME_THING};

    #[test]
    fn composes_the_frozen_layout() {
        // conifer (kind 1) in forest (subtype 6), variant 4 — every field round-trips.
        let id = compose(TYPE_BIOME_THING, 6, 1, 4, "conifer", "4", "forest").unwrap();
        assert_eq!(def_type_id(id), TYPE_BIOME_THING);
        assert_eq!(def_subtype_id(id), 6);
        assert_eq!(def_kind_id(id), 1);
        assert_eq!(def_variant_id(id), 4);
    }

    #[test]
    fn a_version_bump_burns_kind_space_and_leaves_siblings_alone() {
        // F5: conifer v1 takes a NEW kind_id; the rock beside it in the same subType does not move.
        let conifer_v0 = compose(TYPE_BIOME_THING, 6, 1, 4, "conifer", "4", "forest").unwrap();
        let rock = compose(TYPE_BIOME_THING, 6, 2, 0, "rock", "0", "forest").unwrap();
        let conifer_v1 = compose(TYPE_BIOME_THING, 6, 3, 4, "conifer", "4", "forest").unwrap();

        assert_ne!(conifer_v0, conifer_v1, "a bump must mint a different id");
        assert_eq!(def_subtype_id(conifer_v0), def_subtype_id(conifer_v1), "subType is untouched");
        assert_eq!(def_variant_id(conifer_v0), def_variant_id(conifer_v1), "variant is untouched");
        // The sibling kind is bit-identical before and after — the whole point of burning KIND.
        assert_eq!(rock, compose(TYPE_BIOME_THING, 6, 2, 0, "rock", "0", "forest").unwrap());
    }

    #[test]
    fn a_seventeenth_variant_is_refused_not_wrapped() {
        // The wolf sits at 15 of 16 today, so this boundary is one art folder away.
        for slot in 0..VARIANT_ID_LIMIT {
            assert!(compose(TYPE_BIOME_THING, 0, 1, slot, "conifer", "v", "default").is_ok());
        }
        let err = compose(TYPE_BIOME_THING, 0, 1, VARIANT_ID_LIMIT, "conifer", "v16", "default")
            .expect_err("a 17th variant must fail");
        assert!(matches!(err, AllocError::VariantOverflow { .. }), "{err}");
        // WHY the refusal matters, demonstrated rather than asserted away: the packer MASKS, so a
        // slot of 16 silently becomes variant 0 and the 17th definition aliases onto the 1st.
        // Nothing downstream could tell them apart — the id is the whole identity.
        let zero = compose(TYPE_BIOME_THING, 0, 1, 0, "conifer", "v0", "default").unwrap();
        assert_eq!(
            pack_definition_from_ids(TYPE_BIOME_THING, 0, 1, 16u8),
            zero,
            "the packer wraps 16 → 0; refusing at the ceiling is what stops the alias"
        );
    }

    #[test]
    fn a_kind_past_u12_is_refused() {
        assert!(compose(TYPE_BIOME_THING, 0, 4095, 0, "last", "0", "default").is_ok());
        let err = compose(TYPE_BIOME_THING, 0, 4096, 0, "one_too_many", "0", "default")
            .expect_err("kind 4096 must fail");
        assert!(matches!(err, AllocError::KindOverflow { .. }), "{err}");
    }
}
