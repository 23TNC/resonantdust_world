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

use resonantdust_codec::object::{
    gameplay_subtype_id, pack_definition_from_ids, KIND_ID_LIMIT, TYPE_BIOME_THING,
    TYPE_BIOME_TILE, TYPE_GAMEPLAY, TYPE_PAWN, VARIANT_ID_LIMIT,
};
use resonantdust_content::loader::{Bundle, Taxonomy};

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
    /// A `type` name the CODE palette does not know. Types are structural — each implies a pipeline
    /// — so an unknown one is a corpus error, never a number invented on the spot.
    UnknownType { type_name: String },
    /// A `subType` name with no id: an unauthored biome, or a species outside the palette.
    UnknownSubType { type_name: String, sub_type: String },
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
            Self::UnknownType { type_name } => {
                write!(f, "type `{type_name}` is not in the code palette (biome-tile|biome-thing|pawn)")
            }
            Self::UnknownSubType { type_name, sub_type } => write!(
                f,
                "type `{type_name}`: subType `{sub_type}` has no id — an unauthored biome, or a \
                 species outside the palette"
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

    /// The repo's authored corpus, or `None` in a packaged build without it.
    fn corpus() -> Option<Bundle> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let sources = resonantdust_content::content::read_content_dir(&root).ok()?;
        if sources.is_empty() {
            return None;
        }
        Some(resonantdust_content::load(&sources).expect("the repo corpus loads clean"))
    }

    #[test]
    fn each_authored_version_gets_its_own_row_at_its_own_position() {
        // B6/F17: versions are AUTHORED and coexist in the corpus, so each is simply another def
        // with its own position and its own id. No fingerprint, no bump to derive, no fresh
        // coordinate — which is what dissolved B5.
        let sources = vec![
            // The species' subtype id lives in the corpus now (F16), so the fixture needs it too.
            (
                "subtypes.toml".to_string(),
                "[[subtype]]\ntype = \"pawn\"\nname = \"animal\"\nid = 1\n".to_string(),
            ),
            (
            "things.toml".to_string(),
            r##"
[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
speed = 12

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
version = 1
speed = 9
"##
            .to_string(),
            ),
        ];
        let bundle = resonantdust_content::load(&sources).expect("loads");
        let allocs = allocations(&bundle).expect("expands");
        let wolves: Vec<_> = allocs.iter().filter(|a| a.kind == "wolf").collect();
        assert_eq!(wolves.len(), 2, "both versions register");

        let v0 = wolves.iter().find(|a| a.version == 0).unwrap();
        let v1 = wolves.iter().find(|a| a.version == 1).unwrap();
        assert_ne!(v0.id, v1.id, "each version is its own identity");
        // Positional: v0 is thing 1, v1 is thing 2 — both inside the corpus, both indexable.
        assert_eq!(def_kind_id(v0.id), 1);
        assert_eq!(def_kind_id(v1.id), 2);
        // Same taxonomy — they are versions of ONE definition.
        assert_eq!(v0.sub_type, v1.sub_type);
        assert_eq!(def_subtype_id(v0.id), def_subtype_id(v1.id));
    }

    #[test]
    fn the_corpus_expands_to_the_ids_it_already_has() {
        // THE MIGRATION PROOF (I9). Seeded from the corpus's authored ids, the expansion must
        // reproduce exactly the numbering the world already stores — every one of which P0's
        // golden and the npc's def_fixture pin independently.
        let Some(bundle) = corpus() else { return };
        let allocs = allocations(&bundle).expect("the corpus expands cleanly");

        // One row per tuple, no duplicates — a duplicate id would mean two definitions aliased.
        let mut ids: Vec<u32> = allocs.iter().map(|a| a.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "an id was allocated twice");

        let find = |kind: &str, sub: &str, variant: &str| {
            allocs
                .iter()
                .find(|a| a.kind == kind && a.sub_type == sub && a.variant == variant)
                .unwrap_or_else(|| panic!("no allocation for {sub}/{kind}/{variant}"))
                .id
        };

        // The wolf: TYPE_PAWN | species animal | thing_object_id 7 | variant 0. This is the exact
        // value the live npc logs on every boot and `npc::def_fixture` pins.
        assert_eq!(find("wolf", "animal", "0"), 0x3001_0070, "the wolf's def moved");
        assert_eq!(find("female", "human", "0"), 0x3002_00A0);
        assert_eq!(find("male", "human", "0"), 0x3002_00B0);

        // grass is tile def_id 1 — the number every stored zone is full of.
        assert_eq!(def_kind_id(find("grass", "default", "0")), 1);
        assert_eq!(def_type_id(find("grass", "default", "0")), TYPE_BIOME_TILE);
        // wall_smooth is tile 6, and its NAMED variant takes slot 0.
        assert_eq!(def_kind_id(find("smooth", "default", "wall")), 6);
        assert_eq!(def_variant_id(find("smooth", "default", "wall")), 0);

        // GAMEPLAY defs (interactions F1/F9): the allocator's rows must equal the loader's SEED
        // fallback exactly — that agreement is what lets a registry-less boot resolve the same
        // refs a fresh registry records.
        assert_eq!(find("drink", "interaction", "default"), 0x8004_0010);
        assert_eq!(
            find("thirst", "need", "default"),
            bundle.gameplay_reference("need", "thirst").expect("seed ref"),
            "allocator and seed fallback disagree on thirst"
        );
        // The stat-model categories (predicate affordances + the new `stat` subtype 6).
        assert_eq!(
            find("can_drink", "affordance", "default"),
            bundle.gameplay_reference("affordance", "can_drink").expect("seed ref"),
        );
        assert_eq!(find("walks", "trait", "default"), 0x8003_0020);
        assert_eq!(find("ground_speed", "stat", "default"), 0x8006_0010);
        assert_eq!(
            find("metabolism", "stat", "default"),
            bundle.gameplay_reference("stat", "metabolism").expect("seed ref"),
        );

        // The cross-product is real: a 16-variant def contributes 16 rows that differ ONLY in the
        // variant nibble, so worldgen's per-cell roll always lands on a registered definition.
        let conifers: Vec<u32> =
            allocs.iter().filter(|a| a.kind == "conifer").map(|a| a.id).collect();
        assert_eq!(conifers.len(), 16, "conifer should expand to its 16 variants");
        for id in &conifers {
            assert_eq!(def_kind_id(*id), 1, "every conifer variant shares one kind_id");
        }
    }

    #[test]
    fn a_kind_past_u12_is_refused() {
        assert!(compose(TYPE_BIOME_THING, 0, 4095, 0, "last", "0", "default").is_ok());
        let err = compose(TYPE_BIOME_THING, 0, 4096, 0, "one_too_many", "0", "default")
            .expect_err("kind 4096 must fail");
        assert!(matches!(err, AllocError::KindOverflow { .. }), "{err}");
    }
}


// ── the cross-product: corpus → one allocation per tuple ─────────────────────────────────────
//
// [F2]: a def's `subType` and `variant` are APPLICABILITY ARRAYS, so one `conifer` block covers
// every `(subType, variant)` pair it names. Expanding them here is what turns "this definition
// applies to 48 tuples" into 48 registry rows, each with the number that identifies it.
//
// The `kind_id` comes from the corpus's authored `id` — the allocation SEED ([I9]). This is the
// migration proof, not a shortcut: a fresh registry seeded from today's authored ids reproduces
// today's numbering exactly, and P0's golden pins every one of those numbers. A registry allocated
// from scratch would only be a hope.

/// The `type_id` a taxonomy's `type` name maps to. The type palette is CODE-owned (structural —
/// each type implies a pipeline and a `data` decode), so an unknown name is a load error rather
/// than a number invented on the spot.
fn type_id_of(type_name: &str) -> Option<u8> {
    match type_name {
        "biome-tile" => Some(TYPE_BIOME_TILE),
        "biome-thing" => Some(TYPE_BIOME_THING),
        "pawn" => Some(TYPE_PAWN),
        "gameplay" => Some(TYPE_GAMEPLAY),
        _ => None,
    }
}

/// The `subtype_id` a taxonomy's `subType` name maps to, for a given type.
///
/// Three sources, because subtype means different things per type and each authors it where it
/// belongs (F16): `"default"` is the reserved biome-agnostic `0`; a BIOME authors its id on the
/// biome record; anything else — a pawn SPECIES today — authors it in `subtypes.toml`. All three
/// are content now; the code-owned species palette is gone.
fn subtype_id_of(bundle: &Bundle, type_id: u8, sub_type: &str) -> Option<u16> {
    // The gameplay CATEGORY palette is code-owned like the type palette (interactions F1/F9):
    // a category implies a loader schema + an executor, so it is schema vocabulary, not
    // content the corpus adds to — unlike a species, which is (F16).
    if type_id == TYPE_GAMEPLAY {
        return gameplay_subtype_id(sub_type);
    }
    if sub_type == "default" {
        return Some(0);
    }
    if type_id == TYPE_PAWN {
        return bundle.subtype_id_of("pawn", sub_type);
    }
    bundle.biome_subtype_id(sub_type)
}

/// A variant LABEL's u4 slot. A numeric label is its own slot (an art variation index that worldgen
/// rolls); a NAMED form (`wall`, `fence`) takes slot 0 — it is the only variant its kind has, which
/// is exactly why it appears in the stem instead ([`Taxonomy::stem`]).
fn variant_slot(variant: &str) -> u32 {
    variant.parse::<u32>().unwrap_or(0)
}

/// Expand one def's taxonomy into its registry allocations.
fn expand(
    bundle: &Bundle,
    tax: &Taxonomy,
    seed_kind_id: u16,
    version: u32,
) -> Result<Vec<Allocation>, AllocError> {
    let type_id = type_id_of(&tax.type_name)
        .ok_or_else(|| AllocError::UnknownType { type_name: tax.type_name.clone() })?;
    let mut out = Vec::new();
    for (sub_type, variant) in tax.tuples() {
        let subtype_id = subtype_id_of(bundle, type_id, sub_type).ok_or_else(|| {
            AllocError::UnknownSubType { type_name: tax.type_name.clone(), sub_type: sub_type.to_string() }
        })?;
        let id = compose(
            type_id,
            subtype_id,
            seed_kind_id,
            variant_slot(variant),
            &tax.kind,
            variant,
            sub_type,
        )?;
        out.push(Allocation {
            id,
            version,
            type_name: tax.type_name.clone(),
            sub_type: sub_type.to_string(),
            kind: tax.kind.clone(),
            variant: variant.to_string(),
        });
    }
    Ok(out)
}

/// Every allocation the corpus implies — the full set the master hands to `ensure_definition`.
///
/// Defs with no authored taxonomy are SKIPPED, not guessed: a guessed taxonomy would mint rows
/// nobody authored.
///
/// `version` is AUTHORED ([F17]) — the corpus carries every live revision, so each is simply another
/// def with its own position and its own number. There is no bump to derive and no fresh
/// coordinate to allocate: `kind_id` is the corpus POSITION for every version, which is what
/// dissolved [B5](../../../docs/work/2026-08-04-definition-registry/blockers.md#b5).
pub fn allocations(bundle: &Bundle) -> Result<Vec<Allocation>, AllocError> {
    let mut out = Vec::new();
    for (i, name) in bundle.tile_names().iter().enumerate() {
        let def_id = (i + 1) as u16;
        if name.is_empty() {
            continue; // a retired id — a hole stays a hole
        }
        if let Some(tax) = bundle.tile_taxonomy(def_id) {
            out.extend(expand(bundle, tax, def_id, bundle.tile_version(def_id).unwrap_or(0))?);
        }
    }
    for (i, name) in bundle.thing_names().iter().enumerate() {
        let object_id = (i + 1) as u16;
        if name.is_empty() {
            continue;
        }
        if let Some(tax) = bundle.thing_taxonomy(object_id) {
            out.extend(expand(bundle, tax, object_id, bundle.thing_version(object_id).unwrap_or(0))?);
        }
    }
    // The GAMEPLAY categories (interactions F1/F9): the taxonomy is DERIVED —
    // `gameplay/<category>/<name>/default` — and the kind seed is the corpus position,
    // exactly the posture the tile/thing seeds established (the migration proof: a fresh
    // registry reproduces what the seed fallback already resolves).
    for (category, names) in [
        ("need", bundle.need_names()),
        ("condition", bundle.condition_names()),
        ("trait", bundle.trait_names()),
        ("interaction", bundle.interaction_names()),
        ("affordance", bundle.affordance_names()),
        ("stat", bundle.stat_names()),
    ] {
        for (i, name) in names.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            let tax = Taxonomy {
                type_name: "gameplay".to_string(),
                kind: name.clone(),
                sub_type: vec![category.to_string()],
                variant: vec!["default".to_string()],
            };
            out.extend(expand(bundle, &tax, (i + 1) as u16, 0)?);
        }
    }
    Ok(out)
}


/// What the registry already knows: tuple → the `(version, sim_fingerprint, kind_id)` rows on
/// record. `kind_id` matters because a bump must take a FRESH one ([F5]) — reusing it would put two
/// versions on one id, and the id is the primary key.

/// Seed the registry from a corpus: expand, then `ensure_definition` each allocation.
///
/// Idempotent end to end — the reducer no-ops on a row it already holds ([P2]), so this runs on
/// every master boot without coordination and without churning the table. Returns how many
/// allocations were sent.
///
/// A corpus that will not expand is a HARD failure: the master would otherwise fan a world whose
/// definitions nothing has registered, and the first stored id would mean nothing. Better to log
/// loudly and leave the registry as it was.
pub async fn seed_registry(
    index: &resonantdust_st_bindings::index::DbConnection,
    bundle: &resonantdust_content::loader::Bundle,
) -> Result<usize, AllocError> {
    use resonantdust_st_bindings::index::ensure_definition as _;
    use spacetimedb_sdk::DbContext;
    let allocs = allocations(bundle)?;

    // KIND-SPACE PRESSURE ([F7](../../../docs/work/2026-08-04-definition-registry/forks.md#f7)).
    // Reclaim is designed and deliberately unbuilt; this is the number that decides when that
    // stops being the right call. `used` counts DISTINCT kind ids the registry has ever handed
    // out — every version of every kind, since none is ever reused — against the u12 ceiling.
    let used: std::collections::HashSet<u16> =
        allocs.iter().map(|a| resonantdust_codec::object::def_kind_id(a.id)).collect();
    let versions = allocs.iter().filter(|a| a.version > 0).count();
    tracing::info!(
        kind_ids_used = used.len(),
        kind_ids_free = KIND_ID_LIMIT as usize - used.len(),
        bumps_on_record = versions,
        "definition kind-space"
    );
    for a in &allocs {
        if let Err(err) = index.reducers().ensure_definition(
            a.id,
            a.version,
            a.type_name.clone(),
            a.sub_type.clone(),
            a.kind.clone(),
            a.variant.clone(),
        ) {
            // A send failure is the uplink's problem, not the corpus's — log and keep going, so
            // one dropped call does not abandon the rest of the seed. The next boot re-sends.
            tracing::warn!(id = format!("{:#010x}", a.id), %err, "ensure_definition send failed");
        }
    }
    Ok(allocs.len())
}
