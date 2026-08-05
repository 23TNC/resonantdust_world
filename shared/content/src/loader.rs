//! Content loader — parse `content/*.toml` into one ready-to-query, fully
//! MATERIALIZED [`Bundle`] (work `2026-08-04-toml-content`; the `.rd` DSL that
//! preceded it is deleted — git holds its history).
//!
//! The corpus is DATA: explicit ids (F1 — they are stored in zones, pawn defs and
//! payload words, so the loader REFUSES duplicates), one record per def, biome
//! classification as declarative rule tables (F3). Schema: `docs/VARIABLES.md`.
//!
//! Every accessor reads precomputed data. The server links this as an rlib while
//! the client reaches it through the `resonantdust-shared` wasm bundle — same
//! content, same ids, both sides.

/// A single load-time problem (a parse error, an id-law violation), tagged with its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
  pub file: String,
  pub message: String,
}

/// The outcome of classifying one tile: which biome claimed the cell and what it
/// decided to place. Names, not ids: worldgen resolves them to `def_id`s through
/// this same bundle before packing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GenTile {
  /// The biome that claimed this cell (`None` if none matched).
  pub biome: Option<String>,
  /// The ground tile name it chose.
  pub tile: Option<String>,
  /// The primary thing-layer name, or `None` if the cell stays bare.
  pub thing1: Option<String>,
}

/// One packed-map channel's MATERIAL binding on a prim. The stem's `packed` map
/// holds a per-pixel weight in each of its (up to 4) RGBA channels; this says what
/// that weight paints — a `tint` (base `0xRRGGBB` colour) and, optionally, a
/// `material_id` (1-based into the material registry; `0` = none) whose noise-driven
/// hue/chroma jitter the bake pass applies. All-zero = an unbound channel (identity:
/// the residual reconstructs the flat albedo, no variation). See `docs/components/client/webgl/design/lighting.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedChannel {
  /// 1-based material-registry id (`material_id`); `0` = no material (flat tint).
  pub material_id: u16,
  /// The channel's base colour, packed `0xRRGGBB`.
  pub tint: u32,
}

/// A named material's rendering parameters (the `<material>` registry, mirrored to
/// the client). `noise_field` names a tiling character field the client resolves to
/// an atlas index; the swings drive hue/chroma jitter in OKLab (never lightness);
/// `sample_space` is `"uv"` (rides the sprite) or `"world"` (pinned to the ground).
/// All-zero swings = identity (smooth tintable).
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParams {
  /// The noise-field character name (`strand`/`mottle`/…); `""` = no field (flat).
  pub noise_field: String,
  /// Hue rotation amplitude at full noise, in DEGREES.
  pub hue_swing: f64,
  /// Chroma perturbation amplitude at full noise.
  pub chroma_swing: f64,
  /// Warm↔cool asymmetry of the swing, `-1`..`1` (cool..warm).
  pub warm_cool_bias: f64,
  /// `"uv"` (default) or `"world"` — where the noise is sampled.
  pub sample_space: String,
  /// NORMAL-DETAIL field name (material-system P1) — a tiling field RNM-blended onto the
  /// base normal at bake, restoring the high-frequency structure generated normals lack.
  /// `""` = none.
  pub detail_field: String,
  /// Normal-detail amplitude; `0` = off (identity — the base normal is untouched).
  pub detail_amp: f64,
  /// Normal-detail spatial scale (UV tiling multiplier); `1` = the field's native tile.
  pub detail_scale: f64,
}

impl Default for MaterialParams {
  fn default() -> Self {
    Self { noise_field: String::new(), hue_swing: 0.0, chroma_swing: 0.0, warm_cool_bias: 0.0,
           sample_space: "uv".into(), detail_field: String::new(), detail_amp: 0.0, detail_scale: 1.0 }
  }
}

/// The visual parts a def declares. `tint` multiplies a LOADED texture (usually
/// white/no-op for full-colour art); `geo_color` is the flat silhouette colour shown
/// while the prim is still on the geo tier (defaults to `tint` when the def sets
/// none); `texture` is the sprite STEM (`None` when the def declares no texture, or
/// declares the built-in `"white"` fill). Shared by tiles and things.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualParts {
  pub tint: u32,
  pub geo_color: u32,
  pub texture: Option<String>,
  /// The tiles this prim OCCUPIES — `(w, h)`, default `(1, 1)`. Logical grid extent
  /// (movement / hit-testing), authored in the object's own frame; the object's
  /// position is its TOP-LEFT tile. Rotation swaps `w`/`h` for an n/s facing.
  /// **Server-side occupancy is deferred** (`docs/components/shared/codec/design/object-model.md`).
  pub footprint: (f64, f64),
  /// The prim's logical ANCHOR within its footprint — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)` (centre). `(0.5, 1.0)` is bottom-centre. The `sprite_anchor`
  /// point of the art is pinned here.
  pub anchor: (f64, f64),
  /// The sprite's on-screen scale in TILES, default `1.0`. SUPERSEDED by `span` +
  /// `sprite_scale` (def-frame-anchors P5) — kept while legacy consumers migrate.
  pub size: f64,
  /// The sprite frame's WORLD SPAN in tiles — pow2, ≤ one zone — default `1.0`.
  pub span: f64,
  /// Pre-atlas sprite scale `(w, h)`, default `(1, 1)`: applied to the decoded sprite
  /// BEFORE it is packed.
  pub sprite_scale: (f64, f64),
  /// The pivot ON THE SPRITE that aligns to `anchor` — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)`. Measured against the [`DirFrame::sub`] SUBFRAME (subframe-ingest F7).
  pub sprite_anchor: (f64, f64),
  /// Per-(variant, rotation) subframe + pivot (subframe-ingest F1/F7/I10):
  /// `[VARIANTS_PER_DEF][ROTATIONS_PER_DEF]`, resolved through the authoring
  /// fallback chain at LOAD (most-specific first: `v3.r1 → v3.e → v3 → r1 → e → base`,
  /// per COMPONENT). Index 3 (west) is the east master mirrored, derived by the
  /// client — never authored.
  pub dir_frames: [[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
  /// The prim's up-to-4 packed-map channel material bindings, indexed by packed RGBA
  /// channel. Default (all zero) = no material system in play.
  pub packed: [PackedChannel; 4],
  /// The LIGHT this kind emits, or `None` when it emits nothing. Authored per KIND,
  /// not per instance (work `2026-07-25-primitive-graph`).
  pub light: Option<LightParts>,
  /// EVERY part slot, in slot order — the pawn PARTS list (human-pawns P2).
  /// `parts[0]` mirrors the flat prim-0 fields above (a wolf's whole visual = one
  /// entry); a human carries body + head.
  pub parts: Vec<VisualPart>,
}

/// One DIRECTION's authored art rect and pivot (subframe-ingest F1/F7). Both are
/// FRACTIONS (`0..1`): a fraction addresses the same art at every served size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirFrame {
  /// `(x, y, w, h)` of the art inside the master, `0..1`. Default `(0, 0, 1, 1)` — the
  /// whole frame, i.e. "this master is already tight".
  pub sub: (f64, f64, f64, f64),
  /// The pivot ON [`Self::sub`] that pins to the prim's `anchor`, `0..1`. Default `(0.5, 0.5)`.
  pub anchor: (f64, f64),
}

impl Default for DirFrame {
  fn default() -> Self {
    Self { sub: (0.0, 0.0, 1.0, 1.0), anchor: (0.5, 0.5) }
  }
}

/// Rotation slots per definition — **the one index space for facings and linked cells alike**
/// (subframe-ingest F9). A sprite uses the first four (`0 = s, 1 = e, 2 = n, 3 = w`) and a
/// linked tile uses all sixteen (the autotile cell, `y·4 + x`). Mirrors `ROTATIONS_PER_DEF`
/// in `client/webgl`'s `records.ts` — the two must move together.
pub const ROTATIONS_PER_DEF: usize = 16;

/// Variant slots per definition. **A variant and a rotation are DIFFERENT AXES**
/// (subframe-ingest I10) — a facing lands in the STEM, a variant in the CELL — so a
/// subframe is indexed by BOTH: `[variant][rotation]`.
pub const VARIANTS_PER_DEF: usize = 16;

/// Band slots per need / need slots per kind — the authoring caps (needs-moodlets P1).
pub const NEED_BANDS: usize = 4;
pub const NEEDS_PER_KIND: usize = 8;

/// One BAND on a need's satisfaction — the DERIVED condition active while
/// `lo <= satisfaction < hi` (needs-moodlets F2: band conditions are computed by every
/// observer from `(need row, tic, corpus)`; no grant events exist for them). Bands are
/// authored EXCLUSIVE, so at most one band condition per need is active.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedBand {
  /// The `<condition>` name this band activates (resolved via [`Bundle::condition_id`]).
  pub condition: String,
  /// Band start (inclusive), `0..1` satisfaction. Default `0`.
  pub lo: f64,
  /// Band end (exclusive), `0..1` satisfaction.
  pub hi: f64,
}

/// A `<need>` def's parameters (needs-moodlets P1). A need is a 0..1 SATISFACTION that
/// depletes toward zero (F1); nothing ticks it — observers compute `satisfaction_at(tic)`
/// from `deplete` (F4).
#[derive(Debug, Clone, PartialEq)]
pub struct NeedParams {
  /// Display label ("Thirst") — authoring/debug only. The need itself is NEVER shown.
  pub label: String,
  /// TICS from full (`1.0`) to empty (`0.0`). `0` = unauthored (the need never drains).
  pub deplete: f64,
  /// The derived-condition bands, in authored slot order.
  pub bands: Vec<NeedBand>,
}

/// A `<condition>` def's parameters — the DISPLAYED consequence of hidden state.
/// `duration == 0` marks a DERIVED condition (alive exactly while its band holds);
/// `> 0` a TIMED one (a stored grant expiring `duration` tics after its `grant_tic`).
///
/// `mood` is ONE effect, not the definition (conditions F6): a condition acts on pawn
/// state, and mood is simply the first such effect we carry. The open effect set — need
/// rates, later stats — is the successor stream's charter; add fields beside `mood`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionParams {
  pub label: String,
  /// Mood offset while active, `-1..1` (mood = clamp(base + Σ offsets), F5).
  pub mood: f64,
  /// Lifetime in TICS for a stored grant; `0` = DERIVED (band-computed).
  pub duration: f64,
  /// Display PRIORITY, descending — the details panel maximizes the top 4 and minimizes the
  /// rest (conditions F2). AUTHORED, never derived: `|mood|` cannot express "mild but urgent",
  /// and it means nothing at all once a condition's effect is a need rather than a mood ([F6]).
  /// Absent = `0`. Author in tens so a new condition can be slotted between two without
  /// renumbering. The full sort — `priority` desc, `|mood|` desc, `condition_id` asc — lives in
  /// [`crate::needs_eval::active_conditions`], NOT in any consumer (F3).
  pub priority: i32,
}

/// One part SLOT of a kind's visual skeleton (human-pawns P2). Slot index =
/// declaration order; slot 0 boxes the carrier.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualPart {
  pub tint: u32,
  pub geo_color: u32,
  /// The sprite STEM (`None` = a flat tint rect / the built-in `white`).
  pub texture: Option<String>,
  /// The master FILE part suffix (`<facing>.<part>` — a head is part 1).
  pub part: u32,
  /// The slot's pre-atlas art scale (uniform), def-frame-anchors semantics.
  pub scale: f64,
  /// Tile-space draw offset from the carrier's anchor.
  pub offset: (f64, f64),
  /// ELEVATION above the ground plane, tiles (z-positioning) — projects to a y-shift.
  pub elevation: f64,
  /// Per-slot draw depth (pawn-part-placement P2) — negated when facing away.
  pub depth: f64,
  /// The slot's own frame fields, same semantics as the flat prim-0 copies above.
  pub size: f64,
  pub span: f64,
  pub sprite_scale: (f64, f64),
  pub sprite_anchor: (f64, f64),
  pub anchor: (f64, f64),
  pub dir_frames: [[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
}

/// The deterministic per-tile draw — `<salt> ^rand` / a biome rule's scatter roll:
/// SplitMix64 finalizer over `seed ^ salt·φ`, yielding `[0, 1)`. Different salts give
/// independent draws for one tile. **The one derivation** — the `.rd` VM and the TOML
/// rule classifier both call this, so no scatter re-rolls across the migration
/// (toml-content P0 pinned it).
pub fn tile_rand(seed: u64, salt: i64) -> f64 {
  let mut h = seed ^ (salt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
  h ^= h >> 30;
  h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
  h ^= h >> 27;
  h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
  h ^= h >> 31;
  (h >> 11) as f64 / (1u64 << 53) as f64
}

// ── materialized defs (internal; toml_loader fills these too) ─────────────────────

/// A definition's TAXONOMY — the four names that identify it (work
/// `2026-08-04-definition-registry` F1). The corpus authors these; the server numbers them into a
/// `definition_reference` through the registry.
///
/// `sub_type` and `variant` are APPLICABILITY ARRAYS ([F2]): the def applies to every tuple in the
/// cross-product, so one `conifer` covers `[forest, plains, grassland] × [0..15]` rather than 48
/// blocks. They are also the TEXTURE PATH — `<type>/<sub_type>/<kind>/<variant>` is the art tree's
/// shape, which is why the stem is derived rather than authored.
///
/// Absent (`None`) on a def that has not been given a taxonomy yet — the field is being introduced
/// additively so the corpus and the golden fixture stay green throughout ([I9]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Taxonomy {
  pub type_name: String,
  pub kind: String,
  /// Every subType this def applies to. Never empty when a taxonomy is present.
  pub sub_type: Vec<String>,
  /// Every variant label it applies to — `"0".."15"` for art variations, or a named form
  /// (`"wall"`). Never empty when a taxonomy is present.
  pub variant: Vec<String>,
}

impl Taxonomy {
  /// The texture stem this taxonomy addresses. The corpus used to author this string; deriving it
  /// is the point of authoring the taxonomy at all.
  ///
  /// **A NAMED variant is part of the address; a NUMERIC one is not.** This is the linked-vs-plain
  /// split `VARIABLES.md` already draws: a linked object's `variant` is its FORM (`wall`, `fence`,
  /// `rock`) and names a distinct art folder, so it belongs in the stem —
  /// `biome-tile/default/smooth/wall`. A plain def's `variant` is an art VARIATION index that
  /// worldgen rolls per cell, so the stem stops at the kind and the resolver appends the roll —
  /// `biome-thing/default/conifer`, then `/4/e` at resolve time.
  ///
  /// Uses the FIRST subType, which is what the single-subType defs (every one today) resolve to.
  /// A multi-subType def's art is shared across its subTypes by construction — the stem names the
  /// kind, and the biome only decides where it is scattered.
  pub fn stem(&self) -> String {
    let sub = self.sub_type.first().map(String::as_str).unwrap_or("default");
    let base = format!("{}/{}/{}", self.type_name, sub, self.kind);
    match self.variant.as_slice() {
      // Exactly one variant, and it is a NAME rather than an index → part of the address.
      [only] if only.parse::<u32>().is_err() => format!("{base}/{only}"),
      _ => base,
    }
  }

  /// Every `(sub_type, variant)` pair this def applies to — [F2]'s cross-product, which is one
  /// registry row each.
  pub fn tuples(&self) -> Vec<(&str, &str)> {
    let mut out = Vec::with_capacity(self.sub_type.len() * self.variant.len());
    for s in &self.sub_type {
      for v in &self.variant {
        out.push((s.as_str(), v.as_str()));
      }
    }
    out
  }
}

/// One tile def, fully evaluated. `name` may be `""` for a RETIRED id (an F1 hole).
#[derive(Debug, Default, Clone)]
pub(crate) struct TileDef {
  pub name: String,
  /// Which REVISION ([F17]). Unauthored = 0; every live version stays authored.
  pub version: u32,
  /// The authored taxonomy, or `None` while the corpus is mid-migration ([I9]).
  pub taxonomy: Option<Taxonomy>,
  pub color: Option<u32>,
  pub visual: Option<VisualParts>,
  pub build: Option<String>,
  pub height: Option<f64>,
  /// `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]`.
  pub lanes: [f64; 6],
}

/// One thing def, fully evaluated. `name` may be `""` for a retired id.
#[derive(Debug, Default, Clone)]
pub(crate) struct ThingDef {
  pub name: String,
  /// Which REVISION ([F17]). Unauthored = 0; every live version stays authored.
  pub version: u32,
  /// The authored taxonomy, or `None` while the corpus is mid-migration ([I9]).
  pub taxonomy: Option<Taxonomy>,
  pub color: Option<u32>,
  pub visual: Option<VisualParts>,
  pub speed: Option<u16>,
  /// 1-based need ids (`&thing.needs` / `needs = [...]`), corpus-resolved.
  pub needs: Vec<u16>,
}

/// One biome, with its classifier body in either dialect.
#[derive(Debug, Clone)]
pub(crate) struct BiomeDef {
  pub name: String,
  /// The STORED subtype id (`0` = none/reserved — `biome_subtype_id` returns `None`).
  pub subtype: u16,
  pub body: BiomeBody,
}

/// The classifier body — the declarative rule form (F3). An enum still, so a future
/// dialect slots in behind the same `generate()` without touching consumers.
#[derive(Debug, Clone)]
pub(crate) enum BiomeBody {
  Rules(BiomeRules),
}

/// A comparison op, spelled exactly as the retired `.rd` ops were (F3): the golden
/// gate depends on reproducing the same half-open edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Cmp {
  Gte,
  Gt,
  Lt,
  Lte,
}

impl Cmp {
  fn pass(self, v: f64, t: f64) -> bool {
    match self {
      Cmp::Gte => v >= t,
      Cmp::Gt => v > t,
      Cmp::Lt => v < t,
      Cmp::Lte => v <= t,
    }
  }
}

/// The declarative biome body: a CONJUNCTION of dimension thresholds (the corpus never
/// used `or` — toml-content I1), a ground tile, and ordered scatter draws where the
/// LAST passing draw wins the cell (the `.rd` overwrite semantics).
#[derive(Debug, Clone, Default)]
pub(crate) struct BiomeRules {
  /// `(dimension index, op, threshold)` — all must pass.
  pub when: Vec<(usize, Cmp, f64)>,
  pub tile: Option<String>,
  /// `(salt, p, thing)` — rolls `tile_rand(seed, salt) < p`.
  pub scatter: Vec<(i64, f64, String)>,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv_bytes(h: &mut u64, bytes: &[u8]) {
  for &b in bytes {
    *h ^= b as u64;
    *h = h.wrapping_mul(0x0000_0100_0000_01b3);
  }
}

fn fnv_str(h: &mut u64, s: &str) {
  fnv_bytes(h, s.as_bytes());
  fnv_bytes(h, b"\0"); // separator, so ("ab","c") and ("a","bc") differ
}

/// Everything the runtime needs, built once at load — REGISTRIES in id order
/// (index = id − 1; a `""`-named slot is a retired id) + evaluated params.
#[derive(Default, Debug)]
pub struct Bundle {
  pub(crate) tiles: Vec<TileDef>,
  pub(crate) things: Vec<ThingDef>,
  /// Evaluation order = priority (first match wins), NOT an id namespace.
  pub(crate) biomes: Vec<BiomeDef>,
  pub(crate) materials: Vec<(String, MaterialParams)>,
  pub(crate) needs: Vec<(String, NeedParams)>,
  pub(crate) conditions: Vec<(String, ConditionParams)>,
  /// `(type, name) → subtype_id` for subtype axes with no record of their own — pawn species today
  /// (definition-registry F16). A biome's subtype is authored on the biome instead.
  pub(crate) subtypes: Vec<(String, String, u16)>,
  /// Registry name caches (id order) — what the slice-returning accessors serve.
  tile_names: Vec<String>,
  thing_names: Vec<String>,
  biome_names: Vec<String>,
  material_names: Vec<String>,
  need_names: Vec<String>,
  condition_names: Vec<String>,
  /// The REGISTRY override for `name → definition_reference` (definition-registry P4/P5), keyed
  /// `(is_tile, name)`. `None` means "resolve from corpus position", which is what a registry-less
  /// boot and every unit test does.
  ///
  /// The FULL packed def, not just the kind half. The kind half is what `tile_def_id` and
  /// `thing_object_id` serve, but the type and subtype halves are the taxonomy's numbering too —
  /// and throwing them away at bind time is what used to force the npc to recover a pawn's species
  /// by string-parsing its texture path. [`Bundle::definition_reference`] hands the whole thing
  /// back, which is what lets that parsing (and the code-owned species palette behind it) die.
  registry: Option<std::collections::HashMap<(bool, String), u32>>,
}

impl Bundle {
  /// Supply the registry's `name → kind_id` resolution, keyed `(is_tile, name)`.
  ///
  /// A name the registry does not carry falls back to the corpus's authored id, so a partially
  /// seeded registry degrades to today's behaviour rather than to nothing.
  pub fn with_registry(mut self, map: std::collections::HashMap<(bool, String), u32>) -> Self {
    self.registry = Some(map);
    self
  }

  /// Whether a registry has been injected — for a consumer that wants to log which authority it
  /// is resolving through.
  pub fn has_registry(&self) -> bool {
    self.registry.is_some()
  }

  /// The full `definition_reference` the registry carries for a name, if any. This is the
  /// TAXONOMY's number — type, subtype, kind and variant all — so a caller that needs a pawn's
  /// species reads it off the id rather than off an art path.
  pub fn definition_reference(&self, is_tile: bool, name: &str) -> Option<u32> {
    self.registry.as_ref()?.get(&(is_tile, name.to_string())).copied()
  }

  /// The KIND half of the registry's answer — what the `*_def_id` accessors serve.
  fn registry_id(&self, is_tile: bool, name: &str) -> Option<u16> {
    // `kind_reference = kind_id:12 | variant_id:4`; the kind half is the high 12 of the low 16.
    self.definition_reference(is_tile, name).map(|d| ((d >> 4) & 0xFFF) as u16)
  }

  /// Finalize the name caches after the def vecs are filled (both loaders call this).
  pub(crate) fn index(mut self) -> Self {
    self.tile_names = self.tiles.iter().map(|d| d.name.clone()).collect();
    self.thing_names = self.things.iter().map(|d| d.name.clone()).collect();
    self.biome_names = self.biomes.iter().map(|d| d.name.clone()).collect();
    self.material_names = self.materials.iter().map(|(n, _)| n.clone()).collect();
    self.need_names = self.needs.iter().map(|(n, _)| n.clone()).collect();
    self.condition_names = self.conditions.iter().map(|(n, _)| n.clone()).collect();
    self
  }

  fn id_of(names: &[String], name: &str) -> Option<u16> {
    names.iter().position(|n| n == name && !n.is_empty()).map(|i| i as u16 + 1)
  }
  fn name_of(names: &[String], id: u16) -> Option<&str> {
    (id != 0)
      .then(|| names.get(id as usize - 1))
      .flatten()
      .filter(|n| !n.is_empty())
      .map(String::as_str)
  }

  // ---------- tiles ----------

  /// Every tile name in `def_id` order (index 0 → def_id 1; `""` = a retired id).
  pub fn tile_names(&self) -> &[String] {
    &self.tile_names
  }
  /// The `def_id` for a tile name (1-based; `None` if unknown). This is the u12
  /// packed into a zone's tile slot (`resonantdust_codec::packed::pack_tile`).
  pub fn tile_def_id(&self, name: &str) -> Option<u16> {
    self.registry_id(true, name).or_else(|| Self::id_of(&self.tile_names, name))
  }
  /// The `subtype_id` for a `(type, name)` pair authored in `subtypes.toml`, or `None`.
  ///
  /// This is where a pawn SPECIES gets its number now that the code-owned palette is gone
  /// (F16): `subtype_id_of("pawn", "animal")` is `1`, exactly the value the palette held, so every
  /// stored pawn def keeps reading the same.
  pub fn subtype_id_of(&self, type_name: &str, name: &str) -> Option<u16> {
    self.subtypes.iter().find(|(t, n, _)| t == type_name && n == name).map(|(_, _, id)| *id)
  }

  /// A FNV-1a fingerprint of the fields the **simulation** reads for a tile — its VERSION input
  /// (definition-registry [F12](../../../docs/work/2026-08-04-definition-registry/forks.md#f12)).
  ///
  /// Deliberately excludes `color`, `visual` and the lighting lanes: art, tint and comments do not
  /// bump a version, because the invariant the versioning buys is **"same id ⇒ same behaviour"**,
  /// and behaviour is what the simulation reads. A re-master already propagates through the
  /// texture manifest's own content hash without touching identity — bumping there would mint an
  /// id per art tweak and burn kind space for nothing.
  ///
  /// For a tile that is `height` (walls occlude and block) and `build` (what the panel places).
  pub fn tile_sim_version(&self, def_id: u16) -> Option<u64> {
    let d = self.tiles.get(def_id.checked_sub(1)? as usize)?;
    let mut h = FNV_OFFSET;
    fnv_str(&mut h, &d.name);
    fnv_str(&mut h, d.build.as_deref().unwrap_or(""));
    fnv_bytes(&mut h, &d.height.unwrap_or(0.0).to_bits().to_le_bytes());
    Some(h)
  }

  /// The same for a thing: `speed` (hop cost) and `needs` (what it depletes). Not its art.
  pub fn thing_sim_version(&self, object_id: u16) -> Option<u64> {
    let d = self.things.get(object_id.checked_sub(1)? as usize)?;
    let mut h = FNV_OFFSET;
    fnv_str(&mut h, &d.name);
    fnv_bytes(&mut h, &d.speed.unwrap_or(0).to_le_bytes());
    for n in &d.needs {
      fnv_bytes(&mut h, &n.to_le_bytes());
    }
    Some(h)
  }

  /// A tile's authored VERSION ([F17]) — which revision this def is. `0` when unauthored.
  pub fn tile_version(&self, def_id: u16) -> Option<u32> {
    Some(self.tiles.get(def_id.checked_sub(1)? as usize)?.version)
  }
  /// A thing's authored VERSION ([F17]).
  pub fn thing_version(&self, object_id: u16) -> Option<u32> {
    Some(self.things.get(object_id.checked_sub(1)? as usize)?.version)
  }

  /// A tile's authored TAXONOMY by `def_id`, or `None` if it has none yet ([I9]).
  pub fn tile_taxonomy(&self, def_id: u16) -> Option<&Taxonomy> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.taxonomy.as_ref()
  }
  /// A thing's authored TAXONOMY by `object_id`, or `None` if it has none yet ([I9]).
  pub fn thing_taxonomy(&self, object_id: u16) -> Option<&Taxonomy> {
    self.things.get(object_id.checked_sub(1)? as usize)?.taxonomy.as_ref()
  }

  /// The tile name for a `def_id` (`def_id == 0` is the empty sentinel).
  pub fn tile_name(&self, def_id: u16) -> Option<&str> {
    Self::name_of(&self.tile_names, def_id)
  }
  fn tile_def(&self, name: &str) -> Option<&TileDef> {
    self.tiles.iter().find(|d| d.name == name && !name.is_empty())
  }

  /// A tile's background colour as a packed `0xRRGGBB` — its visual prim's tint
  /// (fallback: a static `color.bg` from the older authored shape).
  pub fn tile_color_bg(&self, name: &str) -> Option<u32> {
    self.tile_def(name)?.color
  }
  /// [`Self::tile_color_bg`] keyed by `def_id`.
  pub fn color_bg_for_def(&self, def_id: u16) -> Option<u32> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.color
  }
  /// The [`VisualParts`] for a tile `def_id`.
  pub fn visual_for_def(&self, def_id: u16) -> Option<VisualParts> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.visual.clone()
  }
  /// Every tile's texture stem in `def_id` order; empty string = no texture.
  pub fn tile_texture_stems(&self) -> Vec<String> {
    self
      .tiles
      .iter()
      .map(|d| d.visual.as_ref().and_then(|v| v.texture.clone()).unwrap_or_default())
      .collect()
  }
  /// A tile's build category (`None` = not buildable).
  pub fn tile_build(&self, def_id: u16) -> Option<String> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.build.clone()
  }
  /// Every tile's build category in `def_id` order; `""` = not buildable
  /// (build-walls D3 — the build menu's scan table).
  pub fn tile_builds(&self) -> Vec<String> {
    self.tiles.iter().map(|d| d.build.clone().unwrap_or_default()).collect()
  }
  /// A tile's LIGHTING + LINKED lanes (texture-generalization P0), flat stride-6 per
  /// def_id: `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]` —
  /// zeros where unauthored.
  pub fn tile_lighting_lanes(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.tiles.len() * 6);
    for d in &self.tiles {
      out.extend_from_slice(&d.lanes);
    }
    out
  }
  /// A tile's HEIGHT in tiles (tile-lighting F2: > 0 opts into the cold lighting
  /// class). `None` = flat ground.
  pub fn tile_height(&self, def_id: u16) -> Option<f64> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.height
  }
  /// Every tile's height in `def_id` order (0 = flat/unauthored).
  pub fn tile_heights(&self) -> Vec<f64> {
    self.tiles.iter().map(|d| d.height.unwrap_or(0.0)).collect()
  }
  /// Every tile's 4 packed-channel material bindings in `def_id` order.
  pub fn tile_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self.tiles.iter().map(|d| d.visual.as_ref().map(|v| v.packed).unwrap_or_default()).collect()
  }

  // ---------- things ----------

  /// Every thing name in `object_id` order (index 0 → object_id 1; `""` = retired).
  pub fn thing_names(&self) -> &[String] {
    &self.thing_names
  }
  /// The `object_id` for a thing name (1-based; `None` if unknown) — the u12 packed
  /// into a zone's thing entry.
  pub fn thing_object_id(&self, name: &str) -> Option<u16> {
    self.registry_id(false, name).or_else(|| Self::id_of(&self.thing_names, name))
  }
  /// The thing name for an `object_id` (`0` is the empty sentinel).
  pub fn thing_name(&self, object_id: u16) -> Option<&str> {
    Self::name_of(&self.thing_names, object_id)
  }
  fn thing_def(&self, name: &str) -> Option<&ThingDef> {
    self.things.iter().find(|d| d.name == name && !name.is_empty())
  }

  /// A thing's colour as a packed `0xRRGGBB` — its visual prim tint.
  pub fn thing_color_bg(&self, name: &str) -> Option<u32> {
    self.thing_def(name)?.color
  }
  /// [`Self::thing_color_bg`] keyed by `object_id`.
  pub fn thing_color_for_object(&self, object_id: u16) -> Option<u32> {
    self.things.get(object_id.checked_sub(1)? as usize)?.color
  }
  /// The [`VisualParts`] for a thing `object_id` — cold things and pawns alike.
  pub fn visual_for_object(&self, object_id: u16) -> Option<VisualParts> {
    self.things.get(object_id.checked_sub(1)? as usize)?.visual.clone()
  }
  /// Every thing's texture stem in `object_id` order; `""` for a def with none.
  pub fn thing_texture_stems(&self) -> Vec<String> {
    self
      .things
      .iter()
      .map(|d| d.visual.as_ref().and_then(|v| v.texture.clone()).unwrap_or_default())
      .collect()
  }

  /// Every thing's spatial LAYOUT in `object_id` order, flattened **stride-10** per def:
  /// `[footprint.w, footprint.h, anchor.x, anchor.y, size, sprite_anchor.x,
  /// sprite_anchor.y, span, sprite_scale.w, sprite_scale.h]`. A def with no prim gets
  /// the default row `[1, 1, 0.5, 0.5, 1, 0.5, 0.5, 1, 1, 1]`, so the host never
  /// special-cases "unset".
  pub fn thing_layout(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * 10);
    for d in &self.things {
      match &d.visual {
        Some(v) => out.extend_from_slice(&[
          v.footprint.0, v.footprint.1, v.anchor.0, v.anchor.1, v.size, v.sprite_anchor.0, v.sprite_anchor.1,
          v.span, v.sprite_scale.0, v.sprite_scale.1,
        ]),
        None => out.extend_from_slice(&[1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0]),
      }
    }
    out
  }

  /// Every thing's SUBFRAMES in `object_id` order, flattened **stride-1536** per def:
  /// [`VARIANTS_PER_DEF`]` × `[`ROTATIONS_PER_DEF`]` × [sub.x, sub.y, sub.w, sub.h,
  /// anchor.x, anchor.y]`, all fractions. This is what the atlas CROPS to
  /// (subframe-ingest): one rect, four maps, registered by construction.
  pub fn thing_subframe(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * VARIANTS_PER_DEF * ROTATIONS_PER_DEF * 6);
    for d in &self.things {
      let frames = d.visual.as_ref().map(|v| v.dir_frames).unwrap_or(
        [[DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: (0.5, 0.5) }; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
      );
      for by_rot in frames {
        for f in by_rot {
          out.extend_from_slice(&[f.sub.0, f.sub.1, f.sub.2, f.sub.3, f.anchor.0, f.anchor.1]);
        }
      }
    }
    out
  }

  /// Every thing's emitted LIGHT in `object_id` order, flattened **stride-8** per def:
  /// `[r, g, b, intensity, reach, radius, height, flags]` (`flags` bit 0 = cast,
  /// bit 1 = hot, bit 2 = flicker). **`reach == 0` IS the "no light" test.**
  pub fn thing_light(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * 8);
    for d in &self.things {
      match d.visual.as_ref().and_then(|v| v.light) {
        Some(l) => out.extend_from_slice(&[
          l.color.0, l.color.1, l.color.2, l.intensity, l.reach, l.radius, l.height,
          f64::from(u8::from(l.cast) | (u8::from(l.hot) << 1) | (u8::from(l.flicker) << 2)),
        ]),
        None => out.extend_from_slice(&[0.0; 8]),
      }
    }
    out
  }

  /// A thing's movement speed in **tics per tile** (pawn-movement F1: speed is
  /// content, measured in tics). `None` when unauthored — the caller resolves the
  /// default (`codec::speed`); the corpus never invents one.
  pub fn thing_speed(&self, object_id: u16) -> Option<u16> {
    self.things.get(object_id.checked_sub(1)? as usize)?.speed
  }
  /// Every thing's speed in `object_id` order, `0` = unauthored.
  pub fn thing_speeds(&self) -> Vec<f64> {
    self.things.iter().map(|d| f64::from(d.speed.unwrap_or(0))).collect()
  }
  /// Every thing's 4 packed-channel material bindings in `object_id` order.
  pub fn thing_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self.things.iter().map(|d| d.visual.as_ref().map(|v| v.packed).unwrap_or_default()).collect()
  }

  /// The needs a thing kind carries, as 1-based need ids (needs-moodlets P1).
  pub fn thing_needs(&self, object_id: u16) -> Vec<u16> {
    self
      .things
      .get(object_id.checked_sub(1).map(usize::from).unwrap_or(usize::MAX))
      .map(|d| d.needs.clone())
      .unwrap_or_default()
  }
  /// Every thing's needs in `object_id` order, flattened **stride-[`NEEDS_PER_KIND`]**
  /// per kind (`0` = empty slot).
  pub fn thing_needs_table(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * NEEDS_PER_KIND);
    for d in &self.things {
      for i in 0..NEEDS_PER_KIND {
        out.push(d.needs.get(i).copied().unwrap_or(0) as f64);
      }
    }
    out
  }

  // ---------- biomes ----------

  /// Every biome name in evaluation order (first match wins).
  pub fn biome_names(&self) -> &[String] {
    &self.biome_names
  }
  /// A biome's stable `subtype_id` — its identity in stored zones (explicit, never
  /// derived from order). `None` if unauthored or `0` (the reserved subtype).
  pub fn biome_subtype_id(&self, name: &str) -> Option<u16> {
    let d = self.biomes.iter().find(|b| b.name == name)?;
    (d.subtype > 0).then_some(d.subtype)
  }

  /// The first biome whose condition passes for these `dims`, in evaluation order.
  /// (`seed` kept in the signature — classification is dims-pure today, but the seed
  /// belongs to the per-tile contract worldgen calls with.)
  pub fn select_biome(&self, dims: &[f64], _seed: u64) -> Option<&str> {
    self.biomes.iter().find_map(|b| self.biome_matches(b, dims).then_some(b.name.as_str()))
  }

  fn biome_matches(&self, b: &BiomeDef, dims: &[f64]) -> bool {
    match &b.body {
      BiomeBody::Rules(r) => r
        .when
        .iter()
        .all(|&(dim, cmp, t)| cmp.pass(dims.get(dim).copied().unwrap_or(0.0), t)),
    }
  }

  /// Classify one tile end to end: pick its biome by `dims`, then read the ground
  /// `tile` + any `thing1` its body places (seeded scatter). The per-tile worldgen
  /// entry point — content decides *what*, worldgen packs it.
  pub fn generate(&self, dims: &[f64], seed: u64) -> GenTile {
    let Some(b) = self.biomes.iter().find(|b| self.biome_matches(b, dims)) else {
      return GenTile::default();
    };
    let mut gen = GenTile { biome: Some(b.name.clone()), ..GenTile::default() };
    match &b.body {
      BiomeBody::Rules(r) => {
        gen.tile = r.tile.clone();
        // Ordered draws, LAST hit wins (least→most dominant, as authored).
        for (salt, p, thing) in &r.scatter {
          if tile_rand(seed, *salt) < *p {
            gen.thing1 = Some(thing.clone());
          }
        }
      }
    }
    gen
  }

  // ---------- materials ----------

  /// Every material name in `material_id` order (index 0 → id 1).
  pub fn material_names(&self) -> &[String] {
    &self.material_names
  }
  /// The 1-based `material_id` for a name (`None` if unknown).
  pub fn material_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.material_names, name)
  }
  /// The material name for a `material_id` (`0` is the empty sentinel).
  pub fn material_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.material_names, id)
  }
  /// A material's [`MaterialParams`], or `None` if unknown.
  pub fn material_params(&self, name: &str) -> Option<MaterialParams> {
    self.materials.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole material registry in `material_id` order.
  pub fn material_params_all(&self) -> Vec<MaterialParams> {
    self.materials.iter().map(|(_, p)| p.clone()).collect()
  }

  // ---------- needs & conditions (needs-moodlets P1) ----------

  /// Every need name in `need_id` order (index 0 → id 1).
  pub fn need_names(&self) -> &[String] {
    &self.need_names
  }
  /// The 1-based `need_id` for a name (`None` if unknown).
  pub fn need_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.need_names, name)
  }
  /// The need name for a `need_id` (`0` is the empty sentinel).
  pub fn need_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.need_names, id)
  }
  /// A need's [`NeedParams`], or `None` if unknown.
  pub fn need_params(&self, name: &str) -> Option<NeedParams> {
    self.needs.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole need registry in `need_id` order.
  pub fn need_params_all(&self) -> Vec<NeedParams> {
    self.needs.iter().map(|(_, p)| p.clone()).collect()
  }

  /// Every condition name in `condition_id` order (index 0 → id 1).
  pub fn condition_names(&self) -> &[String] {
    &self.condition_names
  }
  /// The 1-based `condition_id` for a name (`None` if unknown).
  pub fn condition_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.condition_names, name)
  }
  /// The condition name for a `condition_id` (`0` is the empty sentinel).
  pub fn condition_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.condition_names, id)
  }
  /// A condition's [`ConditionParams`], or `None` if unknown.
  pub fn condition_params(&self, name: &str) -> Option<ConditionParams> {
    self.conditions.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole condition registry in `condition_id` order.
  pub fn condition_params_all(&self) -> Vec<ConditionParams> {
    self.conditions.iter().map(|(_, p)| p.clone()).collect()
  }
}

/// Parse every `(name, source)` — all `.toml` — into a [`Bundle`]. A non-TOML source
/// name is a load error (the `.rd` dialect is deleted; git holds its history).
pub fn load(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  if let Some((name, _)) = sources.iter().find(|(n, _)| !n.ends_with(".toml")) {
    return Err(vec![LoadError {
      file: name.clone(),
      message: "not a .toml source — the corpus is TOML (the .rd DSL was deleted 2026-08-04)".into(),
    }]);
  }
  crate::toml_loader::load_toml(sources)
}

/// The kind's emitted light — see [`VisualParts::light`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightParts {
  pub color: (f64, f64, f64),
  pub intensity: f64,
  pub reach: f64,
  pub radius: f64,
  pub height: f64,
  /// Casts shadows (default true). A fill light that lights without occluding costs
  /// the gather nothing — it is skipped in the shadow walk entirely.
  pub cast: bool,
  /// Animates per frame ⇒ the HOT class, re-baked every frame. Default false.
  pub hot: bool,
  /// Emits DECAY-LIGHTMAP flicker particles (lighting-feel P2). Orthogonal to `hot`.
  pub flicker: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn non_toml_sources_refuse_to_load() {
    let e = load(&[("data/tiles.rd".to_string(), "<tile>".to_string())]).unwrap_err();
    assert!(e[0].message.contains("TOML"), "{}", e[0].message);
    assert_eq!(e[0].file, "data/tiles.rd");
  }

  #[test]
  fn tile_rand_is_the_documented_derivation() {
    // Pinned: SplitMix64 over seed ^ salt*phi. Distinct salts decorrelate; same
    // inputs reproduce (the worldgen sweep in tests/golden.rs leans on this).
    assert_eq!(tile_rand(42, 6), tile_rand(42, 6));
    assert_ne!(tile_rand(42, 6), tile_rand(42, 7));
    assert_ne!(tile_rand(42, 6), tile_rand(43, 6));
    let r = tile_rand(123456789, 9);
    assert!((0.0..1.0).contains(&r));
  }
}
