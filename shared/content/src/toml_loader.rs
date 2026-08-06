//! The TOML corpus loader (toml-content P2) — `content/*.toml` → the same materialized
//! [`Bundle`] the `.rd` path produced, byte-identical under the golden oracle (F5).
//!
//! Schema: `docs/VARIABLES.md § TOML content schema` — it outranks this file on layouts.
//! The ID LAW (F1) is enforced here: every def authors `id = N` (1-based); a duplicate,
//! a zero, or a missing id REFUSES the load. Holes are legal (a retired id keeps its
//! slot as an empty placeholder forever).

use crate::loader::{
  AffordanceParams, BiomeBody, BiomeDef, BiomeRules, Bundle, Cmp, ConditionParams, DirFrame,
  InteractionParams, LightParts, LoadError, MaterialParams, NeedBand, NeedParams, Operand,
  PackedChannel, SatisfyEffect, Taxonomy, ThingDef, TileDef, TraitParams, VisualPart, VisualParts,
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
  #[serde(default, rename = "trait")]
  trait_: Vec<TraitToml>,
  #[serde(default)]
  interaction: Vec<InteractionToml>,
  #[serde(default)]
  affordance: Vec<AffordanceToml>,
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
  /// Affordance bindings (interactions F2) — the water tile's `drink_water 3`.
  #[serde(default)]
  affordances: Vec<AffordanceBindToml>,
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
  speed: Option<u16>,
  #[serde(default)]
  needs: Vec<String>,
  /// The trait names this kind carries (interactions F6).
  #[serde(default)]
  traits: Vec<String>,
  /// Affordance bindings (interactions F2) — a carried thing's drink source, later.
  #[serde(default)]
  affordances: Vec<AffordanceBindToml>,
  #[serde(default)]
  light: Option<LightToml>,
  #[serde(default)]
  packed: Vec<PackedToml>,
  #[serde(default)]
  part: Vec<PartToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LightToml {
  #[serde(default = "one")]
  r: f64,
  #[serde(default = "one")]
  g: f64,
  #[serde(default = "one")]
  b: f64,
  #[serde(default = "one")]
  intensity: f64,
  reach: f64,
  #[serde(default = "quarter")]
  radius: f64,
  #[serde(default = "half")]
  height: f64,
  #[serde(default = "yes")]
  cast: bool,
  #[serde(default)]
  hot: bool,
  #[serde(default)]
  flicker: bool,
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
  /// defaults `0..1` (the pre-F7 fractional domain).
  #[serde(default)]
  min: f64,
  #[serde(default = "one")]
  max: f64,
  #[serde(default)]
  deplete: f64,
  #[serde(default)]
  band: Vec<BandToml>,
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
  mood: f64,
  #[serde(default)]
  duration: f64,
  /// Card sort key, descending; absent = 0. See [`ConditionParams::priority`].
  #[serde(default)]
  priority: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractionToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// The input SIGNATURE — event input words bind these in order (F5).
  #[serde(default)]
  inputs: Vec<String>,
  #[serde(default)]
  satisfy: Option<SatisfyToml>,
  /// TIMED condition grants on execute.
  #[serde(default)]
  grant: Vec<String>,
  /// The placement rule; `"on"` is this turn's only value (F8).
  #[serde(default = "on")]
  location: String,
  /// RESERVED (I9) — interactions are instantaneous; author 0.
  #[serde(default)]
  duration: f64,
}

fn on() -> String {
  "on".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffordanceToml {
  name: String,
  #[serde(default)]
  label: Option<String>,
  /// The trait gate — ALL must be carried.
  #[serde(default)]
  requires: Vec<String>,
  interaction: String,
  /// Which variants of the interaction this affordance offers (F1's variation lane).
  #[serde(default = "default_variants")]
  variants: Vec<String>,
}

fn default_variants() -> Vec<String> {
  vec!["default".into()]
}

/// A CARRIER's affordance binding — the reference plus this carrier's parameters (F2).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffordanceBindToml {
  name: String,
  #[serde(default)]
  magnitude: f64,
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
        all.interaction.extend(c.interaction);
        all.affordance.extend(c.affordance);
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
  unique("trait", all.trait_.iter().map(|d| d.name.as_str()).collect());
  unique("interaction", all.interaction.iter().map(|d| d.name.as_str()).collect());
  unique("affordance", all.affordance.iter().map(|d| d.name.as_str()).collect());
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
      (n.name.clone(), NeedParams {
        label: n.label.clone().unwrap_or_else(|| n.name.clone()),
        min: n.min,
        max: n.max,
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
      (m.name.clone(), ConditionParams {
        label: m.label.clone().unwrap_or_else(|| m.name.clone()),
        mood: m.mood,
        duration: m.duration,
        priority: m.priority,
      })
    })
    .collect();

  b.traits = all
    .trait_
    .iter()
    .map(|t| (t.name.clone(), TraitParams { label: t.label.clone().unwrap_or_else(|| t.name.clone()) }))
    .collect();

  // The name-resolution closures every cross-reference below validates through.
  let condition_exists =
    |name: &str| all.condition.iter().any(|c| c.name == name);
  let need_exists = |name: &str| all.need.iter().any(|n| n.name == name);
  let trait_exists = |name: &str| all.trait_.iter().any(|t| t.name == name);

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
      for g in &i.grant {
        if !condition_exists(g) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("interaction `{}`: grants unknown condition `{g}`", i.name),
          });
        }
      }
      if i.location != "on" {
        errors.push(LoadError {
          file: String::new(),
          message: format!(
            "interaction `{}`: location `{}` — `on` is the only rule built (interactions F8)",
            i.name, i.location
          ),
        });
      }
      (i.name.clone(), InteractionParams {
        label: i.label.clone().unwrap_or_else(|| i.name.clone()),
        inputs: i.inputs.clone(),
        satisfy,
        grants: i.grant.clone(),
        location: i.location.clone(),
        duration: i.duration,
      })
    })
    .collect();

  b.affordances = all
    .affordance
    .iter()
    .map(|a| {
      for r in &a.requires {
        if !trait_exists(r) {
          errors.push(LoadError {
            file: String::new(),
            message: format!("affordance `{}`: requires unknown trait `{r}`", a.name),
          });
        }
      }
      if !all.interaction.iter().any(|i| i.name == a.interaction) {
        errors.push(LoadError {
          file: String::new(),
          message: format!("affordance `{}`: unknown interaction `{}`", a.name, a.interaction),
        });
      }
      (a.name.clone(), AffordanceParams {
        label: a.label.clone().unwrap_or_else(|| a.name.clone()),
        requires: a.requires.clone(),
        interaction: a.interaction.clone(),
        variants: a.variants.clone(),
      })
    })
    .collect();

  let material_id = |name: &str| -> u16 {
    b.materials.iter().position(|(n, _)| n == name).map(|i| i as u16 + 1).unwrap_or(0)
  };
  let affordance_exists = |name: &str| all.affordance.iter().any(|a| a.name == name);

  for slot in &tiles {
    b.tiles.push(match slot {
      Some(t) => tile_def(t, &material_id, &affordance_exists, &mut errors),
      None => TileDef::default(), // a retired id holds its place
    });
  }
  for slot in &things {
    b.things.push(match slot {
      Some(t) => {
        thing_def(t, &material_id, &need_exists, &trait_exists, &affordance_exists, &mut errors)
      }
      None => ThingDef::default(),
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
  light_toml: &Option<LightToml>,
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
    light: light_toml.as_ref().map(|l| LightParts {
      color: (l.r, l.g, l.b),
      intensity: l.intensity,
      reach: l.reach,
      radius: l.radius,
      height: l.height,
      cast: l.cast,
      hot: l.hot,
      flicker: l.flicker,
    }),
    parts,
  })
}

fn tile_def(
  t: &TileToml,
  material_id: &dyn Fn(&str) -> u16,
  affordance_exists: &dyn Fn(&str) -> bool,
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
    .then(|| visual(&[part], &t.packed, &None, material_id, &t.name, tax.as_ref(), errors))
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
    affordances: affordance_binds(&t.affordances, affordance_exists, "tile", &t.name, errors),
  }
}

/// Validate + flatten a carrier's affordance bindings (interactions F2).
fn affordance_binds(
  binds: &[AffordanceBindToml],
  affordance_exists: &dyn Fn(&str) -> bool,
  what: &str,
  name: &str,
  errors: &mut Vec<LoadError>,
) -> Vec<(String, f64)> {
  for b in binds {
    if !affordance_exists(&b.name) {
      errors.push(LoadError {
        file: String::new(),
        message: format!("{what} `{name}`: unknown affordance `{}`", b.name),
      });
    }
  }
  binds.iter().map(|b| (b.name.clone(), b.magnitude)).collect()
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
  trait_exists: &dyn Fn(&str) -> bool,
  affordance_exists: &dyn Fn(&str) -> bool,
  errors: &mut Vec<LoadError>,
) -> ThingDef {
  let tax = taxonomy(&t.type_name, &t.kind, &t.sub_type, &t.variant, &format!("thing `{}`", t.name), errors);
  let visual = visual(&t.part, &t.packed, &t.light, material_id, &t.name, tax.as_ref(), errors);
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
  for tr in &t.traits {
    if !trait_exists(tr) {
      errors.push(LoadError {
        file: String::new(),
        message: format!("thing `{}`: unknown trait `{tr}`", t.name),
      });
    }
  }
  ThingDef {
    version: t.version,
    taxonomy: tax,
    name: t.name.clone(),
    color: visual.as_ref().map(|v| v.tint),
    visual,
    speed: t.speed,
    needs,
    traits: t.traits.clone(),
    affordances: affordance_binds(&t.affordances, affordance_exists, "thing", &t.name, errors),
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
  use crate::loader::load;

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
speed = 12
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

[[condition]]
name = "thirsty"
label = "Thirsty"
mood = -0.15
priority = 20

[[condition]]
name = "dehydrated"
mood = -0.4

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

    // the wolf: speed, needs, subframe chain (default + e + v2 compose per component)
    assert_eq!(b.thing_object_id("wolf"), Some(1));
    assert_eq!(b.thing_speed(1), Some(12));
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
    assert_eq!(b.condition_params("thirsty").unwrap().mood, -0.15);
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
  fn the_gameplay_categories_round_trip_with_resolved_effects() {
    // Interactions P1: trait/interaction/affordance parse, the F5 operand resolution turns
    // `@refs` into input indices at LOAD, carriers bind, and the F6 gate reads a trait SET.
    let text = r##"
[[need]]
name = "thirst"
min = 0
max = 100

[[condition]]
name = "quenched"
duration = 3600

[[trait]]
name = "biological_lifeform"
label = "Biological Lifeform"

[[interaction]]
name = "drink"
inputs = ["pawn", "need", "amount"]
satisfy = { target = "@pawn", need = "@need", amount = "@amount" }
grant = ["quenched"]

[[affordance]]
name = "drink_water"
requires = ["biological_lifeform"]
interaction = "drink"

[[tile]]
name = "water"
tint = "#2e5a78"
affordances = [ { name = "drink_water", magnitude = 3 } ]

[[thing]]
name = "wolf"
traits = ["biological_lifeform"]
[[thing.part]]
tint = "#ffffff"
"##;
    let b = load(&[src("t.toml", text)]).expect("clean load");

    // The consumer read path: the interaction's signature + resolved effect.
    let drink = b.interaction_params("drink").expect("drink");
    assert_eq!(drink.inputs, vec!["pawn", "need", "amount"]);
    let satisfy = drink.satisfy.expect("satisfy");
    assert_eq!(satisfy.target, crate::loader::Operand::Input(0));
    assert_eq!(satisfy.need, crate::loader::Operand::Input(1));
    assert_eq!(satisfy.amount, crate::loader::Operand::Input(2));
    assert_eq!(drink.grants, vec!["quenched"]);
    assert_eq!(drink.location, "on");

    // Carrier bindings + the availability gate over a trait SET (F6).
    assert_eq!(b.tile_affordances(1), vec![("drink_water".to_string(), 3.0)]);
    let traits = b.thing_traits(1);
    assert!(b.affordance_available("drink_water", &traits));
    assert!(!b.affordance_available("drink_water", &[]), "no traits, no drink");
    assert!(!b.affordance_available("ghost", &traits), "unknown affordance is unavailable");

    // Refs pack under the derived taxonomy, and the reverse lookup agrees.
    let a = b.gameplay_reference("affordance", "drink_water").expect("ref");
    assert_eq!(b.gameplay_lookup(a), Some(("affordance".to_string(), "drink_water".to_string())));

    // Refusals: a dangling @ref, an unknown grant, an unbuilt location.
    let e = load(&[src(
      "t.toml",
      "[[interaction]]\nname = \"drink\"\nsatisfy = { target = \"@ghost\", need = \"x\", amount = 1 }\n",
    )])
    .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("@ghost")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\ngrant = [\"ghost\"]\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown condition `ghost`")), "{e:?}");
    let e = load(&[src("t.toml", "[[interaction]]\nname = \"d\"\nlocation = \"adjacent\"\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("only rule built")), "{e:?}");
    // A carrier binding a ghost affordance refuses too.
    let e = load(&[src("t.toml", "[[tile]]\nname = \"w\"\naffordances = [{ name = \"x\" }]\n")])
      .unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("unknown affordance `x`")), "{e:?}");
    // An inverted domain refuses.
    let e = load(&[src("t.toml", "[[need]]\nname = \"n\"\nmin = 5\nmax = 1\n")]).unwrap_err();
    assert!(e.iter().any(|e| e.message.contains("below max")), "{e:?}");
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
    // manifest's own hash); a speed or a height change must.
    let base = "[[thing]]\nname = \"wolf\"\nspeed = 12\n[[thing.part]]\ntint = \"#ffffff\"\n";
    let v = |t: &str| load(&[src("t.toml", t)]).unwrap().thing_sim_version(1).unwrap();

    // ART: a different tint, same behaviour → same version.
    let recoloured = "[[thing]]\nname = \"wolf\"\nspeed = 12\n[[thing.part]]\ntint = \"#ff0000\"\n";
    assert_eq!(v(base), v(recoloured), "a recolour must NOT bump a version");

    // DATA: a different speed is a different wolf to the simulation → different version.
    let faster = "[[thing]]\nname = \"wolf\"\nspeed = 6\n[[thing.part]]\ntint = \"#ffffff\"\n";
    assert_ne!(v(base), v(faster), "a speed change MUST bump a version");

    // The apple case in miniature: needs are simulation state too.
    let thirsty = "[[need]]\nname = \"thirst\"\n\
                   [[thing]]\nname = \"wolf\"\nspeed = 12\nneeds = [\"thirst\"]\n\
                   [[thing.part]]\ntint = \"#ffffff\"\n";
    assert_ne!(v(base), v(thirsty), "gaining a need MUST bump a version");
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
"##,
    )])
    .unwrap();
    // Both are present, at their own positions, with their OWN behaviour.
    assert_eq!(b.thing_version(1), Some(0));
    assert_eq!(b.thing_speed(1), Some(12), "v0 keeps the old speed");
    assert_eq!(b.thing_version(2), Some(1));
    assert_eq!(b.thing_speed(2), Some(9), "v1 has the new one");
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
}
