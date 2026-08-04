//! The TOML corpus loader (toml-content P2) — `content/*.toml` → the same materialized
//! [`Bundle`] the `.rd` path produced, byte-identical under the golden oracle (F5).
//!
//! Schema: `docs/VARIABLES.md § TOML content schema` — it outranks this file on layouts.
//! The ID LAW (F1) is enforced here: every def authors `id = N` (1-based); a duplicate,
//! a zero, or a missing id REFUSES the load. Holes are legal (a retired id keeps its
//! slot as an empty placeholder forever).

use crate::loader::{
  BiomeBody, BiomeDef, BiomeRules, Bundle, Cmp, DirFrame, LightParts, LoadError, MaterialParams,
  MoodletParams, NeedBand, NeedParams, PackedChannel, ThingDef, TileDef, VisualPart, VisualParts,
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
  moodlet: Vec<MoodletToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TileToml {
  id: u16,
  name: String,
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
  id: u16,
  name: String,
  #[serde(default)]
  speed: Option<u16>,
  #[serde(default)]
  needs: Vec<String>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NeedToml {
  id: u16,
  name: String,
  #[serde(default)]
  label: Option<String>,
  #[serde(default)]
  deplete: f64,
  #[serde(default)]
  band: Vec<BandToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandToml {
  moodlet: String,
  #[serde(default)]
  lo: f64,
  hi: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoodletToml {
  id: u16,
  name: String,
  #[serde(default)]
  label: Option<String>,
  #[serde(default)]
  mood: f64,
  #[serde(default)]
  duration: f64,
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
        all.moodlet.extend(c.moodlet);
      }
      Err(e) => errors.push(LoadError { file: name.clone(), message: format!("toml: {e}") }),
    }
  }
  if !errors.is_empty() {
    return Err(errors);
  }

  let mut b = Bundle::default();

  // Registries with the id law. Materials/needs/moodlets first — visuals and
  // `needs = [...]` resolve names against them.
  let materials = place(&all.material, "material", |m| (m.id, m.name.clone()), &mut errors);
  let needs_slots = place(&all.need, "need", |n| (n.id, n.name.clone()), &mut errors);
  let moodlet_slots = place(&all.moodlet, "moodlet", |m| (m.id, m.name.clone()), &mut errors);
  let tiles = place(&all.tile, "tile", |t| (t.id, t.name.clone()), &mut errors);
  let things = place(&all.thing, "thing", |t| (t.id, t.name.clone()), &mut errors);
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

  b.needs = needs_slots
    .iter()
    .map(|slot| match slot {
      Some(n) => (n.name.clone(), NeedParams {
        label: n.label.clone().unwrap_or_else(|| n.name.clone()),
        deplete: n.deplete,
        bands: n
          .band
          .iter()
          .map(|band| NeedBand { moodlet: band.moodlet.clone(), lo: band.lo, hi: band.hi })
          .collect(),
      }),
      None => (String::new(), NeedParams { label: String::new(), deplete: 0.0, bands: Vec::new() }),
    })
    .collect();

  b.moodlets = moodlet_slots
    .iter()
    .map(|slot| match slot {
      Some(m) => (m.name.clone(), MoodletParams {
        label: m.label.clone().unwrap_or_else(|| m.name.clone()),
        mood: m.mood,
        duration: m.duration,
      }),
      None => (String::new(), MoodletParams { label: String::new(), mood: 0.0, duration: 0.0 }),
    })
    .collect();

  let material_id = |name: &str| -> u16 {
    b.materials.iter().position(|(n, _)| n == name).map(|i| i as u16 + 1).unwrap_or(0)
  };
  let need_id = |name: &str| -> Option<u16> {
    b.needs.iter().position(|(n, _)| n == name && !n.is_empty()).map(|i| i as u16 + 1)
  };

  for slot in &tiles {
    b.tiles.push(match slot {
      Some(t) => tile_def(t, &material_id, &mut errors),
      None => TileDef::default(), // a retired id holds its place
    });
  }
  for slot in &things {
    b.things.push(match slot {
      Some(t) => thing_def(t, &material_id, &need_id, &mut errors),
      None => ThingDef::default(),
    });
  }

  for biome in &all.biome {
    match biome_def(biome) {
      Ok(d) => b.biomes.push(d),
      Err(e) => errors.push(e),
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
fn visual(
  parts_toml: &[PartToml],
  packed_toml: &[PackedToml],
  light_toml: &Option<LightToml>,
  material_id: &dyn Fn(&str) -> u16,
  what: &str,
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
      texture: p.texture.clone(),
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

fn tile_def(t: &TileToml, material_id: &dyn Fn(&str) -> u16, errors: &mut Vec<LoadError>) -> TileDef {
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
  let has_visual = t.texture.is_some() || t.tint.is_some();
  let visual = has_visual
    .then(|| visual(&[part], &t.packed, &None, material_id, &t.name, errors))
    .flatten();
  TileDef {
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
  }
}

fn thing_def(
  t: &ThingToml,
  material_id: &dyn Fn(&str) -> u16,
  need_id: &dyn Fn(&str) -> Option<u16>,
  errors: &mut Vec<LoadError>,
) -> ThingDef {
  let visual = visual(&t.part, &t.packed, &t.light, material_id, &t.name, errors);
  let mut needs = Vec::new();
  for n in t.needs.iter().take(NEEDS_PER_KIND) {
    match need_id(n) {
      Some(id) => needs.push(id),
      None => errors.push(LoadError {
        file: String::new(),
        message: format!("thing `{}`: unknown need `{n}`", t.name),
      }),
    }
  }
  ThingDef {
    name: t.name.clone(),
    color: visual.as_ref().map(|v| v.tint),
    visual,
    speed: t.speed,
    needs,
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
id = 1
name = "grass"
texture = "white"
tint = "#4b573e"

[[tile]]
id = 3
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
id = 2
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
id = 1
name = "thirst"
deplete = 21600
band = [ { moodlet = "thirsty", lo = 0.10, hi = 0.35 } ]

[[moodlet]]
id = 1
name = "thirsty"
label = "Thirsty"
mood = -0.15

[[biome]]
name = "forest"
subtype = 6
when = { humidity = { gte = 0.55 }, elevation = { lt = 0.82 } }
tile = "grass"
scatter = [ { salt = 6, p = 0.99, thing = "wolf" } ]
"##;
    let b = load(&[src("content.toml", text)]).expect("clean load");

    // ids: explicit, with a HOLE at tile 2 (retired).
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("stone"), Some(3));
    assert_eq!(b.tile_name(2), None, "a retired id resolves nothing");
    assert_eq!(b.tile_names().len(), 3);
    assert_eq!(b.tile_texture_stems(), vec!["white".to_string(), String::new(), "linked/wall_smooth".to_string()]);

    // tile lanes + packed + height
    assert_eq!(b.tile_lighting_lanes()[12..18], [4.0, 4.0, 0.5, 0.0, 0.0, 0.0]);
    assert_eq!(b.tile_height(3), Some(1.0));
    assert_eq!(b.tile_packed_channels()[2][0].material_id, 1);
    assert_eq!(b.tile_packed_channels()[2][0].tint, 0x6b6b6b);

    // the wolf: speed, needs, subframe chain (default + e + v2 compose per component)
    assert_eq!(b.thing_object_id("wolf"), Some(2));
    assert_eq!(b.thing_speed(2), Some(12));
    assert_eq!(b.thing_needs(2), vec![1]);
    let v = b.visual_for_object(2).unwrap();
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
    assert_eq!(b.moodlet_params("thirsty").unwrap().mood, -0.15);
  }

  #[test]
  fn the_id_law_refuses_duplicates_and_zero() {
    let dup = r##"
[[tile]]
id = 1
name = "grass"
[[tile]]
id = 1
name = "dirt"
"##;
    let e = load(&[src("t.toml", dup)]).unwrap_err();
    assert!(e[0].message.contains("already taken"), "{}", e[0].message);

    let zero = "[[tile]]\nid = 0\nname = \"grass\"\n";
    let e = load(&[src("t.toml", zero)]).unwrap_err();
    assert!(e[0].message.contains("id 0"), "{}", e[0].message);

    let missing = "[[tile]]\nname = \"grass\"\n";
    let e = load(&[src("t.toml", missing)]).unwrap_err();
    assert!(e[0].message.contains("missing field"), "{}", e[0].message);
  }

  #[test]
  fn unknown_fields_refuse_loudly() {
    // deny_unknown_fields: a typo'd key is a LOAD ERROR, not silence — the TOML
    // answer to the DSL's silently-dropped writes.
    let e = load(&[src("t.toml", "[[tile]]\nid = 1\nname = \"grass\"\ntnit = \"#fff\"\n")]).unwrap_err();
    assert!(e[0].message.contains("tnit"), "{}", e[0].message);
  }
}
