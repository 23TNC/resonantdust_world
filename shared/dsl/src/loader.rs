//! Content loader — parse a set of `.rd` sources into one ready-to-query
//! [`Bundle`].
//!
//! This is the entry point both sides use to turn on-disk content into something
//! executable. The server links it as an rlib and asks "what `def_id` is
//! `grass`?" to pack a zone's terrain; the client reaches it through the wasm
//! bundle and asks "what colour is this `def_id`?" to paint a tile. Same content,
//! same ids, both sides — which only works because the id scheme is derived from
//! the content itself, not assigned out of band.
//!
//! A tile is a `<tile>` def with two facets authored in separate files:
//!   - `:data` (content/data/…) — the simulation side the server reads.
//!   - `:visual` (content/visual/…) — the rendering side the client reads.
//! Each facet holds lifecycle hooks (`@define` / `@on_create` / `@on_update` /
//! `@on_destroy`); for now only `@define` is run, to materialise a tile's static
//! aspects (e.g. `visual.color.bg`).

use crate::parser::{Header, Node};
use crate::vm::{run, Store};
use std::collections::HashMap;

/// A single load-time problem (a parse error, tagged with its file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
  pub file: String,
  pub message: String,
}

/// The outcome of running a biome's lifecycle on one tile: which biome claimed
/// the cell and what it decided to place. `tile` names the ground (floor / wall /
/// cliff), `thing1` the primary-layer thing (a tree, a shrub — `None` when the
/// biome scattered nothing on this cell). Names, not ids: worldgen resolves them
/// to `def_id`s through this same bundle before packing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GenTile {
  /// The biome whose `@define` claimed this cell (`None` if none matched).
  pub biome: Option<String>,
  /// The ground tile name the biome's `@on_create` set (`&tile set`).
  pub tile: Option<String>,
  /// The primary thing-layer name (`&thing.1 set`), or `None` if unset.
  pub thing1: Option<String>,
}

/// The visual parts a def's `:visual @on_create` prim declares. `tint` multiplies
/// a LOADED texture (usually white/no-op for full-colour art); `geo_color` is the
/// flat silhouette colour shown while the prim is still on the geo tier (defaults
/// to `tint` when the def sets none); `texture` is the sprite STEM (`None` when the
/// def declares no texture, or declares the built-in `"white"` fill). Shared by
/// tiles and things — both author the same prim shape.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualParts {
  pub tint: u32,
  pub geo_color: u32,
  pub texture: Option<String>,
  /// The sprite's DRIVING (min) axis footprint, or `0.0` when the def sets none
  /// (the host applies its own default). Host-interpreted units — the renderer
  /// treats it as **pixels** (e.g. `64` = one tile wide) and derives the other
  /// axis from the texture's aspect, bottom-anchoring + z-sorting by base row.
  pub size: f64,
}

/// Everything the runtime needs to resolve and render tiles, built once at load.
#[derive(Default, Debug)]
pub struct Bundle {
  /// Tile defs by name (the `::id` node — its `:data` / `:visual` facets merged
  /// across files), navigable for hooks.
  tiles: HashMap<String, Node>,
  /// Tile names ordered by `def_id`: a tile's `def_id` is `index + 1` (1-based;
  /// `0` is the empty/no-tile sentinel the codec reserves). **First-appearance
  /// order** across the loaded sources, so appending a new tile never shifts an
  /// existing id — already-stored zones stay valid. Server and client agree
  /// because both load the same canonically-ordered content.
  tile_ids: Vec<String>,
  /// Thing defs by name — the same shape as tiles but a **separate** id
  /// namespace: a thing's `object_id` is `index + 1` in `thing_ids`, packed into
  /// a zone's thing entries ([`resonantdust_codec::packed::pack_thing`]). A biome
  /// names a thing (`tree`); worldgen resolves it here.
  things: HashMap<String, Node>,
  /// Thing names ordered by `object_id` (1-based; `0` = empty). First-appearance
  /// order, append-stable, same discipline as `tile_ids`.
  thing_ids: Vec<String>,
  /// Biome defs by name, navigable for their `@define` / `@on_create` hooks.
  biomes: HashMap<String, Node>,
  /// Biome names in **evaluation order** (first-appearance across sources). A
  /// tile takes the FIRST biome whose `@define` returns non-zero, so a catch-all
  /// biome (an always-true define) belongs last. Not an id namespace — biomes are
  /// a generation-time classifier, never packed into a zone.
  biome_ids: Vec<String>,
}

impl Bundle {
  /// The tile def `name` (its merged `:data` + `:visual` facets), or `None`.
  pub fn tile(&self, name: &str) -> Option<&Node> {
    self.tiles.get(name)
  }
  /// Every tile name in `def_id` order (index 0 → def_id 1).
  pub fn tile_names(&self) -> &[String] {
    &self.tile_ids
  }
  /// The `def_id` for a tile name (1-based; `None` if unknown). This is the u12
  /// packed into a zone's tile slot (`resonantdust_codec::packed::pack_tile`).
  pub fn tile_def_id(&self, name: &str) -> Option<u16> {
    self.tile_ids.iter().position(|n| n == name).map(|i| i as u16 + 1)
  }
  /// The tile name for a `def_id` (`def_id == 0` is the empty sentinel).
  pub fn tile_name(&self, def_id: u16) -> Option<&str> {
    (def_id != 0)
      .then(|| self.tile_ids.get(def_id as usize - 1))
      .flatten()
      .map(String::as_str)
  }

  /// Run one of a tile's `:<facet> @<hook>` bodies into a fresh store — the
  /// slots / prims that hook builds. `None` if the tile, facet, or hook is
  /// absent.
  pub fn run_hook(&self, name: &str, facet: &str, hook: &str) -> Option<Store> {
    self.run_node_hook(self.tile(name)?, facet, hook)
  }

  /// Run a `:<facet> @<hook>` body of an arbitrary def node (a tile or a thing —
  /// they share the `:visual @on_create` shape) into a fresh store. The node-keyed
  /// core [`run_hook`] and the thing colour lookup both delegate here.
  fn run_node_hook(&self, node: &Node, facet: &str, hook: &str) -> Option<Store> {
    let h = node.facet(facet)?.hook(hook)?;
    let mut store = Store::default();
    let _ = run(&h.body, &mut store);
    Some(store)
  }

  /// Whether a tile declares a given lifecycle hook — e.g.
  /// `tile_has_hook("grass", "visual", "on_tic")` so the runtime only attaches a
  /// tic to tiles that actually handle one.
  pub fn tile_has_hook(&self, name: &str, facet: &str, hook: &str) -> bool {
    self.tile(name).and_then(|t| t.facet(facet)).and_then(|f| f.hook(hook)).is_some()
  }

  /// A def's background colour as a packed `0xRRGGBB`. In the current model the
  /// visual lifecycle builds a prim and tints it, so the colour is that prim's
  /// tint: run `:visual @on_create` and read the first prim's `tint`. Falls back
  /// to a static `color.bg` / `visual.color.bg` set in an `@export` / `@define`
  /// hook (the older shape), so a half-migrated corpus still resolves. `None` if
  /// the def declares no colour any of those ways. Shared by tiles and things —
  /// both author the same `:visual @on_create` prim+tint.
  fn node_color_bg(&self, node: &Node) -> Option<u32> {
    if let Some(store) = self.run_node_hook(node, "visual", "on_create") {
      if let Some(c) = store.read("prims.0.tint") {
        return Some(c.as_int() as u32);
      }
    }
    for hook in ["export", "define"] {
      if let Some(store) = self.run_node_hook(node, "visual", hook) {
        let c = store.read("color.bg").or_else(|| store.read("visual.color.bg"));
        if let Some(c) = c {
          return Some(c.as_int() as u32);
        }
      }
    }
    None
  }

  /// A def's [`VisualParts`] — its `:visual @on_create` prim's tint, geo colour,
  /// and texture stem, in one hook run. `None` if the def builds no prim (no
  /// tint). The richer sibling of [`node_color_bg`]: same hook, all three fields
  /// the renderer wants instead of just the tint.
  fn node_visual(&self, node: &Node) -> Option<VisualParts> {
    let store = self.run_node_hook(node, "visual", "on_create")?;
    let tint = store.read("prims.0.tint")?.as_int() as u32;
    let geo_color = store.read("prims.0.geoColor").map(|c| c.as_int() as u32).unwrap_or(tint);
    let texture = match store.read("prims.0.texture") {
      Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
      _ => None,
    };
    let size = store.read("prims.0.size").map(|c| c.as_f64()).unwrap_or(0.0);
    Some(VisualParts { tint, geo_color, texture, size })
  }

  /// A tile's background colour as a packed `0xRRGGBB` (see [`node_color_bg`]).
  pub fn tile_color_bg(&self, name: &str) -> Option<u32> {
    self.node_color_bg(self.tile(name)?)
  }

  /// Same as [`tile_color_bg`] but keyed by `def_id` — the lookup the client does
  /// per cell once it has a zone's packed tiles.
  pub fn color_bg_for_def(&self, def_id: u16) -> Option<u32> {
    self.tile_color_bg(self.tile_name(def_id)?)
  }

  /// The [`VisualParts`] for a tile `def_id` — the per-cell lookup the painter's
  /// prim expansion runs (tint + geo colour in one hook run; the stem it indexes
  /// from [`tile_texture_stems`]).
  pub fn visual_for_def(&self, def_id: u16) -> Option<VisualParts> {
    self.node_visual(self.tile(self.tile_name(def_id)?)?)
  }

  /// Every tile's texture stem in `def_id` order (index 0 → def_id 1); the empty
  /// string for a def declaring no texture (or the built-in `"white"` fill). The
  /// client fetches this once and indexes it by the `def_id` the prim expansion
  /// emits — an empty stem means "flat tint, no sprite".
  pub fn tile_texture_stems(&self) -> Vec<String> {
    self
      .tile_ids
      .iter()
      .map(|name| self.tile(name).and_then(|n| self.node_visual(n)).and_then(|v| v.texture).unwrap_or_default())
      .collect()
  }

  // ---------- things ----------

  /// The thing def `name` (its merged facets), or `None`.
  pub fn thing(&self, name: &str) -> Option<&Node> {
    self.things.get(name)
  }
  /// Every thing name in `object_id` order (index 0 → object_id 1).
  pub fn thing_names(&self) -> &[String] {
    &self.thing_ids
  }
  /// The `object_id` for a thing name (1-based; `None` if unknown). This is the
  /// u12 packed into a zone's thing entry
  /// ([`resonantdust_codec::packed::pack_thing`]).
  pub fn thing_object_id(&self, name: &str) -> Option<u16> {
    self.thing_ids.iter().position(|n| n == name).map(|i| i as u16 + 1)
  }
  /// The thing name for an `object_id` (`0` is the empty sentinel).
  pub fn thing_name(&self, object_id: u16) -> Option<&str> {
    (object_id != 0)
      .then(|| self.thing_ids.get(object_id as usize - 1))
      .flatten()
      .map(String::as_str)
  }
  /// A thing's colour as a packed `0xRRGGBB` — its `:visual @on_create` prim tint
  /// (see [`node_color_bg`]), the thing-layer sibling of [`tile_color_bg`].
  pub fn thing_color_bg(&self, name: &str) -> Option<u32> {
    self.node_color_bg(self.thing(name)?)
  }
  /// [`thing_color_bg`] keyed by `object_id` — the per-thing lookup the client
  /// runs over a zone's packed things.
  pub fn thing_color_for_object(&self, object_id: u16) -> Option<u32> {
    self.thing_color_bg(self.thing_name(object_id)?)
  }

  /// The [`VisualParts`] for a thing `object_id` — the thing-layer sibling of
  /// [`visual_for_def`], used for both cold things and object-shard free things.
  pub fn visual_for_object(&self, object_id: u16) -> Option<VisualParts> {
    self.node_visual(self.thing(self.thing_name(object_id)?)?)
  }

  /// Every thing's texture stem in `object_id` order (index 0 → object_id 1); the
  /// empty string for a def declaring no texture. The thing-layer sibling of
  /// [`tile_texture_stems`].
  pub fn thing_texture_stems(&self) -> Vec<String> {
    self
      .thing_ids
      .iter()
      .map(|name| self.thing(name).and_then(|n| self.node_visual(n)).and_then(|v| v.texture).unwrap_or_default())
      .collect()
  }

  /// Every thing's sprite driving-axis footprint in `object_id` order (index 0 →
  /// object_id 1); `0.0` for a def that sets no size (the host applies its
  /// default). Host-interpreted units (pixels) — the min axis; the other comes
  /// from the texture aspect.
  pub fn thing_sizes(&self) -> Vec<f64> {
    self
      .thing_ids
      .iter()
      .map(|name| self.thing(name).and_then(|n| self.node_visual(n)).map(|v| v.size).unwrap_or(0.0))
      .collect()
  }

  // ---------- biomes ----------

  /// Every biome name in evaluation order (first match wins; see [`generate`]).
  pub fn biome_names(&self) -> &[String] {
    &self.biome_ids
  }
  /// The biome def `name` (its `@define` / `@on_create` hooks), or `None`.
  pub fn biome(&self, name: &str) -> Option<&Node> {
    self.biomes.get(name)
  }

  /// Run one of a biome's hooks (`define` / `on_create`) against a store seeded
  /// with this tile's `dims` and rng `seed`. Returns the finished store and the
  /// hook's return value. Biome hooks sit directly under the `::def` (no facet),
  /// unlike tiles. `None` if the biome or hook is absent.
  fn run_biome_hook(&self, name: &str, hook: &str, dims: &[f64], seed: u64) -> Option<(Store, i64)> {
    let h = self.biome(name)?.hook(hook)?;
    let mut store = Store::default();
    store.set_biome(dims.to_vec());
    store.set_seed(seed);
    let ret = run(&h.body, &mut store).unwrap_or(0);
    Some((store, ret))
  }

  /// The first biome whose `@define` returns non-zero for these `dims`, in
  /// evaluation order — the biome that owns this cell. `None` if none matched
  /// (worldgen falls back to a default tile). `seed` is available to `@define`
  /// too, though classification is normally a pure function of `dims`.
  pub fn select_biome(&self, dims: &[f64], seed: u64) -> Option<&str> {
    self.biome_ids.iter().find_map(|name| {
      let (_, verdict) = self.run_biome_hook(name, "define", dims, seed)?;
      (verdict != 0).then_some(name.as_str())
    })
  }

  /// Classify one tile end to end: pick its biome by `dims`, then run that
  /// biome's `@on_create` (seeded with `dims` + `seed`) to read the ground `tile`
  /// and any primary-layer `thing.1` it placed. This is the per-tile entry point
  /// worldgen calls for every cell — the DSL decides *what*, worldgen packs it.
  /// A biome with no `@on_create` (or that set nothing) yields empty fields.
  pub fn generate(&self, dims: &[f64], seed: u64) -> GenTile {
    let Some(biome) = self.select_biome(dims, seed).map(str::to_string) else {
      return GenTile::default();
    };
    let mut gen = GenTile { biome: Some(biome.clone()), ..GenTile::default() };
    if let Some((store, _)) = self.run_biome_hook(&biome, "on_create", dims, seed) {
      gen.tile = read_sym(&store, "tile");
      gen.thing1 = read_sym(&store, "thing.1");
    }
    gen
  }
}

/// Read a slot as a content name: a `Sym` (`grass &tile set`) yields the string;
/// anything else (an unset slot reads as `0`) yields `None`.
fn read_sym(store: &Store, path: &str) -> Option<String> {
  match store.read(path) {
    Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
    _ => None,
  }
}

/// Parse every `(name, source)` into a [`Bundle`], merging each tile's facets
/// across files. Returns the bundle, or every parse error found.
pub fn load(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  let mut errors = Vec::new();
  let mut bundle = Bundle::default();

  for (name, text) in sources {
    match crate::parser::parse(text) {
      Ok(node) => {
        // Tiles and things share the merge-and-number discipline (separate id
        // namespaces); biomes are keyed by name (order = evaluation priority).
        index_defs(&node, "tile", &mut bundle.tiles, &mut bundle.tile_ids);
        index_defs(&node, "thing", &mut bundle.things, &mut bundle.thing_ids);
        index_defs(&node, "biome", &mut bundle.biomes, &mut bundle.biome_ids);
      }
      Err(e) => errors.push(LoadError { file: name.clone(), message: format!("parse: {e}") }),
    }
  }

  if errors.is_empty() {
    Ok(bundle)
  } else {
    Err(errors)
  }
}

/// Index every `<{bucket}>` def by name into `defs`, recording first-appearance
/// order in `ids`. A def seen again (e.g. a tile's other facet, authored in a
/// separate file) folds its facets onto the existing node rather than clobbering
/// it — the loader is typically fed `:data` first, then `:visual` merges on.
/// Append-only: a later fragment never introduces or renumbers a def, so ids stay
/// stable across content edits. Shared by `<tile>` / `<thing>` / `<biome>` (the
/// id vec is meaningful for the first two, evaluation order for the last).
fn index_defs(node: &Node, bucket_name: &str, defs: &mut HashMap<String, Node>, ids: &mut Vec<String>) {
  for bucket in &node.children {
    if bucket.header != Header::Bucket(bucket_name.into()) {
      continue;
    }
    for d in &bucket.children {
      if let Header::Def(id) = &d.header {
        // Strip any inline `::name:facet>` suffix down to the bare def name.
        let key = id.split(':').next().unwrap_or(id).to_string();
        if let Some(existing) = defs.get_mut(&key) {
          existing.children.extend(d.children.iter().cloned());
        } else {
          defs.insert(key.clone(), d.clone());
          ids.push(key);
        }
      }
    }
  }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
  use super::*;

  fn src(name: &str, text: &str) -> (String, String) {
    (name.to_string(), text.to_string())
  }

  /// The two facets of grass/dirt, authored the way content/ splits them: a data
  /// file and a visual file, loaded data-first.
  fn corpus() -> Vec<(String, String)> {
    let data = "\
<tile>
  ::grass>
    :data>
      @define>
        0 return
  ::dirt>
    :data>
      @define>
        0 return
";
    let visual = "\
<tile>
  ::grass>
    :visual>
      @define>
        #4b573e &visual.color.bg set
        0 return
  ::dirt>
    :visual>
      @define>
        #7d6144 &visual.color.bg set
        0 return
";
    vec![src("data/tiles.rd", data), src("visual/tiles.rd", visual)]
  }

  #[test]
  fn indexes_tiles_with_stable_ids() {
    let b = load(&corpus()).expect("clean load");
    // first-appearance order: grass=1, dirt=2
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("dirt"), Some(2));
    assert_eq!(b.tile_name(1), Some("grass"));
    assert_eq!(b.tile_name(2), Some("dirt"));
    // 0 is the empty sentinel; an unknown name has no id
    assert_eq!(b.tile_name(0), None);
    assert_eq!(b.tile_def_id("stone"), None);
  }

  #[test]
  fn merges_data_and_visual_facets() {
    let b = load(&corpus()).expect("clean load");
    // both facets navigable on the one merged def
    assert!(b.tile("grass").unwrap().facet("data").is_some());
    assert!(b.tile("grass").unwrap().facet("visual").is_some());
  }

  #[test]
  fn extracts_visual_color_bg_fallback_static_shape() {
    // The older shape (a static `visual.color.bg` in a define hook) still
    // resolves via the fallback path.
    let b = load(&corpus()).expect("clean load");
    assert_eq!(b.tile_color_bg("grass"), Some(0x4b573e));
    assert_eq!(b.tile_color_bg("dirt"), Some(0x7d6144));
    assert_eq!(b.color_bg_for_def(1), Some(0x4b573e));
    assert_eq!(b.color_bg_for_def(2), Some(0x7d6144));
  }

  /// The current content shape: the visual lifecycle builds a tile prim in
  /// `@on_create` and tints it. Grass also authors `@on_destroy`; dirt is a
  /// bare static tile (create only).
  fn corpus_lifecycle() -> Vec<(String, String)> {
    let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n  ::dirt>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<tile>
  ::grass>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        \"white &tile.texture set
        #4b573e &tile.tint set
        0 return
      @on_destroy>
        &tile.destroy call drop
        0 return
  ::dirt>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        \"white &tile.texture set
        #653d00 &tile.tint set
        0 return
";
    vec![src("data/tiles.rd", data), src("visual/tiles.rd", visual)]
  }

  #[test]
  fn colour_comes_from_the_on_create_prim_tint() {
    let b = load(&corpus_lifecycle()).expect("clean load");
    // colour = the tile prim's tint built in @on_create
    assert_eq!(b.tile_color_bg("grass"), Some(0x4b573e));
    assert_eq!(b.tile_color_bg("dirt"), Some(0x653d00));
    assert_eq!(b.color_bg_for_def(1), Some(0x4b573e));
    assert_eq!(b.color_bg_for_def(2), Some(0x653d00));
  }

  #[test]
  fn visual_parts_carry_stem_and_geo_color() {
    let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n  ::stone>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<tile>
  ::grass>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        \"white &tile.texture set
        #4b573e &tile.tint set
        0 return
  ::stone>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        \"linked/wall_smooth &tile.texture set
        #ffffff &tile.tint set
        #6b6b6b &tile.geoColor set
        0 return
";
    let b = load(&[src("data/tiles.rd", data), src("visual/tiles.rd", visual)]).expect("load");
    // grass: built-in white fill, geo colour defaults to the tint (none authored)
    let grass = b.visual_for_def(1).unwrap();
    assert_eq!(grass.tint, 0x4b573e);
    assert_eq!(grass.geo_color, 0x4b573e);
    assert_eq!(grass.texture.as_deref(), Some("white"));
    // stone: a real stem, tint left white (no-op on the sprite), a distinct geo
    // silhouette colour shown until the sprite loads
    let stone = b.visual_for_def(2).unwrap();
    assert_eq!(stone.tint, 0xffffff);
    assert_eq!(stone.geo_color, 0x6b6b6b);
    assert_eq!(stone.texture.as_deref(), Some("linked/wall_smooth"));
    // the stem table, in def_id order (index 0 → def_id 1)
    assert_eq!(b.tile_texture_stems(), vec!["white".to_string(), "linked/wall_smooth".to_string()]);
  }

  #[test]
  fn thing_stem_and_size_tables() {
    let data = "<thing>\n  ::tree>\n    :data>\n      @define>\n        0 return\n  ::shrub>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<thing>
  ::tree>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        \"world/conifer &thing.texture set
        #ffffff &thing.tint set
        2.0 &thing.size set
        0 return
  ::shrub>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        \"white &thing.texture set
        #5a6e3a &thing.tint set
        0 return
";
    let b = load(&[src("data/things.rd", data), src("visual/things.rd", visual)]).expect("load");
    // tree carries the conifer stem + a 2-tile footprint; shrub is a flat white
    // fill at the host default (size unset → 0.0).
    assert_eq!(b.thing_texture_stems(), vec!["world/conifer".to_string(), "white".to_string()]);
    assert_eq!(b.thing_sizes(), vec![2.0, 0.0]);
  }

  #[test]
  fn detects_authored_lifecycle_hooks() {
    let b = load(&corpus_lifecycle()).expect("clean load");
    // both author @on_create; grass also has @on_destroy, dirt does not — so the
    // runtime can attach only the hooks a tile actually declares.
    assert!(b.tile_has_hook("grass", "visual", "on_create"));
    assert!(b.tile_has_hook("dirt", "visual", "on_create"));
    assert!(b.tile_has_hook("grass", "visual", "on_destroy"));
    assert!(!b.tile_has_hook("dirt", "visual", "on_destroy"));
    // neither authors @on_update (both are static), so neither joins the loop.
    assert!(!b.tile_has_hook("grass", "visual", "on_update"));
  }

  #[test]
  fn appending_a_tile_does_not_renumber() {
    let mut srcs = corpus();
    // a new tile whose name sorts first must still land LAST by id.
    srcs.push(src(
      "data/aaa.rd",
      "<tile>\n  ::aaa>\n    :data>\n      @define>\n        0 return\n",
    ));
    let b = load(&srcs).unwrap();
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("dirt"), Some(2));
    assert_eq!(b.tile_def_id("aaa"), Some(3));
  }

  /// A tile corpus plus thing defs and a couple of biomes — the full shape
  /// worldgen loads. Forest is specific (temperate + humid + above water) and
  /// scatters trees; plains is the always-true catch-all, ordered last.
  fn biome_corpus() -> Vec<(String, String)> {
    let data = "\
<tile>
  ::grass>
    :data>
      @define>
        0 return
  ::dirt>
    :data>
      @define>
        0 return
  ::water>
    :data>
      @define>
        0 return
";
    let things = "\
<thing>
  ::tree>
    :data>
      @define>
        0 return
  ::shrub>
    :data>
      @define>
        0 return
";
    let biome = "\
<biome>
  ::ocean>
    @define>
      ^biome call &b set
      *b.2 0.35 lt return
    @on_create>
      water &tile set
      0 return
  ::forest>
    @define>
      ^biome call &b set
      *b.2 0.35 ge *b.1 0.55 ge and return
    @on_create>
      grass &tile set
      1 ^rand call 0.999 lt if tree &thing.1 set
      0 return
  ::plains>
    @define>
      1 return
    @on_create>
      grass &tile set
      0 return
";
    vec![
      src("data/tiles.rd", data),
      src("data/things.rd", things),
      src("biome/biomes.rd", biome),
    ]
  }

  #[test]
  fn indexes_things_in_their_own_id_namespace() {
    let b = load(&biome_corpus()).expect("clean load");
    // things number from 1, independent of the tile namespace
    assert_eq!(b.thing_object_id("tree"), Some(1));
    assert_eq!(b.thing_object_id("shrub"), Some(2));
    assert_eq!(b.thing_name(1), Some("tree"));
    assert_eq!(b.thing_name(0), None);
    // tile ids are untouched by the thing namespace
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("water"), Some(3));
  }

  #[test]
  fn select_biome_takes_first_matching_define() {
    let b = load(&biome_corpus()).expect("clean load");
    // low elevation → ocean claims it first
    assert_eq!(b.select_biome(&[0.5, 0.7, 0.2], 0), Some("ocean"));
    // land + humid → forest
    assert_eq!(b.select_biome(&[0.5, 0.7, 0.6], 0), Some("forest"));
    // land + dry → neither ocean nor forest, so the plains catch-all
    assert_eq!(b.select_biome(&[0.5, 0.1, 0.6], 0), Some("plains"));
  }

  #[test]
  fn generate_reads_tile_and_scattered_thing() {
    let b = load(&biome_corpus()).expect("clean load");
    // forest cell: grass ground, and (threshold ~1.0) a scattered tree
    let g = b.generate(&[0.5, 0.7, 0.6], 42);
    assert_eq!(g.biome.as_deref(), Some("forest"));
    assert_eq!(g.tile.as_deref(), Some("grass"));
    assert_eq!(g.thing1.as_deref(), Some("tree"));
    // ocean cell: water ground, no thing layer
    let o = b.generate(&[0.5, 0.7, 0.2], 42);
    assert_eq!(o.tile.as_deref(), Some("water"));
    assert_eq!(o.thing1, None);
  }

  #[test]
  fn generate_is_deterministic() {
    let b = load(&biome_corpus()).expect("clean load");
    assert_eq!(b.generate(&[0.5, 0.7, 0.6], 7), b.generate(&[0.5, 0.7, 0.6], 7));
  }

  #[test]
  fn thing_colour_comes_from_the_visual_prim_tint() {
    // A thing authors the same `:visual @on_create` prim+tint tiles do; the
    // colour lookup resolves it by name and by object_id.
    let visual = "\
<thing>
  ::tree>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        \"white &thing.texture set
        #2f4a2a &thing.tint set
        0 return
";
    let mut srcs = biome_corpus();
    srcs.push(src("visual/things.rd", visual));
    let b = load(&srcs).expect("clean load");
    assert_eq!(b.thing_color_bg("tree"), Some(0x2f4a2a));
    let tree = b.thing_object_id("tree").unwrap();
    assert_eq!(b.thing_color_for_object(tree), Some(0x2f4a2a));
    // an object_id with no visual (shrub is data-only here) resolves to None
    let shrub = b.thing_object_id("shrub").unwrap();
    assert_eq!(b.thing_color_for_object(shrub), None);
  }

  #[test]
  fn surfaces_parse_errors() {
    let errs = load(&[src("bad.rd", "<tile>\n  ::x>\n  10 &data.cost set\n")]).unwrap_err();
    assert!(errs.iter().any(|e| e.file == "bad.rd" && e.message.starts_with("parse:")), "{errs:?}");
  }
}
