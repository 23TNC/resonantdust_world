//! The TOML corpus loader (toml-content P2) — `content/*.toml` → the same materialized
//! [`Bundle`] the `.rd` path produced, byte-identical under the golden oracle (F5).
//!
//! Schema: `docs/VARIABLES.md § TOML content schema` — it outranks this file on layouts.
//! The ID LAW (F1) is enforced here: every def authors `id = N` (1-based); a duplicate,
//! a zero, or a missing id REFUSES the load. Holes are legal (a retired id keeps its
//! slot as an empty placeholder forever).

use crate::loader::{
  AffordanceCheck, AffordanceParams, BiomeBody, BiomeDef, BiomeRules, Bundle, Cmp,
  ConditionParams, DirFrame, EmotionModifier, EmotionParams, InteractionParams,
  LoadError, MaterialParams, MoveEffect, NeedBand, NeedModifier, NeedParams, Operand,
  PackedChannel, SatisfyEffect, SpawnEffect, StatModifier, StatParams, Taxonomy, ThingDef,
  TileDef, TraitBind, TraitLevel, TraitLight, TraitParams, VisualPart, VisualParts,
  NEEDS_PER_KIND, ROTATIONS_PER_DEF, VARIANTS_PER_DEF,
};
use serde::Deserialize;
use std::collections::HashMap;

// ── the schema (serde mirrors of VARIABLES.md) ─────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Corpus {
  #[serde(default)]
  tile: Vec<TileToml>,
  #[serde(default)]
  thing: Vec<ThingToml>,
  #[serde(default)]
  biome: Vec<BiomeToml>,
  #[serde(default)]
  material: Vec<MaterialToml>,
  #[serde(default)]
  need: Vec<NeedToml>,
  #[serde(default)]
  condition: Vec<ConditionToml>,
  /// RETIRED (trait-rows-u32 F2): parsed only to REFUSE with a migration message.
  #[serde(default, rename = "trait")]
  trait_: Vec<TraitToml>,
  /// RETIRED (trait-rows-u32 F2): parsed only to REFUSE with a migration message.
  #[serde(default)]
  player_trait: Vec<TraitToml>,
  // The SIX live trait categories (trait-rows-u32 F2), subtype order 8..13.
  #[serde(default)]
  pawn_trait_constant: Vec<TraitToml>,
  #[serde(default)]
  pawn_trait_active: Vec<TraitToml>,
  #[serde(default)]
  pawn_trait_passive: Vec<TraitToml>,
  #[serde(default)]
  player_trait_constant: Vec<TraitToml>,
  #[serde(default)]
  player_trait_active: Vec<TraitToml>,
  #[serde(default)]
  player_trait_passive: Vec<TraitToml>,
  /// Brain defs (npc-host F5) — an npc module's constants as content.
  #[serde(default)]
  brain: Vec<BrainToml>,
  #[serde(default)]
  interaction: Vec<InteractionToml>,
  #[serde(default)]
  affordance: Vec<AffordanceToml>,
  #[serde(default)]
  stat: Vec<StatToml>,
  #[serde(default)]
  emotion: Vec<EmotionToml>,
  #[serde(default)]
  subtype: Vec<SubtypeToml>,
}

/// `(type, name) → subtype_id` for a subtype axis with no record of its own — pawn species today
/// (definition-registry F16). A biome authors its own `subtype` on the biome record instead.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubtypeToml {
  #[serde(rename = "type")]
  type_name: String,
  name: String,
  id: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TileToml {
  name: String,
  /// Which REVISION of this definition ([F17](../../../docs/work/2026-08-04-definition-registry/forks.md#f17)).
  /// Unauthored = 0. Every live version stays authored — an entity holding an old id gets its
  /// behaviour from the block that is still here, which is the whole point ([B6]).
  #[serde(default)]
  version: u32,
  // ── taxonomy (definition-registry F1) — additive; `id` remains the allocation SEED (I9) ──
  #[serde(default, rename = "type")]
  type_name: Option<String>,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default, rename = "subType")]
  sub_type: Vec<String>,
  #[serde(default)]
  variant: Vec<String>,
  #[serde(default)]
  texture: Option<String>,
  #[serde(default)]
  tint: Option<String>,
  #[serde(default)]
  geo: Option<String>,
  #[serde(default)]
  height: Option<f64>,
  #[serde(default)]
  build: Option<String>,
  // Numeric lighting MODES, not booleans — the corpus authors `receives_shadows = 2`
  // (the ground mode). Passed through to the lanes untouched.
  #[serde(default)]
  cast_shadow: Option<f64>,
  #[serde(default)]
  receives_shadows: Option<f64>,
  #[serde(default)]
  rotation: Option<f64>,
  #[serde(default)]
  linked: Option<LinkedToml>,
  #[serde(default)]
  padding: Option<f64>,
  #[serde(default)]
  packed: Vec<PackedToml>,
  /// Interaction bindings (stat-model F5/F9) — the water tile's `drink 3`.
  #[serde(default)]
  interactions: Vec<InteractionBindToml>,
  /// May a pawn ENTER this tile? ABSENCE = true (pathfinding F1) — water authors false.
  #[serde(default = "yes")]
  pathable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkedToml {
  w: f64,
  h: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackedToml {
  #[serde(default)]
  material: Option<String>,
  #[serde(default)]
  tint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThingToml {
  name: String,
  /// Which REVISION of this definition ([F17](../../../docs/work/2026-08-04-definition-registry/forks.md#f17)).
  /// Unauthored = 0. Every live version stays authored — an entity holding an old id gets its
  /// behaviour from the block that is still here, which is the whole point ([B6]).
  #[serde(default)]
  version: u32,
  // ── taxonomy (definition-registry F1) — additive; `id` remains the allocation SEED (I9) ──
  #[serde(default, rename = "type")]
  type_name: Option<String>,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default, rename = "subType")]
  sub_type: Vec<String>,
  #[serde(default)]
  variant: Vec<String>,
  #[serde(default)]
  needs: Vec<String>,
  /// STARTING trait bindings (stat-model F11) — a bare string = level 1.
  #[serde(default)]
  traits: Vec<TraitBindToml>,
  /// Interaction bindings (stat-model F5/F9) — a carried thing's drink source, later.
  #[serde(default)]
  interactions: Vec<InteractionBindToml>,
  #[serde(default)]
  packed: Vec<PackedToml>,
  #[serde(default)]
  part: Vec<PartToml>,
  /// May a pawn ENTER a cell this thing occupies? ABSENCE = true (pathfinding F1) —
  /// the TREE authors false; the composed view (kind-0 suppresses) decides occupancy.
  #[serde(default = "yes")]
  pathable: bool,
  /// The GEO-TIER glyph (survival F1). Absent = the name's first char, uppercased.
  #[serde(default)]
  geo_label: Option<String>,
}

fn one() -> f64 { 1.0 }
fn quarter() -> f64 { 0.25 }
fn half() -> f64 { 0.5 }
fn yes() -> bool { true }
fn centre() -> f64 { 0.5 }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct XY {
  #[serde(default = "centre")]
  x: f64,
  #[serde(default = "centre")]
  y: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WH {
  #[serde(default = "one")]
  w: f64,
  #[serde(default = "one")]
  h: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartToml {
  #[serde(default)]
  texture: Option<String>,
  #[serde(default)]
  tint: Option<String>,
  #[serde(default)]
  geo: Option<String>,
  #[serde(default)]
  footprint: Option<WH>,
  #[serde(default)]
  anchor: Option<XY>,
  #[serde(default)]
  sprite_anchor: Option<XY>,
  #[serde(default)]
  size: Option<f64>,
  #[serde(default)]
  span: Option<f64>,
  /// The PART scale (a pawn slot's art scale — `&body.scale`); distinct from
  /// `sprite_scale`, the pre-atlas ingest pair. TWO channels, as the corpus authors them.
  #[serde(default)]
  scale: Option<f64>,
  #[serde(default)]
  sprite_scale: Option<WH>,
  #[serde(default)]
  part: Option<u32>,
  #[serde(default)]
  depth: Option<f64>,
  #[serde(default)]
  offset: Option<Offset>,
  /// Subframe entries keyed by the fallback chain's spellings: `default`,
  /// `s`/`e`/`n`/`w` or `r0`..`r15`, `v0`..`v15`, or fully-specific `v3.r1`/`v3.e`.
  #[serde(default)]
  subframe: HashMap<String, SubframeRect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offset {
  #[serde(default)]
  x: f64,
  #[serde(default)]
  y: f64,
  #[serde(default)]
  z: f64,
}

/// One authored rect — every component OPTIONAL so the per-COMPONENT fallback the `.rd`
/// chain had survives (an entry may set only `x`, inheriting the rest from less-specific
/// keys).
#[derive(Deserialize, Clone, Copy, Default)]
#[serde(deny_unknown_fields)]
struct SubframeRect {
  x: Option<f64>,
  y: Option<f64>,
  w: Option<f64>,
  h: Option<f64>,
  ax: Option<f64>,
  ay: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BiomeToml {
  name: String,
  #[serde(default)]
  subtype: u16,
  #[serde(default)]
  when: HashMap<String, CondToml>,
  #[serde(default)]
  tile: Option<String>,
  #[serde(default)]
  scatter: Vec<ScatterToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CondToml {
  gte: Option<f64>,
  gt: Option<f64>,
  lt: Option<f64>,
  lte: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScatterToml {
  salt: i64,
  p: f64,
  thing: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialToml {
  id: u16,
  name: String,
  #[serde(default)]
  noise_field: Option<String>,
  #[serde(default)]
  hue_swing: Option<f64>,
  #[serde(default)]
  chroma_swing: Option<f64>,
  #[serde(default)]
  warm_cool_bias: Option<f64>,
  #[serde(default)]
  sample_space: Option<String>,
  #[serde(default)]
  detail: Option<DetailToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailToml {
  field: String,
  #[serde(default)]
  amp: f64,
  #[serde(default = "one")]
  scale: f64,
}

// ── the gameplay categories (interactions F1/F9) — identity is the derived
// `gameplay/<category>/<name>/default` taxonomy; NO ids and NO authored taxonomy fields.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NeedToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// The authored value domain (interactions F7) — min may be negative (deficit);
  /// defaults `0..1` (the pre-F7 fractional domain). ALSO the fixed-point encoding
  /// domain (stat-model F4).
  #[serde(default)]
  min: f64,
  #[serde(default = "one")]
  max: f64,
  /// The empty-intersection rule for modifier ranges (stat-model F6): `"min"` (default)
  /// or `"max"`.
  #[serde(default)]
  winner: Option<String>,
  #[serde(default)]
  deplete: f64,
  #[serde(default)]
  band: Vec<BandToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// Authored GLOBAL safety bounds (stat-model F6) — the outermost clamp.
  #[serde(default)]
  min: f64,
  #[serde(default = "one")]
  max: f64,
  #[serde(default)]
  winner: Option<String>,
}

/// A condition's SCALAR stat modifier (conditions have no levels).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModStatToml {
  stat: String,
  #[serde(default)]
  add: f64,
  #[serde(default)]
  min: Option<f64>,
  #[serde(default)]
  max: Option<f64>,
}

/// A condition's scalar need modifier — `rate` multiplies depletion (default 1.0).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModNeedToml {
  need: String,
  #[serde(default = "one")]
  rate: f64,
  #[serde(default)]
  min: Option<f64>,
  #[serde(default)]
  max: Option<f64>,
  /// survival F3: a DEPLETION MODIFIER — the need's own field and units (TICS
  /// full→empty): "while this condition is active, the need depletes at this pace".
  /// Sources combine as rates summing. How a deplete-0 need (corpus) moves at all.
  #[serde(default)]
  deplete: Option<f64>,
}

/// A trait's LEVELED stat modifier — arrays index level − 1 (stat-model F5); every
/// authored array within one trait must agree on length (the trait's level count).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelStatToml {
  stat: String,
  #[serde(default)]
  add: Vec<f64>,
  #[serde(default)]
  min: Vec<f64>,
  #[serde(default)]
  max: Vec<f64>,
}

/// A trait's leveled need modifier.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelNeedToml {
  need: String,
  #[serde(default)]
  rate: Vec<f64>,
  #[serde(default)]
  min: Vec<f64>,
  #[serde(default)]
  max: Vec<f64>,
  /// survival F3: per-LEVEL depletion modifiers (see ModNeedToml.deplete).
  #[serde(default)]
  deplete: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandToml {
  condition: String,
  #[serde(default)]
  lo: f64,
  hi: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  #[serde(default)]
  duration: f64,
  /// Card sort key, descending; absent = 0. See [`ConditionParams::priority`].
  #[serde(default)]
  priority: i32,
  /// Stat contributions while active (stat-model F5/F6).
  #[serde(default)]
  stats: Vec<ModStatToml>,
  /// Need modifiers while active — quenched's `rate = 0.5` on thirst.
  #[serde(default)]
  needs: Vec<ModNeedToml>,
  /// Emotion contributions while active (emotions F2; MOOD is retired, F4).
  #[serde(default)]
  emotions: Vec<ModEmotionToml>,
}

/// One emotion contribution — a scalar on conditions, a per-LEVEL array on traits
/// (emotions F2). Magnitudes are 1..=15 (the u4 bound; +0 = author nothing).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModEmotionToml {
  emotion: String,
  magnitude: toml::Value,
}

/// One `[[emotion]]` def (emotions F1) — SIXTEEN max, declaration-ordered, `fine` first.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmotionToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  color: String,
}

/// A `[[brain]]` def (npc-host F5, VARIABLES.md §Brains): needs + CONSTANT player-trait
/// binds only — no visuals, no parts; never a world object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrainToml {
  name: String,
  #[serde(default)]
  version: u32,
  #[serde(default, rename = "type")]
  type_name: Option<String>,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default, rename = "subType")]
  sub_type: Vec<String>,
  #[serde(default)]
  variant: Vec<String>,
  #[serde(default)]
  needs: Vec<String>,
  /// CONSTANT player-trait binds ONLY (validated at load).
  #[serde(default)]
  traits: Vec<TraitBindToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// Per-LEVEL stat contributions (stat-model F5) — walks' tics/tile table.
  #[serde(default)]
  stats: Vec<LevelStatToml>,
  /// Per-level need modifiers.
  #[serde(default)]
  needs: Vec<LevelNeedToml>,
  /// Per-level emotion contributions (emotions F2) — `magnitude = [1, 2, …]`.
  #[serde(default)]
  emotions: Vec<ModEmotionToml>,
  /// Capability TAGS (attack F1): free-form strings a TAG-check affordance matches
  /// (`bite` authors `["attack"]`). Not leveled — presence is the capability.
  #[serde(default)]
  tags: Vec<String>,
  /// Per-LEVEL emitted light (trait-lights F4) — one entry per level, under the same
  /// array-agreement law as every per-level table. Empty = the trait never emits.
  #[serde(default)]
  emit_light: Vec<TraitLightToml>,
}

/// A light color: `"#rrggbb"` for authoring comfort, or `[r, g, b]` floats when the
/// exact channel values matter (the torch conversion held the old block's floats
/// bit-identically — a hex byte cannot express 0.85).
#[derive(Deserialize)]
#[serde(untagged)]
enum LightColorToml {
  Hex(String),
  Rgb([f64; 3]),
}

/// One authored light level (trait-lights F4). Defaults mirror the old visual light
/// block's (`radius` 0.25, `elevation` — its `height` — 0.5, `cast` true).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitLightToml {
  color: LightColorToml,
  #[serde(default = "one")]
  intensity: f64,
  reach: f64,
  /// Authored-not-yet-consumed (trait-lights I10) — carried to the render's successor.
  #[serde(default = "one")]
  fall_off: f64,
  #[serde(default = "half")]
  elevation: f64,
  #[serde(default = "quarter")]
  radius: f64,
  #[serde(default = "yes")]
  cast: bool,
  #[serde(default)]
  hot: bool,
  #[serde(default)]
  flicker: bool,
}

/// One effect operand: `"@name"` = a reference into the interaction's `inputs`; a bare
/// string = a baked name; a number = a baked value (interactions F5).
#[derive(Deserialize)]
#[serde(untagged)]
enum OperandToml {
  Num(f64),
  Str(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SatisfyToml {
  target: OperandToml,
  need: OperandToml,
  amount: OperandToml,
}

/// The move effect (input-rework F6): walk `target` to `to`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveToml {
  target: OperandToml,
  to: OperandToml,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractionToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// The pie-menu label (input-rework F1); default = label.
  #[serde(default)]
  menu_text: Option<String>,
  /// The affordance GATES (stat-model F5) — every listed predicate must pass.
  #[serde(default)]
  affordances: Vec<String>,
  /// The input SIGNATURE — event input words bind these in order (F5).
  #[serde(default)]
  inputs: Vec<String>,
  #[serde(default)]
  satisfy: Option<SatisfyToml>,
  /// The move effect (input-rework F6). At least one of satisfy/move must be authored.
  #[serde(default, rename = "move")]
  move_: Option<MoveToml>,
  /// TIMED condition grants on execute.
  #[serde(default)]
  grant: Vec<String>,
  /// The destroy effect (lumberjack F5): `"carrier"` is the only value — clear the
  /// validated offerer's cell. `yields` is the RESERVED successor (I9), not a field yet.
  #[serde(default)]
  destroy: Option<String>,
  /// The spawn effect (food-chain F5/F6): `{ thing, at = "on"|"adjacent" }`.
  #[serde(default)]
  spawn: Option<SpawnToml>,
  /// The remove effect (food-chain F5): `"target"` is the only value.
  #[serde(default)]
  remove: Option<String>,
  /// The store effect (inventory F4): `"carrier"` is the only value — the offerer
  /// leaves the world and lands in the acting pawn's first free inventory slot.
  #[serde(default)]
  store: Option<String>,
  /// The placement rule (input-rework F4 / lumberjack F2): `"on"` | `"adjacent"` | `"target"`.
  #[serde(default = "on")]
  location: String,
  /// TICS this interaction takes; 0 = instantaneous, N > 0 completes (and re-validates)
  /// at +N (lumberjack — the I9 reservation consumed).
  #[serde(default)]
  duration: f64,
  /// The intent-queue DISPLAY block (intent-queue-ui F2).
  #[serde(default)]
  queue: Option<QueueToml>,
}

/// The `queue = { … }` display block — all fields optional (intent-queue-ui F2).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct QueueToml {
  #[serde(default)]
  hover: Option<String>,
  #[serde(default)]
  size: Option<f64>,
  #[serde(default)]
  background: Option<String>,
  #[serde(default)]
  progress: Option<String>,
  #[serde(default)]
  progress_color: Option<String>,
  #[serde(default)]
  progress_fill: Option<bool>,
  #[serde(default)]
  cancelable: Option<bool>,
}

fn on() -> String {
  "on".into()
}

/// An affordance is a structured stat PREDICATE (stat-model F5/F10) — never an
/// expression string. Exactly one of `above`/`below` (both EXCLUSIVE).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffordanceToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  check: CheckToml,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckToml {
  /// The STAT form (stat-model F10) — exactly one of `above`/`below`.
  #[serde(default)]
  stat: Option<String>,
  #[serde(default)]
  above: Option<f64>,
  #[serde(default)]
  below: Option<f64>,
  /// The NEED form (food-chain F4) — exactly one of gte/gt/lt/lte. A need check is
  /// ALSO the need-write trigger key (F5).
  #[serde(default)]
  need: Option<String>,
  #[serde(default)]
  gte: Option<f64>,
  #[serde(default)]
  gt: Option<f64>,
  #[serde(default)]
  lt: Option<f64>,
  #[serde(default)]
  lte: Option<f64>,
  /// The TAG form (attack F1): passes iff ANY carried trait authors this tag.
  #[serde(default)]
  tag: Option<String>,
}

/// The spawn effect's TOML shape — a `{ thing, at }` table (food-chain F5/F6) or the
/// string `"carried"` (inventory F5: spawn the acting pawn's SLOT item beside it).
#[derive(Deserialize)]
#[serde(untagged)]
enum SpawnToml {
  Carried(String),
  Thing(SpawnThingToml),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnThingToml {
  thing: String,
  at: String,
}

/// A CARRIER's interaction binding — what it offers, plus its parameters (stat-model F9).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractionBindToml {
  name: String,
  #[serde(default)]
  magnitude: f64,
  /// The thing a destroy-effect interaction leaves at this carrier's cell
  /// (logs-drop F1) — must name a `[[thing]]` kind.
  #[serde(default)]
  yields: Option<String>,
}

/// A thing's STARTING trait binding (stat-model F11): a bare string (level 1) or
/// `{ name, level }`.
#[derive(Deserialize)]
#[serde(untagged)]
enum TraitBindToml {
  Name(String),
  Full {
    name: String,
    /// The bound TIER = the def's VARIANT (trait-rows-u32 F6, 0-BASED). `level` and
    /// `constant` are RETIRED — the loader refuses them by deny_unknown_fields; constancy
    /// is the def's CATEGORY.
    #[serde(default)]
    variant: u16,
  },
}

impl TraitBindToml {
  /// The full resolved bind: `(name, variant)` — a bare string is variant 0.
  fn bind(&self) -> (String, u16) {
    match self {
      TraitBindToml::Name(n) => (n.clone(), 0),
      TraitBindToml::Full { name, variant } => (name.clone(), *variant),
    }
  }
}

// ── loading ────────────────────────────────────────────────────────────────────────

/// Parse every `.toml` source and build the one [`Bundle`]. Category arrays merge
/// across files (append); the id law is enforced per registry.
pub(crate) fn load_toml(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  let mut errors = Vec::new();
  let mut all = Corpus::default();
  for (name, text) in sources {
    match toml::from_str::<Corpus>(text) {
      Ok(c) => {
        all.tile.extend(c.tile);
        all.thing.extend(c.thing);
        all.biome.extend(c.biome);
        all.material.extend(c.material);
        all.need.extend(c.need);
        all.condition.extend(c.condition);
        all.trait_.extend(c.trait_);
        all.player_trait.extend(c.player_trait);
        all.pawn_trait_constant.extend(c.pawn_trait_constant);
        all.pawn_trait_active.extend(c.pawn_trait_active);
        all.pawn_trait_passive.extend(c.pawn_trait_passive);
        all.player_trait_constant.extend(c.player_trait_constant);
        all.player_trait_active.extend(c.player_trait_active);
        all.player_trait_passive.extend(c.player_trait_passive);
        all.brain.extend(c.brain);
        all.interaction.extend(c.interaction);
        all.affordance.extend(c.affordance);
        all.stat.extend(c.stat);
        all.emotion.extend(c.emotion);
        all.subtype.extend(c.subtype);
      }
      Err(e) => errors.push(LoadError { file: name.clone(), message: format!("toml: {e}") }),
    }
  }
  if !errors.is_empty() {
    return Err(errors);
  }

  let mut b = Bundle::default();

  // Materials keep the explicit id law (a RENDER registry — nothing numbers them but the
  // corpus). The five GAMEPLAY categories carry NO ids (interactions F1): identity is the
  // registry-allocated `gameplay/<category>/<name>/default` tuple, with corpus position only
  // the allocation SEED — so here a NAME must be unique per category, the same aliasing rule
  // the (taxonomy, version) check enforces for tiles/things.
  let materials = place(&all.material, "material", |m| (m.id, m.name.clone()), &mut errors);
  let mut unique = |category: &str, names: Vec<&str>| {
    let mut seen = std::collections::HashSet::new();
    for n in names {
      if !seen.insert(n.to_string()) {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "{category} `{n}` is defined twice — a gameplay def's name IS its kind \
             (gameplay/{category}/{n}), so two defs cannot share it"
          ),
        });
      }
    }
  };
  unique("need", all.need.iter().map(|d| d.name.as_str()).collect());
  unique("condition", all.condition.iter().map(|d| d.name.as_str()).collect());
  unique("brain", all.brain.iter().map(|d| d.name.as_str()).collect());
  // ONE namespace across the six live categories (trait-rows-u32 F2).
  unique(
    "trait (any category)",
    all
      .pawn_trait_constant
      .iter()
      .chain(&all.pawn_trait_active)
      .chain(&all.pawn_trait_passive)
      .chain(&all.player_trait_constant)
      .chain(&all.player_trait_active)
      .chain(&all.player_trait_passive)
      .map(|d| d.name.as_str())
      .collect(),
  );
  unique("interaction", all.interaction.iter().map(|d| d.name.as_str()).collect());
  unique("affordance", all.affordance.iter().map(|d| d.name.as_str()).collect());
  unique("stat", all.stat.iter().map(|d| d.name.as_str()).collect());
  unique("emotion", all.emotion.iter().map(|d| d.name.as_str()).collect());
  drop(unique);
  // The RETIRED tables refuse with a migration message (trait-rows-u32 F2).
  for d in all.trait_.iter().chain(&all.player_trait) {
    errors.push(LoadError {
      file: String::new(),
      message: format!(
        "`[[trait]]`/`[[player_trait]]` are RETIRED (trait-rows-u32): re-author `{}` under          [[pawn_trait_constant|active|passive]] or [[player_trait_constant|active|passive]]",
        d.name
      ),
    });
  }
  // Tiles and things carry NO ids (definition-registry F1/F15): the corpus describes and the
  // server numbers. Their position here is only the SEED a fresh registry allocates from — an
  // existing registry overrides it through `Bundle::with_registry`, which is what makes a
  // reorder harmless.
  let subtypes: Vec<(String, String, u16)> =
    all.subtype.iter().map(|s| (s.type_name.clone(), s.name.clone(), s.id)).collect();
  let tiles: Vec<Option<&TileToml>> = all.tile.iter().map(Some).collect();
  let things: Vec<Option<&ThingToml>> = all.thing.iter().map(Some).collect();
  if !errors.is_empty() {
    return Err(errors);
  }

  b.materials = materials
    .iter()
    .map(|slot| match slot {
      Some(m) => (m.name.clone(), MaterialParams {
        noise_field: m.noise_field.clone().unwrap_or_default(),
        hue_swing: m.hue_swing.unwrap_or(0.0),
        chroma_swing: m.chroma_swing.unwrap_or(0.0),
        warm_cool_bias: m.warm_cool_bias.unwrap_or(0.0),
        sample_space: m.sample_space.clone().unwrap_or_else(|| "uv".into()),
        detail_field: m.detail.as_ref().map(|d| d.field.clone()).unwrap_or_default(),
        detail_amp: m.detail.as_ref().map(|d| d.amp).unwrap_or(0.0),
        detail_scale: m.detail.as_ref().map(|d| d.scale).unwrap_or(1.0),
      }),
      None => (String::new(), MaterialParams::default()),
    })
    .collect();

  // The name-resolution closures every cross-reference below validates through.
  let condition_exists = |name: &str| all.condition.iter().any(|c| c.name == name);
  let need_exists = |name: &str| all.need.iter().any(|n| n.name == name);
  let stat_exists = |name: &str| all.stat.iter().any(|s| s.name == name);
  let affordance_exists = |name: &str| all.affordance.iter().any(|a| a.name == name);
  let interaction_exists = |name: &str| all.interaction.iter().any(|i| i.name == name);
  // A binding's `yields` must name a thing kind (logs-drop F1).
  let thing_exists = |name: &str| all.thing.iter().any(|t| t.name == name);
  // An emotion's u4 identity IS its declaration index (emotions F1).
  let emotion_index = |name: &str| all.emotion.iter().position(|e| e.name == name);

  // Emotions (emotions F1): SIXTEEN max (the u4 bound), `fine` required at index 0 —
  // it is the empty-argmax default and the +0 display colour. Not registry-numbered:
  // the declaration index is the wire id, so corpus order is LAW here.
  if all.emotion.len() > 16 {
    errors.push(LoadError {
      file: String::new(),
      message: format!(
        "{} emotions defined — the u4 identity holds SIXTEEN max (emotions F1)",
        all.emotion.len()
      ),
    });
  }
  if let Some(first) = all.emotion.first() {
    if first.name != "fine" {
      errors.push(LoadError {
        file: String::new(),
        message: format!(
          "the first [[emotion]] must be `fine` — index 0 is the empty-argmax default \
           (emotions F1); found `{}`",
          first.name
        ),
      });
    }
  }
  b.emotions = all
    .emotion
    .iter()
    .map(|e| {
      let c = color(&Some(e.color.clone()), &format!("emotion `{}`", e.name), &mut errors)
        .unwrap_or(0);
      (e.name.clone(), EmotionParams {
        label: e.label.clone().unwrap_or_else(|| e.name.clone()),
        color: c,
      })
    })
    .collect();

  // The empty-intersection rule (stat-model F6): `"min"` (default) or `"max"`.
  fn parse_winner(
    winner: &Option<String>,
    who: &str,
    name: &str,
    errors: &mut Vec<LoadError>,
  ) -> bool {
    match winner.as_deref() {
      None | Some("min") => true,
      Some("max") => false,
      Some(w) => {
        errors.push(LoadError {
          file: String::new(),
          message: format!("{who} `{name}`: winner `{w}` — must be `min` or `max` (stat-model F6)"),
        });
        true
      }
    }
  }

  b.stats = all
    .stat
    .iter()
    .map(|s| {
      if s.min >= s.max {
        errors.push(LoadError {
          file: String::new(),
          message: format!("stat `{}`: min ({}) must be below max ({})", s.name, s.min, s.max),
        });
      }
      let min_wins = parse_winner(&s.winner, "stat", &s.name, &mut errors);
      (s.name.clone(), StatParams {
        label: s.label.clone().unwrap_or_else(|| s.name.clone()),
        min: s.min,
        max: s.max,
        min_wins,
      })
    })
    .collect();

  b.needs = all
    .need
    .iter()
    .map(|n| {
      if n.min >= n.max {
        errors.push(LoadError {
          file: String::new(),
          message: format!("need `{}`: min ({}) must be below max ({})", n.name, n.min, n.max),
        });
      }
      let min_wins = parse_winner(&n.winner, "need", &n.name, &mut errors);
      (n.name.clone(), NeedParams {
        label: n.label.clone().unwrap_or_else(|| n.name.clone()),
        min: n.min,
        max: n.max,
        min_wins,
        deplete: n.deplete,
        bands: n
          .band
          .iter()
          .map(|band| NeedBand { condition: band.condition.clone(), lo: band.lo, hi: band.hi })
          .collect(),
      })
    })
    .collect();

  b.conditions = all
    .condition
    .iter()
    .map(|m| {
      for s in &m.stats {
        if !stat_exists(&s.stat) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("condition `{}`: modifier names unknown stat `{}`", m.name, s.stat),
          });
        }
      }
      for n in &m.needs {
        if !need_exists(&n.need) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("condition `{}`: modifier names unknown need `{}`", m.name, n.need),
          });
        }
      }
      // stat-model F13, refined by survival F3: a DERIVED (band) condition may not
      // author rate/min/max need modifiers — its own liveness is computed FROM need
      // evaluation, so the modifier would feed the thing that decides it (a fixpoint
      // the lazy eval cannot host). A DEPLETE modifier is the acyclic exception: it
      // drains a DIFFERENT need (starving drains corpus), and the eval derives the
      // in-band window from the SOURCE's trajectory at depth 1. The cycle guard for
      // deplete targets runs after the need table below.
      if m.duration <= 0.0
        && m.needs.iter().any(|n| n.rate != 1.0 || n.min.is_some() || n.max.is_some())
      {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "condition `{}` is DERIVED (duration 0) and authors rate/min/max need \
             modifiers — only TIMED conditions may (stat-model F13; a `deplete` \
             modifier on ANOTHER need is the survival-F3 exception)",
            m.name
          ),
        });
      }
      (m.name.clone(), ConditionParams {
        label: m.label.clone().unwrap_or_else(|| m.name.clone()),
        duration: m.duration,
        priority: m.priority,
        stats: m
          .stats
          .iter()
          .map(|s| StatModifier { stat: s.stat.clone(), add: s.add, min: s.min, max: s.max })
          .collect(),
        needs: m
          .needs
          .iter()
          .map(|n| NeedModifier {
            need: n.need.clone(), rate: n.rate, min: n.min, max: n.max, deplete: n.deplete,
          })
          .collect(),
        emotions: m
          .emotions
          .iter()
          .filter_map(|em| {
            let Some(idx) = emotion_index(&em.emotion) else {
              errors.push(LoadError {
                file: String::new(),
                message: format!(
                  "condition `{}`: modifier names unknown emotion `{}`",
                  m.name, em.emotion
                ),
              });
              return None;
            };
            match em.magnitude.as_integer() {
              Some(v) if (1..=15).contains(&v) => {
                Some(EmotionModifier { emotion: idx as u8, magnitude: v as u8 })
              }
              _ => {
                errors.push(LoadError {
                  file: String::new(),
                  message: format!(
                    "condition `{}`: emotion `{}` magnitude must be an integer 1..=15 \
                     (the u4 bound; +0 = author nothing)",
                    m.name, em.emotion
                  ),
                });
                None
              }
            }
          })
          .collect(),
      })
    })
    .collect();

  // Traits: the authored per-field ARRAYS become per-LEVEL modifier tables (stat-model F5).
  // Every authored array within one trait must agree on length — that length IS the trait's
  // level count; a trait with no modifier arrays has ONE (empty) level.
  // player-pawns F4: `[[player_trait]]` shares the SCHEMA, so both lanes convert through the
  // ONE pass (chained, split by count below) — the classification is the only difference.
  // The six categories through the ONE conversion (trait-rows-u32 F2), tagged with their
  // subtype ids in palette order 8..13.
  let category_lanes: [(&[TraitToml], u16); 6] = [
    (&all.pawn_trait_constant, 8),
    (&all.pawn_trait_active, 9),
    (&all.pawn_trait_passive, 10),
    (&all.player_trait_constant, 11),
    (&all.player_trait_active, 12),
    (&all.player_trait_passive, 13),
  ];
  let converted_traits: Vec<(String, TraitParams, u16)> = category_lanes
    .iter()
    .flat_map(|(lane, cat)| lane.iter().map(move |t| (t, *cat)))
    .map(|(t, cat)| {
      let mut lens: Vec<usize> = Vec::new();
      for s in &t.stats {
        for l in [s.add.len(), s.min.len(), s.max.len()] {
          if l > 0 {
            lens.push(l);
          }
        }
      }
      for n in &t.needs {
        for l in [n.rate.len(), n.min.len(), n.max.len()] {
          if l > 0 {
            lens.push(l);
          }
        }
      }
      for em in &t.emotions {
        if emotion_index(&em.emotion).is_none() {
          errors.push(LoadError {
            file: String::new(),
            message: format!("trait `{}`: modifier names unknown emotion `{}`", t.name, em.emotion),
          });
        }
        match em.magnitude.as_array() {
          Some(a) if !a.is_empty() => {
            lens.push(a.len());
            if a.iter().any(|v| !matches!(v.as_integer(), Some(1..=15))) {
              errors.push(LoadError {
                file: String::new(),
                message: format!(
                  "trait `{}`: emotion `{}` — every per-level magnitude must be an \
                   integer 1..=15 (the u4 bound; +0 = author nothing)",
                  t.name, em.emotion
                ),
              });
            }
          }
          _ => {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "trait `{}`: emotion `{}` magnitude must be a per-LEVEL array (emotions F2)",
                t.name, em.emotion
              ),
            });
          }
        }
      }
      // emit_light is per-LEVEL like every other authored array (trait-lights F4) —
      // it participates in the agreement law and can be the level-count source for a
      // pure light trait.
      if !t.emit_light.is_empty() {
        lens.push(t.emit_light.len());
      }
      let count = lens.iter().copied().max().unwrap_or(1);
      if lens.iter().any(|&l| l != count) {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "trait `{}`: level arrays disagree on length — every authored array is per-LEVEL \
             and must have the trait's level count ({count})",
            t.name
          ),
        });
      }
      for s in &t.stats {
        if !stat_exists(&s.stat) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("trait `{}`: modifier names unknown stat `{}`", t.name, s.stat),
          });
        }
      }
      for n in &t.needs {
        if !need_exists(&n.need) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("trait `{}`: modifier names unknown need `{}`", t.name, n.need),
          });
        }
      }
      let levels = (0..count)
        .map(|i| TraitLevel {
          stats: t
            .stats
            .iter()
            .map(|s| StatModifier {
              stat: s.stat.clone(),
              add: s.add.get(i).copied().unwrap_or(0.0),
              min: s.min.get(i).copied(),
              max: s.max.get(i).copied(),
            })
            .collect(),
          needs: t
            .needs
            .iter()
            .map(|n| NeedModifier {
              need: n.need.clone(),
              rate: n.rate.get(i).copied().unwrap_or(1.0),
              min: n.min.get(i).copied(),
              max: n.max.get(i).copied(),
              deplete: n.deplete.get(i).copied(),
            })
            .collect(),
          emotions: t
            .emotions
            .iter()
            .filter_map(|em| {
              let idx = emotion_index(&em.emotion)? as u8;
              let v = em.magnitude.as_array()?.get(i)?.as_integer()?;
              if !(1..=15).contains(&v) {
                return None; // range errors already recorded above
              }
              Some(EmotionModifier { emotion: idx, magnitude: v as u8 })
            })
            .collect(),
        })
        .collect();
      let emit_light = t
        .emit_light
        .iter()
        .map(|l| {
          let rgb = match &l.color {
            LightColorToml::Rgb([r, g, b]) => (*r, *g, *b),
            LightColorToml::Hex(s) => {
              let v =
                color(&Some(s.clone()), &format!("trait `{}` emit_light", t.name), &mut errors)
                  .unwrap_or(0xFFFFFF);
              (
                f64::from((v >> 16) & 0xFF) / 255.0,
                f64::from((v >> 8) & 0xFF) / 255.0,
                f64::from(v & 0xFF) / 255.0,
              )
            }
          };
          TraitLight {
            color: rgb,
            intensity: l.intensity,
            reach: l.reach,
            fall_off: l.fall_off,
            elevation: l.elevation,
            radius: l.radius,
            cast: l.cast,
            hot: l.hot,
            flicker: l.flicker,
          }
        })
        .collect();
      (t.name.clone(), TraitParams {
        label: t.label.clone().unwrap_or_else(|| t.name.clone()),
        levels,
        tags: t.tags.clone(),
        emit_light,
      }, cat)
    })
    .collect();
  b.trait_defs = converted_traits;

  // Band → condition references (needs already built; conditions above).
  for n in &all.need {
    for band in &n.band {
      if !condition_exists(&band.condition) {
        errors.push(LoadError {
          file: String::new(),
          message: format!("need `{}`: band names unknown condition `{}`", n.name, band.condition),
        });
      }
    }
  }

  // survival F3's ACYCLICITY GUARD: a band-derived deplete may not target its own
  // SOURCE need (self-feeding fixpoint), and a deplete TARGET may not itself carry
  // bands whose conditions author depletes — the eval derives source trajectories at
  // depth 1, so a deeper chain would silently mis-evaluate rather than hang. Refused
  // here so the corpus can never author what the eval cannot honestly compute.
  {
    let drains_of = |cname: &str| -> Vec<String> {
      all.condition
        .iter()
        .find(|c| c.name == cname)
        .map(|c| {
          c.needs.iter().filter(|n| n.deplete.is_some()).map(|n| n.need.clone()).collect()
        })
        .unwrap_or_default()
    };
    for n in &all.need {
      for band in &n.band {
        for target in drains_of(&band.condition) {
          if target == n.name {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "need `{}`: band condition `{}` authors a deplete on its OWN source \
                 need — the self-feeding cycle the lazy eval cannot host (survival F3)",
                n.name, band.condition
              ),
            });
          }
          let target_chains = all
            .need
            .iter()
            .filter(|t| t.name == target)
            .flat_map(|t| t.band.iter())
            .any(|tb| !drains_of(&tb.condition).is_empty());
          if target_chains {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "need `{}` is a deplete TARGET (via `{}`) and its own bands author \
                 depletes — a depth-2 drain chain the depth-1 eval cannot honestly \
                 compute (survival F3's acyclicity law)",
                target, band.condition
              ),
            });
          }
        }
      }
    }
  }

  b.interactions = all
    .interaction
    .iter()
    .map(|i| {
      let operand = |o: &OperandToml, field: &str, errors: &mut Vec<LoadError>| -> Operand {
        match o {
          OperandToml::Num(v) => Operand::Value(*v),
          OperandToml::Str(s) => match s.strip_prefix('@') {
            Some(input) => match i.inputs.iter().position(|n| n == input) {
              Some(idx) => Operand::Input(idx),
              None => {
                errors.push(LoadError {
                  file: String::new(),
                  message: format!(
                    "interaction `{}`: {field} references `@{input}`, which is not in \
                     inputs {:?}",
                    i.name, i.inputs
                  ),
                });
                Operand::Value(0.0)
              }
            },
            None => Operand::Name(s.clone()),
          },
        }
      };
      let satisfy = i.satisfy.as_ref().map(|s| SatisfyEffect {
        target: operand(&s.target, "satisfy.target", &mut errors),
        need: operand(&s.need, "satisfy.need", &mut errors),
        amount: operand(&s.amount, "satisfy.amount", &mut errors),
      });
      if let Some(SatisfyEffect { need: Operand::Name(n), .. }) = &satisfy {
        if !need_exists(n) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("interaction `{}`: satisfy names unknown need `{n}`", i.name),
          });
        }
      }
      let move_effect = i.move_.as_ref().map(|m| MoveEffect {
        target: operand(&m.target, "move.target", &mut errors),
        to: operand(&m.to, "move.to", &mut errors),
      });
      // An interaction must DO something (input-rework F6): at least one effect.
      if satisfy.is_none()
        && move_effect.is_none()
        && i.destroy.is_none()
        && i.spawn.is_none()
        && i.remove.is_none()
        && i.store.is_none()
      {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "interaction `{}` authors no effect — at least one of `satisfy`/`move`/`destroy`/\
             `spawn`/`remove`/`store` is required",
            i.name
          ),
        });
      }
      // The destroy effect names the CARRIER and nothing else (lumberjack F5).
      if let Some(d) = &i.destroy {
        if d != "carrier" {
          errors.push(LoadError {
            file: String::new(),
            message: format!(
              "interaction `{}`: destroy = `{d}` — `\"carrier\"` is the only target \
               (lumberjack F5; `yields` is the reserved successor)",
              i.name
            ),
          });
        }
      }
      if i.duration < 0.0 {
        errors.push(LoadError {
          file: String::new(),
          message: format!("interaction `{}`: duration {} is negative", i.name, i.duration),
        });
      }
      for g in &i.grant {
        if !condition_exists(g) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("interaction `{}`: grants unknown condition `{g}`", i.name),
          });
        }
      }
      for a in &i.affordances {
        if !affordance_exists(a) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("interaction `{}`: unknown affordance `{a}`", i.name),
          });
        }
      }
      if !matches!(i.location.as_str(), "on" | "adjacent" | "target" | "self" | "slot") {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "interaction `{}`: location `{}` — the built rules are `on`, `adjacent`, \
             `target`, `self`, and `slot` (input-rework F4 / lumberjack F2 / \
             food-chain I9 / inventory F5)",
            i.name, i.location
          ),
        });
      }
      // The queue-strip display block (intent-queue-ui F2): defaults for the
      // unauthored case; the ring direction is a closed set.
      let q = i.queue.as_ref();
      let progress =
        q.and_then(|q| q.progress.clone()).unwrap_or_else(|| "none".into());
      if !matches!(progress.as_str(), "cw" | "ccw" | "none") {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "interaction `{}`: queue.progress `{progress}` — the ring directions are \
             `cw`, `ccw` and `none`",
            i.name
          ),
        });
      }
      let size = q.and_then(|q| q.size).unwrap_or(1.0);
      if size <= 0.0 {
        errors.push(LoadError {
          file: String::new(),
          message: format!("interaction `{}`: queue.size {size} is not positive", i.name),
        });
      }
      let queue = crate::loader::QueueVisual {
        hover: q.and_then(|q| q.hover.clone()),
        size,
        background: color(
          &q.and_then(|q| q.background.clone()),
          &format!("interaction `{}` queue.background", i.name),
          &mut errors,
        ),
        progress,
        progress_color: color(
          &q.and_then(|q| q.progress_color.clone()),
          &format!("interaction `{}` queue.progress_color", i.name),
          &mut errors,
        ),
        progress_fill: q.and_then(|q| q.progress_fill).unwrap_or(true),
        cancelable: q.and_then(|q| q.cancelable).unwrap_or(false),
      };
      // The spawn effect: a named thing (food-chain F5/F6 — the thing must exist,
      // `at` is closed) or `"carried"` (inventory F5 — spawn the SLOT's item, which
      // only makes sense where a slot IS the carrier, so the location must be `slot`).
      let spawn = i.spawn.as_ref().map(|s| match s {
        SpawnToml::Thing(s) => {
          if !thing_exists(&s.thing) {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "interaction `{}`: spawn names unknown thing `{}`",
                i.name, s.thing
              ),
            });
          }
          if s.at != "on" && s.at != "adjacent" {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "interaction `{}`: spawn.at `{}` — `on` or `adjacent` (food-chain F6)",
                i.name, s.at
              ),
            });
          }
          SpawnEffect::Thing { thing: s.thing.clone(), at: s.at.clone() }
        }
        SpawnToml::Carried(word) => {
          if word != "carried" {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "interaction `{}`: spawn `{word}` — a `{{ thing, at }}` table or the \
                 string `\"carried\"` (inventory F5)",
                i.name
              ),
            });
          }
          if i.location != "slot" {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "interaction `{}`: spawn = \"carried\" requires location = \"slot\" — \
                 the carried item IS the slot carrier (inventory F5)",
                i.name
              ),
            });
          }
          SpawnEffect::Carried
        }
      });
      if let Some(r) = &i.remove {
        if r != "target" {
          errors.push(LoadError {
            file: String::new(),
            message: format!(
              "interaction `{}`: remove `{r}` — `target` is the only value (food-chain F5)",
              i.name
            ),
          });
        }
      }
      // The store effect names the CARRIER and nothing else (inventory F4).
      if let Some(s) = &i.store {
        if s != "carrier" {
          errors.push(LoadError {
            file: String::new(),
            message: format!(
              "interaction `{}`: store = `{s}` — `\"carrier\"` is the only target \
               (inventory F4)",
              i.name
            ),
          });
        }
      }
      let label = i.label.clone().unwrap_or_else(|| i.name.clone());
      (i.name.clone(), InteractionParams {
        menu_text: i.menu_text.clone().unwrap_or_else(|| label.clone()),
        label,
        affordances: i.affordances.clone(),
        inputs: i.inputs.clone(),
        satisfy,
        move_effect,
        grants: i.grant.clone(),
        destroy: i.destroy.clone(),
        spawn,
        remove: i.remove.clone(),
        store: i.store.clone(),
        location: i.location.clone(),
        duration: i.duration,
        queue,
      })
    })
    .collect();

  b.affordances = all
    .affordance
    .iter()
    .map(|a| {
      let check = match (&a.check.stat, &a.check.need, &a.check.tag) {
        (Some(stat), None, None) => {
          if !stat_exists(stat) {
            errors.push(LoadError {
              file: String::new(),
              message: format!("affordance `{}`: check names unknown stat `{stat}`", a.name),
            });
          }
          if matches!((a.check.above, a.check.below), (None, None) | (Some(_), Some(_))) {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "affordance `{}`: check authors exactly ONE of `above`/`below` (stat-model F10)",
                a.name
              ),
            });
          }
          AffordanceCheck::Stat {
            stat: stat.clone(),
            above: a.check.above,
            below: a.check.below,
          }
        }
        (None, Some(need), None) => {
          if !need_exists(need) {
            errors.push(LoadError {
              file: String::new(),
              message: format!("affordance `{}`: check names unknown need `{need}`", a.name),
            });
          }
          let ops = [
            (a.check.gte, Cmp::Gte),
            (a.check.gt, Cmp::Gt),
            (a.check.lt, Cmp::Lt),
            (a.check.lte, Cmp::Lte),
          ];
          let mut authored = ops.iter().filter_map(|(v, c)| v.map(|v| (*c, v)));
          let first = authored.next();
          let (cmp, value) = match (first, authored.next()) {
            (Some(cv), None) => cv,
            _ => {
              errors.push(LoadError {
                file: String::new(),
                message: format!(
                  "affordance `{}`: a NEED check authors exactly ONE of gte/gt/lt/lte \
                   (food-chain F4)",
                  a.name
                ),
              });
              (Cmp::Lte, 0.0)
            }
          };
          AffordanceCheck::Need { need: need.clone(), cmp, value }
        }
        (None, None, Some(tag)) => {
          // The TAG form (attack F1): the tag must be authored by at least one trait —
          // a tag nothing carries is a typo, refused at load like every other name.
          if !all.trait_.iter().any(|t| t.tags.iter().any(|g| g == tag)) {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "affordance `{}`: check names tag `{tag}` that no trait authors (attack F1)",
                a.name
              ),
            });
          }
          AffordanceCheck::Tag { tag: tag.clone() }
        }
        _ => {
          errors.push(LoadError {
            file: String::new(),
            message: format!(
              "affordance `{}`: check names exactly ONE of `stat`/`need`/`tag` \
               (food-chain F4 / attack F1)",
              a.name
            ),
          });
          AffordanceCheck::Stat { stat: String::new(), above: Some(0.0), below: None }
        }
      };
      (a.name.clone(), AffordanceParams {
        label: a.label.clone().unwrap_or_else(|| a.name.clone()),
        check,
      })
    })
    .collect();

  let material_id = |name: &str| -> u16 {
    b.materials.iter().position(|(n, _)| n == name).map(|i| i as u16 + 1).unwrap_or(0)
  };
  // A trait BINDING must name a TIER the def's table has (F6: 0-based variants) — one
  // map across all six categories: name → (category subtype id, tier count).
  let trait_lookup: HashMap<String, (u16, usize)> =
    b.trait_defs.iter().map(|(n, p, c)| (n.clone(), (*c, p.levels.len()))).collect();

  for slot in &tiles {
    b.tiles.push(match slot {
      Some(t) => tile_def(t, &material_id, &interaction_exists, &thing_exists, &mut errors),
      None => TileDef::default(), // a retired id holds its place
    });
  }
  for slot in &things {
    b.things.push(match slot {
      Some(t) => thing_def(
        t,
        &material_id,
        &need_exists,
        &trait_lookup,
        &interaction_exists,
        &thing_exists,
        &mut errors,
      ),
      None => ThingDef::default(),
    });
  }

  // Brains (npc-host F5): needs + CONSTANT player-trait binds only. No retirement holes —
  // the vec is fresh; corpus order is the kind seed like every def.
  for br in &all.brain {
    let tax = taxonomy(&br.type_name, &br.kind, &br.sub_type, &br.variant, &format!("brain `{}`", br.name), &mut errors);
    if let Some(t) = &tax {
      if t.type_name != "brain" {
        errors.push(LoadError {
          file: String::new(),
          message: format!("brain `{}`: type must be `brain`, got `{}`", br.name, t.type_name),
        });
      }
    }
    let mut needs = Vec::new();
    for n in &br.needs {
      if need_exists(n) {
        needs.push(n.clone());
      } else {
        errors.push(LoadError {
          file: String::new(),
          message: format!("brain `{}`: unknown need `{n}`", br.name),
        });
      }
    }
    let mut player_traits = Vec::new();
    let mut active_binds = 0usize;
    for tb in &br.traits {
      let (name, variant) = tb.bind();
      match trait_lookup.get(&name) {
        Some(&(cat, tiers)) if cat >= 11 => {
          if variant as usize >= tiers {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "brain `{}`: `{name}` variant {variant} is out of range ({tiers} tier(s),                  0-based)",
                br.name
              ),
            });
          } else if cat == 12 && {
            active_binds += 1;
            active_binds > 3
          } {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "brain `{}`: at most 3 ACTIVE traits (the slot law, trait-rows-u32 F3)",
                br.name
              ),
            });
          } else {
            player_traits.push(TraitBind { name, variant });
          }
        }
        _ => errors.push(LoadError {
          file: String::new(),
          message: format!(
            "brain `{}`: `{name}` is not a PLAYER trait category — brains bind player_* only",
            br.name
          ),
        }),
      }
    }
    b.brains.push(crate::loader::BrainDef {
      version: br.version,
      taxonomy: tax,
      name: br.name.clone(),
      needs,
      player_traits,
    });
  }

  for biome in &all.biome {
    match biome_def(biome) {
      Ok(d) => b.biomes.push(d),
      Err(e) => errors.push(e),
    }
  }
  b.subtypes = subtypes;

  // The ID LAW'S SUCCESSOR ([F17]): two defs may share a taxonomy — that is how wolf v0 and wolf v1
  // coexist — but not a `(tuple, version)` pair. A duplicate would give one identity two
  // definitions, which is the same aliasing the old duplicate-id check refused, wearing the shape
  // versioning gives it.
  {
    let mut seen: HashMap<(String, String, String, String, u32), String> = HashMap::new();
    let mut check = |tax: Option<&Taxonomy>, version: u32, name: &str, errors: &mut Vec<LoadError>| {
      let Some(t) = tax else { return };
      let Some((sub, variant)) = t.tuples().first().map(|(s, v)| (s.to_string(), v.to_string()))
      else {
        return;
      };
      let key = (t.type_name.clone(), sub, t.kind.clone(), variant, version);
      if let Some(prev) = seen.get(&key) {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "`{name}`: {}/{}/{} v{version} is already defined by `{prev}` — two defs may share a \
             taxonomy (that is how versions coexist) but not a (taxonomy, version) pair",
            key.0, key.1, key.2
          ),
        });
      } else {
        seen.insert(key, name.to_string());
      }
    };
    for d in &b.tiles {
      check(d.taxonomy.as_ref(), d.version, &d.name, &mut errors);
    }
    for d in &b.things {
      check(d.taxonomy.as_ref(), d.version, &d.name, &mut errors);
    }
  }

  if !errors.is_empty() {
    return Err(errors);
  }
  Ok(b.index())
}

/// The ID LAW (F1): place each def at `id − 1`, refusing duplicates/zeros. Holes stay
/// `None` (retired ids).
fn place<'a, T>(
  defs: &'a [T],
  what: &str,
  key: impl Fn(&T) -> (u16, String),
  errors: &mut Vec<LoadError>,
) -> Vec<Option<&'a T>> {
  let max = defs.iter().map(|d| key(d).0).max().unwrap_or(0);
  let mut out: Vec<Option<&T>> = vec![None; max as usize];
  for d in defs {
    let (id, name) = key(d);
    if id == 0 {
      errors.push(LoadError { file: String::new(), message: format!("{what} `{name}`: id 0 is reserved (F1)") });
      continue;
    }
    let slot = &mut out[id as usize - 1];
    if let Some(prev) = slot {
      let (_, prev_name) = key(prev);
      errors.push(LoadError {
        file: String::new(),
        message: format!("{what} `{name}`: id {id} already taken by `{prev_name}` (F1 — ids never reuse)"),
      });
      continue;
    }
    *slot = Some(d);
  }
  out
}

fn color(s: &Option<String>, what: &str, errors: &mut Vec<LoadError>) -> Option<u32> {
  let s = s.as_ref()?;
  let hex = s.strip_prefix('#').unwrap_or(s);
  match u32::from_str_radix(hex, 16) {
    Ok(v) => Some(v),
    Err(_) => {
      errors.push(LoadError { file: String::new(), message: format!("{what}: bad colour `{s}`") });
      None
    }
  }
}

fn packed_channels(
  packed: &[PackedToml],
  material_id: &dyn Fn(&str) -> u16,
  what: &str,
  errors: &mut Vec<LoadError>,
) -> [PackedChannel; 4] {
  let mut out: [PackedChannel; 4] = Default::default();
  for (i, ch) in packed.iter().take(4).enumerate() {
    let mid = ch.material.as_deref().map(material_id).unwrap_or(0);
    let tint = color(&ch.tint, what, errors).unwrap_or(0);
    if mid != 0 || tint != 0 {
      out[i] = PackedChannel { material_id: mid, tint };
    }
  }
  out
}

/// Expand a part's authored subframe map into the dense `[variant][rotation]` array,
/// walking the SAME per-component fallback chain the `.rd` loader ran:
/// `v{v}.r{r}` → `v{v}.<alias>` → `v{v}` → `r{r}` → `<alias>` → `default`.
fn dir_frames(
  sub: &HashMap<String, SubframeRect>,
  flat_anchor: (f64, f64),
) -> [[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF] {
  const ALIASES: [&str; 4] = ["s", "e", "n", "w"];
  if sub.is_empty() {
    return [[DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: flat_anchor }; ROTATIONS_PER_DEF];
      VARIANTS_PER_DEF];
  }
  std::array::from_fn(|v| {
    std::array::from_fn(|r| {
      let alias = ALIASES.get(r);
      let mut keys: Vec<String> = vec![format!("v{v}.r{r}")];
      if let Some(a) = alias {
        keys.push(format!("v{v}.{a}"));
      }
      keys.push(format!("v{v}"));
      keys.push(format!("r{r}"));
      if let Some(a) = alias {
        keys.push((*a).to_string());
      }
      keys.push("default".into());
      let pick = |get: fn(&SubframeRect) -> Option<f64>, dflt: f64| -> f64 {
        keys.iter().find_map(|k| sub.get(k).and_then(get)).unwrap_or(dflt)
      };
      DirFrame {
        sub: (
          pick(|s| s.x, 0.0),
          pick(|s| s.y, 0.0),
          pick(|s| s.w, 1.0),
          pick(|s| s.h, 1.0),
        ),
        // ax/ay fall back to the part's flat sprite_anchor — the `.rd` chain's tail.
        anchor: (pick(|s| s.ax, flat_anchor.0), pick(|s| s.ay, flat_anchor.1)),
      }
    })
  })
}

/// A parts array → [`VisualParts`] (part 0 mirrors onto the flat prim-0 fields).
/// `taxonomy` is the def's, used to DERIVE a part's texture stem when the part authors none —
/// definition-registry F1: the taxonomy IS the art path, so authoring the stem was authoring the
/// same fact twice. A part that authors `texture` still wins, which is how the `"white"` no-art
/// fill survives (it is a rendering fallback, not a taxon).
fn visual(
  parts_toml: &[PartToml],
  packed_toml: &[PackedToml],
  material_id: &dyn Fn(&str) -> u16,
  what: &str,
  taxonomy: Option<&Taxonomy>,
  errors: &mut Vec<LoadError>,
) -> Option<VisualParts> {
  let first = parts_toml.first()?;
  let mut parts = Vec::new();
  for p in parts_toml {
    let tint = color(&p.tint, what, errors).unwrap_or(0xffffff);
    let geo = color(&p.geo, what, errors).unwrap_or(tint);
    let sprite_anchor = p.sprite_anchor.as_ref().map(|a| (a.x, a.y)).unwrap_or((0.5, 0.5));
    parts.push(VisualPart {
      tint,
      geo_color: geo,
      texture: p.texture.clone().or_else(|| taxonomy.map(Taxonomy::stem)),
      part: p.part.unwrap_or(0),
      scale: p.scale.unwrap_or(1.0),
      offset: (p.offset.as_ref().map(|o| o.x).unwrap_or(0.0), p.offset.as_ref().map(|o| o.y).unwrap_or(0.0)),
      elevation: p.offset.as_ref().map(|o| o.z).unwrap_or(0.0),
      depth: p.depth.unwrap_or(0.0),
      size: p.size.unwrap_or(1.0),
      span: p.span.unwrap_or(1.0),
      sprite_scale: p.sprite_scale.as_ref().map(|s| (s.w, s.h)).unwrap_or((1.0, 1.0)),
      sprite_anchor,
      anchor: p.anchor.as_ref().map(|a| (a.x, a.y)).unwrap_or((0.5, 0.5)),
      dir_frames: dir_frames(&p.subframe, sprite_anchor),
    });
  }
  let p0 = &parts[0];
  Some(VisualParts {
    tint: p0.tint,
    geo_color: p0.geo_color,
    texture: p0.texture.clone(),
    footprint: first.footprint.as_ref().map(|f| (f.w, f.h)).unwrap_or((1.0, 1.0)),
    anchor: p0.anchor,
    size: p0.size,
    span: p0.span,
    // prim-0's sprite_scale in `.rd` was the {w,h} pair; the part's covers it.
    sprite_scale: p0.sprite_scale,
    sprite_anchor: p0.sprite_anchor,
    dir_frames: p0.dir_frames,
    packed: packed_channels(packed_toml, material_id, what, errors),
    parts,
  })
}

fn tile_def(
  t: &TileToml,
  material_id: &dyn Fn(&str) -> u16,
  interaction_exists: &dyn Fn(&str) -> bool,
  thing_exists: &dyn Fn(&str) -> bool,
  errors: &mut Vec<LoadError>,
) -> TileDef {
  // A tile's visual is the single-part degenerate case: synthesize the one part from
  // the flat fields so `visual()` stays the one constructor.
  let part = PartToml {
    texture: t.texture.clone(),
    tint: t.tint.clone(),
    geo: t.geo.clone(),
    footprint: None,
    anchor: None,
    sprite_anchor: None,
    size: None,
    span: None,
    scale: None,
    sprite_scale: None,
    part: None,
    depth: None,
    offset: None,
    subframe: HashMap::new(),
  };
  let tax = taxonomy(&t.type_name, &t.kind, &t.sub_type, &t.variant, &format!("tile `{}`", t.name), errors);
  // A taxonomy alone is enough to have a visual now: the stem derives from it, so a def no longer
  // has to author `texture` to have art.
  let has_visual = t.texture.is_some() || t.tint.is_some() || tax.is_some();
  let visual = has_visual
    .then(|| visual(&[part], &t.packed, material_id, &t.name, tax.as_ref(), errors))
    .flatten();
  TileDef {
    version: t.version,
    taxonomy: tax,
    name: t.name.clone(),
    color: visual.as_ref().map(|v| v.tint),
    visual,
    build: t.build.clone(),
    height: t.height,
    lanes: [
      t.linked.as_ref().map(|l| l.w).unwrap_or(0.0),
      t.linked.as_ref().map(|l| l.h).unwrap_or(0.0),
      t.padding.unwrap_or(0.0),
      t.rotation.unwrap_or(0.0),
      t.cast_shadow.unwrap_or(0.0),
      t.receives_shadows.unwrap_or(0.0),
    ],
    interactions: interaction_binds(&t.interactions, interaction_exists, thing_exists, "tile", &t.name, errors),
    pathable: t.pathable,
  }
}

/// Validate + flatten a carrier's interaction bindings (stat-model F5/F9; the yield
/// lane — logs-drop F1).
fn interaction_binds(
  binds: &[InteractionBindToml],
  interaction_exists: &dyn Fn(&str) -> bool,
  thing_exists: &dyn Fn(&str) -> bool,
  what: &str,
  name: &str,
  errors: &mut Vec<LoadError>,
) -> Vec<crate::loader::InteractionBind> {
  for b in binds {
    if !interaction_exists(&b.name) {
      errors.push(LoadError {
        file: String::new(),
        message: format!("{what} `{name}`: unknown interaction `{}`", b.name),
      });
    }
    // The yield must name a real thing kind (logs-drop F1) — a ghost yield would
    // compose a SET with a kind the world can't draw.
    if let Some(y) = &b.yields {
      if !thing_exists(y) {
        errors.push(LoadError {
          file: String::new(),
          message: format!("{what} `{name}`: binding `{}` yields unknown thing `{y}`", b.name),
        });
      }
    }
  }
  binds
    .iter()
    .map(|b| crate::loader::InteractionBind {
      name: b.name.clone(),
      magnitude: b.magnitude,
      yields: b.yields.clone(),
    })
    .collect()
}

/// Build a def's [`Taxonomy`] from its authored fields, or `None` if it authors none.
///
/// Validation is deliberately strict on the PARTIAL case: `type` and `kind` together with at least
/// one `subType` and one `variant` is a taxonomy; anything in between is a half-authored def, and a
/// half-authored taxonomy would mint the wrong registry rows silently. Absent entirely is fine —
/// the field is being introduced additively (I9).
fn taxonomy(
  type_name: &Option<String>,
  kind: &Option<String>,
  sub_type: &[String],
  variant: &[String],
  who: &str,
  errors: &mut Vec<LoadError>,
) -> Option<Taxonomy> {
  let any = type_name.is_some() || kind.is_some() || !sub_type.is_empty() || !variant.is_empty();
  if !any {
    return None;
  }
  let (Some(type_name), Some(kind)) = (type_name.clone(), kind.clone()) else {
    errors.push(LoadError {
      file: String::new(),
      message: format!("{who}: a taxonomy needs BOTH `type` and `kind`"),
    });
    return None;
  };
  if sub_type.is_empty() || variant.is_empty() {
    errors.push(LoadError {
      file: String::new(),
      message: format!(
        "{who}: `subType` and `variant` are applicability ARRAYS and must each name at least one \
         entry (a def in exactly one form still writes single-element arrays)"
      ),
    });
    return None;
  }
  Some(Taxonomy {
    type_name,
    kind,
    sub_type: sub_type.to_vec(),
    variant: variant.to_vec(),
  })
}

fn thing_def(
  t: &ThingToml,
  material_id: &dyn Fn(&str) -> u16,
  need_exists: &dyn Fn(&str) -> bool,
  trait_lookup: &HashMap<String, (u16, usize)>,
  interaction_exists: &dyn Fn(&str) -> bool,
  thing_exists: &dyn Fn(&str) -> bool,
  errors: &mut Vec<LoadError>,
) -> ThingDef {
  let tax = taxonomy(&t.type_name, &t.kind, &t.sub_type, &t.variant, &format!("thing `{}`", t.name), errors);
  let visual = visual(&t.part, &t.packed, material_id, &t.name, tax.as_ref(), errors);
  let mut needs = Vec::new();
  for n in t.needs.iter().take(NEEDS_PER_KIND) {
    if need_exists(n) {
      needs.push(n.clone());
    } else {
      errors.push(LoadError {
        file: String::new(),
        message: format!("thing `{}`: unknown need `{n}`", t.name),
      });
    }
  }
  let mut traits = Vec::new();
  let mut player_traits = Vec::new();
  let mut active_binds = 0usize;
  // trait-rows-u32 F2/F6: binds resolve in the ONE namespace; constancy and activation are
  // CATEGORY membership. Non-pawn things take CONSTANT categories only (a runtime trait on
  // a cold thing could not survive a cold save — trait-lights F6's law, category form).
  // The LOAD DOOR of the active-slot law (F3/I5): at most 3 `*_active` binds per def.
  let is_pawn = tax.as_ref().is_some_and(|x| x.type_name == "pawn");
  for tb in &t.traits {
    let (name, variant) = tb.bind();
    match trait_lookup.get(&name) {
      None => errors.push(LoadError {
        file: String::new(),
        message: format!("thing `{}`: unknown trait `{name}`", t.name),
      }),
      Some(&(_, tiers)) if variant as usize >= tiers => errors.push(LoadError {
        file: String::new(),
        message: format!(
          "thing `{}`: trait `{name}` variant {variant} is out of range ({tiers} tier(s),            0-based — trait-rows-u32 F6)",
          t.name
        ),
      }),
      Some(&(cat, _)) if !is_pawn && cat != 8 && cat != 11 => errors.push(LoadError {
        file: String::new(),
        message: format!(
          "thing `{}`: trait `{name}` is not a CONSTANT category — a non-pawn thing has no            runtime trait storage (trait-lights F6 / trait-rows-u32 F2)",
          t.name
        ),
      }),
      Some(&(cat, _)) => {
        if cat == 9 || cat == 12 {
          active_binds += 1;
          if active_binds > 3 {
            errors.push(LoadError {
              file: String::new(),
              message: format!(
                "thing `{}`: a carrier binds at most 3 ACTIVE traits (the slot law,                  trait-rows-u32 F3)",
                t.name
              ),
            });
            continue;
          }
        }
        let bind = TraitBind { name, variant };
        if cat >= 11 {
          player_traits.push(bind);
        } else {
          traits.push(bind);
        }
      }
    }
  }
  ThingDef {
    version: t.version,
    taxonomy: tax,
    name: t.name.clone(),
    color: visual.as_ref().map(|v| v.tint),
    visual,
    needs,
    traits,
    player_traits,
    interactions: interaction_binds(&t.interactions, interaction_exists, thing_exists, "thing", &t.name, errors),
    pathable: t.pathable,
    geo_label: t.geo_label.clone(),
  }
}

fn biome_def(biome: &BiomeToml) -> Result<BiomeDef, LoadError> {
  // Dimension names map by the FIXED sampling order (VARIABLES.md).
  let dim = |name: &str| -> Result<usize, LoadError> {
    match name {
      "temperature" => Ok(0),
      "humidity" => Ok(1),
      "elevation" => Ok(2),
      _ => Err(LoadError {
        file: String::new(),
        message: format!("biome `{}`: unknown dimension `{name}`", biome.name),
      }),
    }
  };
  let mut when = Vec::new();
  // Deterministic rule order: sort by dimension index (a conjunction is order-free,
  // but the bundle must not depend on HashMap iteration order).
  let mut conds: Vec<(&String, &CondToml)> = biome.when.iter().collect();
  conds.sort_by_key(|(name, _)| name.as_str().to_string());
  for (name, c) in conds {
    let d = dim(name)?;
    for (op, t) in [(Cmp::Gte, c.gte), (Cmp::Gt, c.gt), (Cmp::Lt, c.lt), (Cmp::Lte, c.lte)] {
      if let Some(t) = t {
        when.push((d, op, t));
      }
    }
  }
  Ok(BiomeDef {
    name: biome.name.clone(),
    subtype: biome.subtype,
    body: BiomeBody::Rules(BiomeRules {
      when,
      tile: biome.tile.clone(),
      scatter: biome.scatter.iter().map(|s| (s.salt, s.p, s.thing.clone())).collect(),
    }),
  })
}

#[cfg(test)]
mod tests {
  use crate::loader::{
    load, pack_emotion_modifier, unpack_emotion_modifier, AffordanceCheck, TraitBind,
  };

  fn src(name: &str, text: &str) -> (String, String) {
    (name.to_string(), text.to_string())
  }

  #[test]
  fn a_minimal_corpus_round_trips_ids_and_tables() {
    let text = r##"
[[tile]]
name = "grass"
texture = "white"
tint = "#4b573e"

[[tile]]
name = "stone"
texture = "linked/wall_smooth"
tint = "#ffffff"
geo = "#6b6b6b"
height = 1.0
linked = { w = 4, h = 4 }
padding = 0.5
packed = [ { material = "mottle", tint = "#6b6b6b" } ]

[[material]]
id = 1
name = "mottle"
noise_field = "mottle"
hue_swing = 10.0
sample_space = "world"

[[thing]]
name = "wolf"
needs = ["thirst"]
[[thing.part]]
texture = "pawn/animal/wolf"
tint = "#ffffff"
scale = 0.8
sprite_anchor = { x = 0.5, y = 1.0 }
[thing.part.subframe]
default = { x = 0.25, w = 0.5 }
e = { x = 0.05 }
v2 = { x = 0.6 }

[[need]]
name = "thirst"
deplete = 21600
band = [ { condition = "thirsty", lo = 0.10, hi = 0.35 } ]

[[emotion]]
name = "fine"
color = "#9aa4b0"

[[emotion]]
name = "uncomfortable"
label = "Uncomfortable"
color = "#8a8f3c"

[[condition]]
name = "thirsty"
label = "Thirsty"
emotions = [ { emotion = "uncomfortable", magnitude = 2 } ]
priority = 20

[[condition]]
name = "dehydrated"
emotions = [ { emotion = "uncomfortable", magnitude = 5 } ]

[[biome]]
name = "forest"
subtype = 6
when = { humidity = { gte = 0.55 }, elevation = { lt = 0.82 } }
tile = "grass"
scatter = [ { salt = 6, p = 0.99, thing = "wolf" } ]
"##;
    let b = load(&[src("content.toml", text)]).expect("clean load");

    // ids: POSITIONAL now (definition-registry F1/F15) — the corpus describes, the server numbers,
    // and a position is only the SEED a fresh registry allocates from. An existing registry
    // overrides through `with_registry`, which is what makes a reorder harmless.
    //
    // HOLES moved with the numbering. A retired tile used to be a skipped `id`; now it simply
    // stops being authored, and the registry keeps its row so the id is never handed to anything
    // else (reclaim is deliberately unbuilt — F7).
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("stone"), Some(2));
    assert_eq!(b.tile_names().len(), 2);
    assert_eq!(b.tile_texture_stems(), vec!["white".to_string(), "linked/wall_smooth".to_string()]);

    // tile lanes + packed + height
    assert_eq!(b.tile_lighting_lanes()[6..12], [4.0, 4.0, 0.5, 0.0, 0.0, 0.0]);
    assert_eq!(b.tile_height(2), Some(1.0));
    assert_eq!(b.tile_packed_channels()[1][0].material_id, 1);
    assert_eq!(b.tile_packed_channels()[1][0].tint, 0x6b6b6b);

    // the wolf: needs, subframe chain (default + e + v2 compose per component)
    assert_eq!(b.thing_object_id("wolf"), Some(1));
    // needs are u32 gameplay refs now (interactions F1) — the seed packs
    // gameplay(8) | need(1) | position(1) | default(0).
    let thirst = b.gameplay_reference("need", "thirst").expect("thirst ref");
    assert_eq!(thirst, 0x8001_0010);
    assert_eq!(b.thing_needs(1), vec![thirst]);
    let v = b.visual_for_object(1).unwrap();
    let f = &v.dir_frames;
    assert_eq!(f[0][0].sub.0, 0.25, "default");
    assert_eq!(f[0][1].sub.0, 0.05, "east overrides x");
    assert_eq!(f[0][1].sub.2, 0.5, "…but inherits default's w");
    assert_eq!(f[2][0].sub.0, 0.6, "v2 covers every rotation");
    assert_eq!(f[2][1].sub.0, 0.6, "per-variant beats per-rotation");
    assert_eq!(f[0][0].anchor.1, 1.0, "ax/ay fall back to the flat sprite_anchor");

    // biome rules classify + scatter deterministically
    assert_eq!(b.biome_subtype_id("forest"), Some(6));
    let g = b.generate(&[0.5, 0.7, 0.6], 42);
    assert_eq!(g.biome.as_deref(), Some("forest"));
    assert_eq!(g.tile.as_deref(), Some("grass"));
    assert_eq!(g.thing1.as_deref(), Some("wolf"));
    assert_eq!(b.generate(&[0.5, 0.7, 0.9], 42).biome, None, "elevation lt fails");

    // needs registry
    assert_eq!(b.need_id("thirst"), Some(1));
    assert_eq!(b.need_params("thirst").unwrap().bands.len(), 1);
    // emotions (emotions F1/F2): declaration index = identity, modifiers resolve to it.
    assert_eq!(b.emotion_index("fine"), Some(0));
    assert_eq!(b.emotion_index("uncomfortable"), Some(1));
    let em = &b.condition_params("thirsty").unwrap().emotions;
    assert_eq!(em.len(), 1);
    assert_eq!((em[0].emotion, em[0].magnitude), (1, 2));
    assert_eq!(pack_emotion_modifier(em[0]), 0x12, "emotion:4 | magnitude:4");
    let up = unpack_emotion_modifier(0x12);
    assert_eq!((up.emotion, up.magnitude), (1, 2), "the round-trip");
    // priority: authored is read back verbatim; an omitted key is 0, never a derived guess.
    assert_eq!(b.condition_params("thirsty").unwrap().priority, 20);
    assert_eq!(b.condition_params("dehydrated").unwrap().priority, 0, "absent = 0");
  }

  #[test]
  fn the_id_law_survives_where_nothing_else_numbers() {
    // NARROWED again (interactions F1): MATERIALS alone keep the explicit id law — a render
    // registry nothing else numbers. Needs/conditions joined the gameplay taxonomy, where the
    // NAME is the kind, so the aliasing rule becomes name-uniqueness per category.
    let dup = r##"
[[need]]
name = "thirst"
[[need]]
name = "thirst"
"##;
    let e = load(&[src("t.toml", dup)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("defined twice")), "{e:?}");

    let zero = "[[material]]\nid = 0\nname = \"mottle\"\n";
    let e = load(&[src("t.toml", zero)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("id 0")), "{e:?}");

    let missing = "[[material]]\nname = \"mottle\"\n";
    let e = load(&[src("t.toml", missing)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("missing field")), "{e:?}");

    // A need still authoring an id is a LOUD refusal now, not a silently-ignored key.
    let e = load(&[src("t.toml", "[[need]]\nid = 1\nname = \"thirst\"\n")]).unwrap_err();
    assert!(e[0].message.contains("unknown field `id`"), "{}", e[0].message);
  }

  #[test]
  fn the_emotion_laws_refuse_loudly() {
    // emotions F1: `fine` must be declared FIRST — index 0 is the empty-argmax default.
    let not_fine = "[[emotion]]\nname = \"happy\"\ncolor = \"#e8b23a\"\n";
    let e = load(&[src("t.toml", not_fine)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("must be `fine`")), "{e:?}");

    // …SIXTEEN max (the u4 bound).
    let mut many = String::from("[[emotion]]\nname = \"fine\"\ncolor = \"#9aa4b0\"\n");
    for i in 0..16 {
      many.push_str(&format!("[[emotion]]\nname = \"e{i}\"\ncolor = \"#000000\"\n"));
    }
    let e = load(&[src("t.toml", &many)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("SIXTEEN max")), "{e:?}");

    // …color is REQUIRED (deny_unknown_fields serde refusal).
    let e = load(&[src("t.toml", "[[emotion]]\nname = \"fine\"\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("missing field `color`")), "{e:?}");

    // A condition modifier must name a declared emotion, with magnitude 1..=15.
    let fine = "[[emotion]]\nname = \"fine\"\ncolor = \"#9aa4b0\"\n";
    let ghost = format!(
      "{fine}[[condition]]\nname = \"c\"\nemotions = [ {{ emotion = \"ghost\", magnitude = 1 }} ]\n"
    );
    let e = load(&[src("t.toml", &ghost)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown emotion `ghost`")), "{e:?}");
    let big = format!(
      "{fine}[[condition]]\nname = \"c\"\nemotions = [ {{ emotion = \"fine\", magnitude = 16 }} ]\n"
    );
    let e = load(&[src("t.toml", &big)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("integer 1..=15")), "{e:?}");

    // A trait modifier is the PER-LEVEL array form — a scalar refuses…
    let scalar = format!(
      "{fine}[[trait]]\nname = \"t\"\nemotions = [ {{ emotion = \"fine\", magnitude = 1 }} ]\n"
    );
    let e = load(&[src("t.toml", &scalar)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("per-LEVEL array")), "{e:?}");
    // …and mood is DELETED from the schema (emotions F4), not silently ignored.
    let e = load(&[src("t.toml", "[[condition]]\nname = \"c\"\nmood = 0.2\n")]).unwrap_err();
    assert!(e[0].message.contains("unknown field `mood`"), "{}", e[0].message);
  }

  #[test]
  fn the_food_chain_surfaces_round_trip_and_refuse() {
    // food-chain F4/F5/F6: the need-check affordance, the spawn effect, the remove
    // effect — round-trips first.
    let text = r##"
[[need]]
name = "corpus"
min = 0
max = 2

[[thing]]
name = "meat"
packed = [ { tint = "#a04030" } ]

[[interaction]]
name = "death"
affordances = ["can_die"]
inputs = ["pawn"]
location = "self"
spawn = { thing = "meat", at = "on" }
remove = "target"

[[affordance]]
name = "can_die"
check = { need = "corpus", lte = 0.0 }
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    let ap = b.affordance_params("can_die").expect("can_die");
    assert_eq!(
      ap.check,
      AffordanceCheck::Need { need: "corpus".into(), cmp: crate::loader::Cmp::Lte, value: 0.0 }
    );
    assert_eq!(ap.trigger_need(), Some("corpus"), "the need check IS the trigger key");
    let ip = b.interaction_params("death").expect("death");
    assert_eq!(
      ip.spawn,
      Some(crate::loader::SpawnEffect::Thing { thing: "meat".into(), at: "on".into() })
    );
    assert_eq!(ip.remove.as_deref(), Some("target"));

    // Refusals: unknown need; zero ops; a check naming BOTH forms; spawn's unknown
    // thing + bad `at`; remove's only value.
    let e = load(&[src("t.toml", "[[affordance]]\nname = \"a\"\ncheck = { need = \"ghost\", lte = 0.0 }\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown need `ghost`")), "{e:?}");
    let e = load(&[src("t.toml", "[[need]]\nname = \"n\"\n[[affordance]]\nname = \"a\"\ncheck = { need = \"n\" }\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("exactly ONE of gte/gt/lt/lte")), "{e:?}");
    let e = load(&[src(
      "t.toml",
      "[[need]]\nname = \"n\"\n[[stat]]\nname = \"s\"\nmin = 0\nmax = 1\n[[affordance]]\nname = \"a\"\ncheck = { stat = \"s\", above = 0.0, need = \"n\", lte = 0.0 }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("exactly ONE of `stat`/`need`")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nspawn = { thing = \"ghost\", at = \"on\" }\nremove = \"target\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("spawn names unknown thing")), "{e:?}");
    let e = load(&[src(
      "t.toml",
      "[[thing]]\nname = \"meat\"\npacked = [ { tint = \"#a04030\" } ]\n[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nspawn = { thing = \"meat\", at = \"everywhere\" }\nremove = \"target\"\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("spawn.at")), "{e:?}");
    let e = load(&[src(
      "t.toml",
      "[[thing]]\nname = \"meat\"\npacked = [ { tint = \"#a04030\" } ]\n[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nspawn = { thing = \"meat\", at = \"on\" }\nremove = \"carrier\"\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("`target` is the only value")), "{e:?}");
  }

  #[test]
  fn the_inventory_surfaces_round_trip_and_refuse() {
    // inventory F4/F5: `store = "carrier"`, `location = "slot"`, `spawn = "carried"` —
    // round-trips first (pick_up + drop shapes verbatim).
    let text = r##"
[[interaction]]
name = "pick_up"
inputs = ["pawn", "destination"]
location = "adjacent"
store = "carrier"
duration = 10

[[interaction]]
name = "drop"
inputs = ["pawn", "slot"]
location = "slot"
spawn = "carried"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    let pu = b.interaction_params("pick_up").expect("pick_up");
    assert_eq!(pu.store.as_deref(), Some("carrier"));
    assert_eq!(pu.location, "adjacent");
    let dr = b.interaction_params("drop").expect("drop");
    assert_eq!(dr.spawn, Some(crate::loader::SpawnEffect::Carried));
    assert_eq!(dr.location, "slot");
    // The range check: a slot carrier is definitionally the acting pawn's — no gate.
    assert!(crate::loader::location_in_range("slot", 0));

    // Refusals: store's only value; spawn = "carried" outside a slot location; a
    // misspelled carried word; the unknown-location message names `slot` now.
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nstore = \"self\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("`\"carrier\"` is the only target")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nlocation = \"adjacent\"\nspawn = \"carried\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("requires location = \"slot\"")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nlocation = \"slot\"\nspawn = \"carreid\"\n")])
      .unwrap_err();
    assert!(
      e.iter().any(|e| e.message.contains("a `{ thing, at }` table or the string")),
      "{e:?}"
    );
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"i\"\ninputs = [\"pawn\"]\nlocation = \"pocket\"\nstore = \"carrier\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("`slot`")), "{e:?}");
  }

  #[test]
  fn trait_tags_gate_by_capability() {
    // attack F1: bite authors the tag; can_attack passes for ANY tag bearer —
    // a clawed bear joins by content alone. Round-trip + eval + refusal.
    let text = r##"
[[trait]]
name = "bite"
tags = ["attack"]

[[trait]]
name = "walks"
stats = [ { stat = "ground_speed", add = [24] } ]

[[stat]]
name = "ground_speed"
min = 0
max = 240

[[affordance]]
name = "can_attack"
check = { tag = "attack" }

[[interaction]]
name = "attack"
affordances = ["can_attack"]
inputs = ["pawn", "target", "amount"]
location = "adjacent"
satisfy = { target = "@target", need = "corpus", amount = "@amount" }

[[need]]
name = "corpus"
min = 0
max = 2
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    assert_eq!(b.trait_params("bite").unwrap().tags, vec!["attack".to_string()]);
    assert_eq!(
      b.affordance_params("can_attack").unwrap().check,
      AffordanceCheck::Tag { tag: "attack".into() }
    );
    // Eval: a pawn CARRYING bite passes; one carrying only walks does not.
    let bite_ref = b.gameplay_reference("trait", "bite").unwrap();
    let walks_ref = b.gameplay_reference("trait", "walks").unwrap();
    let row = |r: u32| resonantdust_codec::object::pack_gameplay_row(r, 1);
    let pass = |rows: &[u32]| {
      crate::stat_eval::affordance_passes(&b, "can_attack", rows, &[], &[], &[], 0)
    };
    assert!(pass(&[row(bite_ref)]), "the biter attacks");
    assert!(!pass(&[row(walks_ref)]), "the mere walker does not");
    assert!(!pass(&[]), "the traitless do not");
    // Refusals: a tag nothing authors; a check naming two forms.
    let e = load(&[src("t.toml", "[[affordance]]\nname = \"a\"\ncheck = { tag = \"ghost\" }\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("no trait authors")), "{e:?}");
    let e = load(&[src(
      "t.toml",
      "[[trait]]\nname = \"t\"\ntags = [\"x\"]\n[[stat]]\nname = \"s\"\nmin = 0\nmax = 1\n[[affordance]]\nname = \"a\"\ncheck = { stat = \"s\", above = 0.0, tag = \"x\" }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("exactly ONE of `stat`/`need`/`tag`")), "{e:?}");
  }

  #[test]
  fn trait_emotions_land_per_level() {
    let text = r##"
[[emotion]]
name = "fine"
color = "#9aa4b0"

[[emotion]]
name = "playful"
color = "#c93cb8"

[[trait]]
name = "puppyish"
emotions = [ { emotion = "playful", magnitude = [1, 3] } ]
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    let t = b.trait_params("puppyish").expect("trait");
    assert_eq!(t.levels.len(), 2, "the array length IS the level count");
    assert_eq!((t.levels[0].emotions[0].emotion, t.levels[0].emotions[0].magnitude), (1, 1));
    assert_eq!((t.levels[1].emotions[0].emotion, t.levels[1].emotions[0].magnitude), (1, 3));
  }

  #[test]
  fn pathable_defaults_true_and_authors_false() {
    // pathfinding F1: ABSENCE = pathable on tiles and things alike; only an authored
    // `pathable = false` closes a cell. Unknown/zero ids degrade OPEN (version skew
    // must never freeze a pawn against an invisible wall).
    let text = r##"
[[tile]]
name = "grass"
texture = "white"
tint = "#4b573e"

[[tile]]
name = "water"
texture = "white"
tint = "#2e5a78"
pathable = false

[[thing]]
name = "tree"
pathable = false
packed = [ { tint = "#335533" } ]

[[thing]]
name = "logs"
packed = [ { tint = "#7a5a3a" } ]
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    assert!(b.tile_pathable(b.tile_def_id("grass").unwrap()), "absence = pathable");
    assert!(!b.tile_pathable(b.tile_def_id("water").unwrap()), "water authors false");
    assert!(!b.thing_pathable(b.thing_object_id("tree").unwrap()), "the tree authors false");
    assert!(b.thing_pathable(b.thing_object_id("logs").unwrap()), "absence = pathable");
    assert!(b.tile_pathable(0) && b.tile_pathable(999), "unknown ids degrade open");
    assert!(b.thing_pathable(0) && b.thing_pathable(999), "unknown ids degrade open");
  }

  #[test]
  fn the_gameplay_categories_round_trip_with_resolved_effects() {
    // Stat-model P1: all six categories parse; the F5 operand resolution turns `@refs`
    // into input indices at LOAD; traits carry LEVELED modifier tables; affordances are
    // structured stat predicates listed BY the interaction; carriers bind interactions.
    let text = r##"
[[stat]]
name = "metabolism"
min = 0
max = 10

[[need]]
name = "thirst"
min = 0
max = 100

[[condition]]
name = "quenched"
duration = 3600
needs = [ { need = "thirst", rate = 0.5 } ]

[[trait]]
name = "biological_lifeform"
label = "Biological Lifeform"
stats = [ { stat = "metabolism", add = [1] } ]

[[interaction]]
name = "drink"
affordances = ["can_drink"]
inputs = ["pawn", "need", "amount"]
satisfy = { target = "@pawn", need = "@need", amount = "@amount" }
grant = ["quenched"]

[[affordance]]
name = "can_drink"
check = { stat = "metabolism", above = 0.0 }

[[tile]]
name = "water"
tint = "#2e5a78"
interactions = [ { name = "drink", magnitude = 3 } ]

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
traits = ["biological_lifeform"]
[[thing.part]]
tint = "#ffffff"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");

    // The consumer read path: the interaction's signature + resolved effect + gates.
    let drink = b.interaction_params("drink").expect("drink");
    assert_eq!(drink.affordances, vec!["can_drink"]);
    assert_eq!(drink.inputs, vec!["pawn", "need", "amount"]);
    let satisfy = drink.satisfy.expect("satisfy");
    assert_eq!(satisfy.target, crate::loader::Operand::Input(0));
    assert_eq!(satisfy.need, crate::loader::Operand::Input(1));
    assert_eq!(satisfy.amount, crate::loader::Operand::Input(2));
    assert_eq!(drink.grants, vec!["quenched"]);
    assert_eq!(drink.location, "on");

    // The predicate + the trait's leveled table + the condition's need modifier.
    let can = b.affordance_params("can_drink").expect("can_drink");
    assert_eq!(
        can.check,
        AffordanceCheck::Stat { stat: "metabolism".into(), above: Some(0.0), below: None }
    );
    let bl = b.trait_params("biological_lifeform").expect("trait");
    assert_eq!(bl.levels.len(), 1);
    assert_eq!(bl.levels[0].stats[0].add, 1.0);
    let q = b.condition_params("quenched").expect("quenched");
    assert_eq!((q.needs[0].need.as_str(), q.needs[0].rate), ("thirst", 0.5));

    // Carrier bindings: interactions on the tile, leveled traits on the thing (bare = 1).
    assert_eq!(
      b.tile_interactions(1),
      vec![crate::loader::InteractionBind { name: "drink".into(), magnitude: 3.0, yields: None }]
    );
    assert_eq!(
      b.thing_traits(1),
      vec![crate::loader::TraitBind {
        name: "biological_lifeform".to_string(),
        level: 1,
        constant: false,
      }]
    );

    // Refs pack under the derived taxonomy, and the reverse lookup agrees — `stat` too.
    let a = b.gameplay_reference("affordance", "can_drink").expect("ref");
    assert_eq!(b.gameplay_lookup(a), Some(("affordance".to_string(), "can_drink".to_string())));
    let s = b.gameplay_reference("stat", "metabolism").expect("stat ref");
    assert_eq!(b.gameplay_lookup(s), Some(("stat".to_string(), "metabolism".to_string())));

    // Refusals: a dangling @ref, an unknown grant, an unbuilt location.
    let e = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"drink\"\nsatisfy = { target = \"@ghost\", need = \"x\", amount = 1 }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("@ghost")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\ngrant = [\"ghost\"]\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown condition `ghost`")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\nlocation = \"orbit\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("the built rules are")), "{e:?}");
    // An interaction with NO effect refuses (input-rework F6).
    assert!(e.iter().any(|e| e.message.contains("authors no effect")), "{e:?}");
    // The lumberjack surface: destroy/duration round-trip; destroy is an effect; only
    // `"carrier"` is a legal destroy target (F5); adjacent is a built rule now (F2).
    let b = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"cut_down\"\ndestroy = \"carrier\"\nlocation = \"adjacent\"\n\
       duration = 30\n",
    )])
    .expect("destroy is an effect");
    let cut = b.interaction_params("cut_down").expect("cut_down");
    assert_eq!(cut.destroy.as_deref(), Some("carrier"));
    assert_eq!((cut.location.as_str(), cut.duration), ("adjacent", 30.0));
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\ndestroy = \"self\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("the only target")), "{e:?}");
    // The yield lane (logs-drop F1): the binding round-trips it, and a ghost yield
    // refuses loudly.
    let b = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"cut\"\ndestroy = \"carrier\"\nlocation = \"adjacent\"\n\
       [[thing]]\nname = \"logs\"\n[[thing.part]]\ntint = \"#8a6a42\"\n\
       [[thing]]\nname = \"tree\"\ninteractions = [{ name = \"cut\", yields = \"logs\" }]\n\
       [[thing.part]]\ntint = \"#4a7a3a\"\n",
    )])
    .expect("yield binds");
    assert_eq!(b.thing_interactions(2)[0].yields.as_deref(), Some("logs"));
    let e = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"cut\"\ndestroy = \"carrier\"\n\
       [[thing]]\nname = \"tree\"\ninteractions = [{ name = \"cut\", yields = \"ghost\" }]\n\
       [[thing.part]]\ntint = \"#4a7a3a\"\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("yields unknown thing `ghost`")), "{e:?}");
    // The queue display block (intent-queue-ui F2): round-trip + defaults + refusals.
    let b = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"cut\"\ndestroy = \"carrier\"\nduration = 30\n\
       queue = { hover = \"Chopping\", size = 0.6, background = \"#4a6a3a\", \
       progress = \"ccw\", progress_color = \"#3ad64f\", progress_fill = false, \
       cancelable = true }\n",
    )])
    .expect("queue block loads");
    let qv = &b.interaction_params("cut").expect("cut").queue;
    assert_eq!(qv.hover.as_deref(), Some("Chopping"));
    assert_eq!((qv.size, qv.progress.as_str(), qv.progress_fill, qv.cancelable), (0.6, "ccw", false, true));
    assert_eq!((qv.background, qv.progress_color), (Some(0x4a6a3a), Some(0x3ad64f)));
    let dq = &b.interaction_params("cut").unwrap().queue; // defaults on an UNAUTHORED block:
    let _ = dq;
    let b2 = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\ndestroy = \"carrier\"\n")])
      .expect("no queue block");
    let dv = &b2.interaction_params("d").expect("d").queue;
    assert_eq!((dv.size, dv.progress.as_str(), dv.cancelable), (1.0, "none", false));
    let e = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"d\"\ndestroy = \"carrier\"\nqueue = { progress = \"spiral\" }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("ring directions")), "{e:?}");
    // A carrier binding a ghost interaction refuses too.
    let e = load(&[src("t.toml", "[[tile]]\nname = \"w\"\ninteractions = [{ name = \"x\" }]\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown interaction `x`")), "{e:?}");
    // An inverted domain refuses.
    let e = load(&[src("t.toml", "[[need]]\nname = \"n\"\nmin = 5\nmax = 1\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("below max")), "{e:?}");
  }

  #[test]
  fn the_input_rework_fields_round_trip() {
    // input-rework P1: menu_text (default = label), location "target", the move effect
    // with @ref resolution — the move_to shape verbatim.
    let text = r##"
[[stat]]
name = "ground_speed"
min = 0
max = 240

[[affordance]]
name = "can_move_ground"
check = { stat = "ground_speed", above = 0.0 }

[[interaction]]
name = "move_to"
label = "Move To"
menu_text = "Move To"
affordances = ["can_move_ground"]
inputs = ["pawn", "destination"]
move = { target = "@pawn", to = "@destination" }
location = "target"

[[tile]]
name = "grass"
tint = "#4b573e"
interactions = [ { name = "move_to" } ]
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    let m = b.interaction_params("move_to").expect("move_to");
    assert_eq!(m.menu_text, "Move To");
    assert_eq!(m.location, "target");
    let mv = m.move_effect.expect("move effect");
    assert_eq!(mv.target, crate::loader::Operand::Input(0));
    assert_eq!(mv.to, crate::loader::Operand::Input(1));
    assert!(m.satisfy.is_none());
    assert_eq!(
      b.tile_interactions(1),
      vec![crate::loader::InteractionBind { name: "move_to".into(), magnitude: 0.0, yields: None }]
    );

    // menu_text defaults to the label.
    let b2 = load(&[src(
      "t.toml",
      "[[need]]\nname = \"n\"\n[[interaction]]\nname = \"i\"\nlabel = \"Sip\"\n\
       satisfy = { target = \"@pawn\", need = \"n\", amount = 1 }\ninputs = [\"pawn\"]\n",
    )])
    .expect("loads");
    assert_eq!(b2.interaction_params("i").unwrap().menu_text, "Sip");

    // A dangling move @ref refuses like satisfy's.
    let e = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"m\"\nmove = { target = \"@ghost\", to = \"@ghost\" }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("@ghost")), "{e:?}");
  }

  #[test]
  fn the_stat_model_refusals_are_loud() {
    // A predicate must author exactly one of above/below.
    let e = load(&[src(
      "t.toml",
      "[[stat]]\nname = \"s\"\n[[affordance]]\nname = \"a\"\ncheck = { stat = \"s\" }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("exactly ONE")), "{e:?}");
    // ... and its stat must exist.
    let e = load(&[src(
      "t.toml",
      "[[affordance]]\nname = \"a\"\ncheck = { stat = \"ghost\", above = 0.0 }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown stat `ghost`")), "{e:?}");
    // Trait level arrays must agree on length.
    let e = load(&[src(
      "t.toml",
      "[[stat]]\nname = \"s\"\n[[trait]]\nname = \"t\"\nstats = [ { stat = \"s\", add = [1, 2], min = [0] } ]\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("level arrays disagree")), "{e:?}");
    // A thing binding a level past the trait's table refuses.
    let e = load(&[src(
      "t.toml",
      "[[stat]]\nname = \"s\"\n[[trait]]\nname = \"t\"\nstats = [ { stat = \"s\", add = [1] } ]\n\
       [[thing]]\nname = \"w\"\ntraits = [ { name = \"t\", level = 3 } ]\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("out of range")), "{e:?}");
    // A DERIVED condition may not modify needs (F13 — the circularity cut).
    let e = load(&[src(
      "t.toml",
      "[[need]]\nname = \"n\"\n[[condition]]\nname = \"c\"\nneeds = [ { need = \"n\", rate = 0.5 } ]\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("only TIMED")), "{e:?}");
    // An unknown winner refuses.
    let e = load(&[src("t.toml", "[[stat]]\nname = \"s\"\nwinner = \"median\"\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("`min` or `max`")), "{e:?}");
  }

  #[test]
  fn a_tile_authoring_an_id_is_now_a_load_error() {
    // The cutover's own assertion: `id` is gone from the tile/thing schema, so a corpus still
    // carrying one fails loudly instead of being silently ignored — `deny_unknown_fields` doing
    // the work the id law used to.
    let e = load(&[src("t.toml", "[[tile]]\nid = 1\nname = \"grass\"\n")]).unwrap_err();
    assert!(e[0].message.contains("unknown field `id`"), "{}", e[0].message);
    let e = load(&[src("t.toml", "[[thing]]\nid = 1\nname = \"tree\"\n")]).unwrap_err();
    assert!(e[0].message.contains("unknown field `id`"), "{}", e[0].message);
  }

  #[test]
  fn only_simulation_visible_fields_bump_a_version() {
    // F12's boundary, and the invariant it buys: **same id ⇒ same behaviour**. A tint or a texture
    // change must NOT move the fingerprint (a re-master already propagates through the texture
    // manifest's own hash); a needs/traits change must. (`speed` left the schema —
    // input-rework F8: pace is the derived stat, whose inputs — trait bindings — fingerprint.)
    let base = "[[thing]]\nname = \"wolf\"\n[[thing.part]]\ntint = \"#ffffff\"\n";
    let v = |t: &str| load(&[src("t.toml", t)]).unwrap().thing_sim_version(1).unwrap();

    // ART: a different tint, same behaviour → same version.
    let recoloured = "[[thing]]\nname = \"wolf\"\n[[thing.part]]\ntint = \"#ff0000\"\n";
    assert_eq!(v(base), v(recoloured), "a recolour must NOT bump a version");

    // The apple case in miniature: needs are simulation state too.
    let thirsty = "[[need]]\nname = \"thirst\"\n\
                   [[thing]]\nname = \"wolf\"\nneeds = [\"thirst\"]\n\
                   [[thing.part]]\ntint = \"#ffffff\"\n";
    assert_ne!(v(base), v(thirsty), "gaining a need MUST bump a version");

    // A trait-binding change (the pace input) is simulation-visible too.
    let leveled = "[[stat]]\nname = \"ground_speed\"\nmax = 240\n\
                   [[trait]]\nname = \"walks\"\nstats = [ { stat = \"ground_speed\", add = [24, 12] } ]\n\
                   [[thing]]\nname = \"wolf\"\ntype = \"pawn\"\nkind = \"wolf\"\n\
                   subType = [\"animal\"]\nvariant = [\"0\"]\n\
                   traits = [ { name = \"walks\", level = 2 } ]\n\
                   [[thing.part]]\ntint = \"#ffffff\"\n";
    assert_ne!(v(base), v(leveled), "a trait binding MUST bump a version");
  }

  #[test]
  fn a_tile_version_tracks_height_and_build_not_art() {
    let v = |t: &str| load(&[src("t.toml", t)]).unwrap().tile_sim_version(1).unwrap();
    let base = "[[tile]]\nname = \"wall\"\nheight = 1.0\ntint = \"#fff\"\n";
    assert_eq!(v(base), v("[[tile]]\nname = \"wall\"\nheight = 1.0\ntint = \"#000\"\n"));
    // Height is occlusion and blocking — the simulation reads it.
    assert_ne!(v(base), v("[[tile]]\nname = \"wall\"\nheight = 2.0\ntint = \"#fff\"\n"));
  }

  #[test]
  fn two_versions_of_one_def_coexist_in_the_corpus() {
    // B6/F17: the corpus retains every LIVE version. wolf v0 stays authored beside v1 for as long
    // as any v0 wolf exists in the world, so an old entity's behaviour comes from a block that is
    // still here. This is what makes "an old apple stays an old apple" true rather than promised.
    let b = load(&[src(
      "t.toml",
      r##"
[[need]]
name = "thirst"

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
version = 1
needs = ["thirst"]
"##,
    )])
    .unwrap();
    // Both are present, at their own positions, with their OWN behaviour.
    assert_eq!(b.thing_version(1), Some(0));
    assert_eq!(b.thing_needs(1), Vec::<u32>::new(), "v0 keeps the old behaviour");
    assert_eq!(b.thing_version(2), Some(1));
    assert_eq!(b.thing_needs(2).len(), 1, "v1 has the new one");
    // Same taxonomy, so they are versions of ONE definition rather than two definitions.
    assert_eq!(b.thing_taxonomy(1).unwrap().kind, b.thing_taxonomy(2).unwrap().kind);
  }

  #[test]
  fn a_duplicate_taxonomy_and_version_is_a_load_error() {
    // The id law's successor: sharing a taxonomy is fine (that IS versioning), sharing a
    // (taxonomy, version) pair gives one identity two definitions.
    let dup = r##"
[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]

[[thing]]
name = "wolf_again"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
"##;
    let e = load(&[src("t.toml", dup)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("already defined by")), "{e:?}");
  }

  #[test]
  fn an_injected_registry_wins_and_falls_back() {
    // The P4 seam that makes P5's deletion of `id = N` safe. Today it is a no-op BY CONSTRUCTION
    // — the registry is seeded from these same authored ids — so the test drives it with a
    // deliberately DIFFERENT number to prove the override is actually consulted.
    let b = load(&[src(
      "t.toml",
      "[[tile]]\nname = \"grass\"\ntint = \"#fff\"\n\
       [[tile]]\nname = \"dirt\"\ntint = \"#000\"\n",
    )])
    .unwrap();
    assert_eq!(b.tile_def_id("grass"), Some(1), "the authored id, with no registry");
    assert!(!b.has_registry());

    let mut map = std::collections::HashMap::new();
    // A full packed def whose KIND half is 77 (`kind_id:12 | variant_id:4` → 77 << 4).
    map.insert((true, "grass".to_string()), 0x1000_0000u32 | (77u32 << 4));
    let b = b.with_registry(map);
    assert!(b.has_registry());
    assert_eq!(b.tile_def_id("grass"), Some(77), "the registry OVERRIDES the authored id");
    // A name the registry does not carry falls back, so a partially seeded registry degrades to
    // today's behaviour rather than to nothing.
    assert_eq!(b.tile_def_id("dirt"), Some(2), "unlisted names fall back to the corpus");
    assert_eq!(b.tile_def_id("nope"), None);
  }

  #[test]
  fn the_taxonomy_round_trips_and_derives_the_stem() {
    // The four axes as the corpus authors them, read back off the Bundle, plus the stem the
    // loader now DERIVES instead of the corpus authoring it (definition-registry P3).
    let b = load(&[src(
      "t.toml",
      r##"
[[tile]]
name = "wall_smooth"
type = "biome-tile"
kind = "smooth"
subType = ["default"]
variant = ["wall"]
tint = "#ffffff"
"##,
    )])
    .unwrap();
    let t = b.tile_taxonomy(1).expect("wall_smooth has a taxonomy");
    assert_eq!(t.type_name, "biome-tile");
    assert_eq!(t.sub_type, ["default"]);
    assert_eq!(t.kind, "smooth");
    assert_eq!(t.variant, ["wall"]);
    // A NAMED variant is part of the address — it names a distinct art folder.
    assert_eq!(t.stem(), "biome-tile/default/smooth/wall");
    assert_eq!(b.tile_texture_stems()[0], "biome-tile/default/smooth/wall");
  }

  #[test]
  fn a_numeric_variant_stops_the_stem_at_the_kind() {
    // The other half of the rule: an art VARIATION is an index worldgen rolls per cell, so the
    // stem stops at the kind and the resolver appends the roll (`/4/e`) at draw time.
    let b = load(&[src(
      "t.toml",
      r##"
[[thing]]
name = "tree"
type = "biome-thing"
kind = "conifer"
subType = ["default"]
variant = ["0","1","2"]

  [[thing.part]]
  tint = "#ffffff"
"##,
    )])
    .unwrap();
    let t = b.thing_taxonomy(1).expect("tree has a taxonomy");
    assert_eq!(t.stem(), "biome-thing/default/conifer");
    // F2's cross-product: 1 subType × 3 variants = 3 registry rows.
    assert_eq!(t.tuples(), [("default", "0"), ("default", "1"), ("default", "2")]);
  }

  #[test]
  fn a_half_authored_taxonomy_is_a_load_error() {
    // Absent entirely is fine (the field is additive, I9). HALF-authored is not: it would mint
    // the wrong registry rows silently, which is the one thing the registry exists to prevent.
    let e = load(&[src(
      "t.toml",
      r##"
[[tile]]
name = "g"
type = "biome-tile"
"##,
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("BOTH `type` and `kind`")), "{:?}", e);

    let e = load(&[src(
      "t.toml",
      r##"
[[tile]]
name = "g"
type = "biome-tile"
kind = "grass"
"##,
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("applicability ARRAYS")), "{:?}", e);
  }

  #[test]
  fn unknown_fields_refuse_loudly() {
    // deny_unknown_fields: a typo'd key is a LOAD ERROR, not silence — the TOML
    // answer to the DSL's silently-dropped writes.
    let e = load(&[src("t.toml", "[[tile]]\nname = \"grass\"\ntnit = \"#fff\"\n")]).unwrap_err();
    assert!(e[0].message.contains("tnit"), "{}", e[0].message);
  }

  /// trait-lights P1: the full bind parses (name, level, constant); a bare string
  /// still means level 1, non-constant; the emit_light per-level table loads with the
  /// bound level selecting the tuple; and a bind above the authored count refuses.
  #[test]
  fn constant_binds_and_emit_light_tables_load() {
    let text = r##"
[[trait]]
name = "emit_light"
label = "Emits Light"
emit_light = [
  { color = "#ffd98c", reach = 16, elevation = 2.5, radius = 0.35, flicker = true },
  { color = "#8cbfff", reach = 12, intensity = 0.8, fall_off = 2.0 },
]

[[trait]]
name = "marker"

[[thing]]
name = "torch"
type = "biome-thing"
kind = "torch"
subType = ["default"]
variant = ["0"]
traits = [ { name = "emit_light", level = 2, constant = true } ]
[[thing.part]]
tint = "#ffffff"

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
traits = ["marker"]
[[thing.part]]
tint = "#ffffff"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    // The def's per-level table: two authored light levels.
    let p = b.trait_params("emit_light").expect("def");
    assert_eq!(p.emit_light.len(), 2);
    assert_eq!(p.levels.len(), 2, "emit_light is per-LEVEL — it sets the level count");
    let warm = &p.emit_light[0];
    assert!((warm.color.0 - 1.0).abs() < 2.0 / 255.0 && warm.reach == 16.0);
    assert_eq!((warm.elevation, warm.radius, warm.flicker), (2.5, 0.35, true));
    assert_eq!((warm.intensity, warm.fall_off, warm.cast, warm.hot), (1.0, 1.0, true, false));
    let blue = &p.emit_light[1];
    assert_eq!((blue.reach, blue.intensity, blue.fall_off), (12.0, 0.8, 2.0));
    // The torch's constant bind resolved whole; the wolf's bare string is level 1,
    // non-constant.
    let torch = b.thing_object_id("torch").expect("torch");
    assert_eq!(
      b.thing_traits(torch),
      vec![TraitBind { name: "emit_light".into(), level: 2, constant: true }]
    );
    let wolf = b.thing_object_id("wolf").expect("wolf");
    assert_eq!(
      b.thing_traits(wolf),
      vec![TraitBind { name: "marker".into(), level: 1, constant: false }]
    );
    // A bind above the authored level count refuses exactly like before.
    let over = text.replace("level = 2, constant = true", "level = 3, constant = true");
    let e = load(&[src("t.toml", &over)]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("out of range")), "{e:?}");
  }

  /// trait-lights F5 (P2): the merged accessor — a constant bind appears with the
  /// payload absent; a forged payload row for a constant-bound trait is IGNORED
  /// (constant wins); runtime rows for other traits ride through. And F4/F8 through
  /// `object_lights`: the torch yields its bound level's tuple; five light traits
  /// yield five tuples, nothing drops.
  #[test]
  fn the_merged_accessor_derives_constants_and_lights() {
    let mut text = String::from(
      r##"
[[trait]]
name = "glow_a"
emit_light = [ { color = "#ff0000", reach = 4 } ]

[[trait]]
name = "marker"

[[thing]]
name = "torch"
type = "biome-thing"
kind = "torch"
subType = ["default"]
variant = ["0"]
traits = [ { name = "glow_a", constant = true } ]
[[thing.part]]
tint = "#ffffff"

[[thing]]
name = "beacon"
type = "biome-thing"
kind = "beacon"
subType = ["default"]
variant = ["0"]
traits = [
"##,
    );
    for i in 0..5 {
      // five distinct light traits, all bound constant on one thing (F8: all attach)
      text = format!(
        "[[trait]]\nname = \"g{i}\"\nemit_light = [ {{ color = \"#00ff00\", reach = {} }} ]\n{text}",
        i + 1
      );
    }
    text.push_str(
      &(0..5).map(|i| format!("  {{ name = \"g{i}\", constant = true }},\n")).collect::<String>(),
    );
    text.push_str("]\n[[thing.part]]\ntint = \"#ffffff\"\n");
    let b = load(&[src("t.toml", &text)]).expect("clean load");

    let torch = b.thing_object_id("torch").expect("torch");
    // The constant bind appears with NO payload…
    let rows = b.object_trait_rows(torch, &[]);
    assert_eq!(rows.len(), 1);
    // …and a forged payload row for the SAME trait is ignored (constant wins), while
    // a runtime row for another trait rides through.
    let glow_ref = b.gameplay_reference("trait", "glow_a").expect("ref");
    let marker_ref = b.gameplay_reference("trait", "marker").expect("ref");
    let forged = resonantdust_codec::object::pack_gameplay_row(glow_ref, 7);
    let runtime = resonantdust_codec::object::pack_gameplay_row(marker_ref, 1);
    let merged = b.object_trait_rows(torch, &[forged, runtime]);
    assert_eq!(merged.len(), 2, "constant + the marker row: {merged:?}");
    assert_eq!(merged[0], resonantdust_codec::object::pack_gameplay_row(glow_ref, 1));
    assert_eq!(merged[1], runtime);

    // Lights: the torch yields exactly its bound tuple; the beacon yields five.
    let lights = b.object_lights(torch, &[]);
    assert_eq!(lights.len(), 1);
    assert_eq!(lights[0].reach, 4.0);
    let beacon = b.thing_object_id("beacon").expect("beacon");
    let five = b.object_lights(beacon, &[]);
    assert_eq!(five.len(), 5, "all five attach — nothing drops (F8)");
  }

  /// trait-lights F7/I3: the legacy stride-8 `thing_light` vector derives from the
  /// constant trait BIT-IDENTICALLY to what the retired `light = {}` block authored —
  /// pinned against the torch's exact pre-change values, so every downstream consumer
  /// (the wasm `thingLight` lane, the client's cold baking) is untouched by the move.
  #[test]
  fn thing_light_derives_bit_identically() {
    let text = r##"
[[trait]]
name = "emit_light"
emit_light = [
  { color = [1.0, 0.85, 0.55], intensity = 1.0, reach = 16.0, radius = 0.35, elevation = 2.5, flicker = true },
  { color = [0.55, 0.75, 1.0], intensity = 1.0, reach = 16.0, radius = 0.35, elevation = 2.5, flicker = true },
]

[[thing]]
name = "torch"
type = "biome-thing"
kind = "torch"
subType = ["default"]
variant = ["0"]
traits = [ { name = "emit_light", level = 1, constant = true } ]
[[thing.part]]
tint = "#ffd9a0"

[[thing]]
name = "torch_blue"
type = "biome-thing"
kind = "torch_blue"
subType = ["default"]
variant = ["0"]
traits = [ { name = "emit_light", level = 2, constant = true } ]
[[thing.part]]
tint = "#a0c8ff"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    let v = b.thing_light();
    // The OLD block's exact vectors: [r,g,b,intensity,reach,radius,height,flags];
    // flags = cast(1) | flicker(4) = 5.
    assert_eq!(&v[0..8], &[1.0, 0.85, 0.55, 1.0, 16.0, 0.35, 2.5, 5.0]);
    assert_eq!(&v[8..16], &[0.55, 0.75, 1.0, 1.0, 16.0, 0.35, 2.5, 5.0]);
  }

  /// survival F1: the geo glyph defaults to the name's first char uppercased; an
  /// authored `geo_label` wins whole.
  #[test]
  fn geo_labels_default_and_override() {
    let text = r##"
[[thing]]
name = "bunny"
type = "pawn"
kind = "bunny"
subType = ["animal"]
variant = ["0"]
[[thing.part]]
tint = "#ffffff"

[[thing]]
name = "logs"
type = "biome-thing"
kind = "logs"
subType = ["default"]
variant = ["0"]
geo_label = "Lg"
[[thing.part]]
tint = "#ffffff"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");
    assert_eq!(b.thing_geo_labels(), vec!["B".to_string(), "Lg".to_string()]);
  }

  /// trait-lights F6: a NON-constant trait bind on a non-pawn thing refuses with a
  /// named error — a cold thing has no runtime trait storage to save it to.
  #[test]
  fn a_non_constant_bind_on_a_cold_thing_refuses() {
    let text = r##"
[[trait]]
name = "marker"

[[thing]]
name = "crate"
type = "biome-thing"
kind = "crate"
subType = ["default"]
variant = ["0"]
traits = ["marker"]
[[thing.part]]
tint = "#ffffff"
"##;
    let e = load(&[src("t.toml", text)]).unwrap_err();
    assert!(
      e.iter().any(|e| e.message.contains("constant = true")),
      "the refusal must name the fix: {e:?}"
    );
  }
}
