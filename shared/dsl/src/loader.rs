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
    let h = self.tile(name)?.facet(facet)?.hook(hook)?;
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

  /// A tile's background colour as a packed `0xRRGGBB`. In the current model the
  /// visual lifecycle builds a tile prim and tints it, so the colour is that
  /// prim's tint: run `:visual @on_create` and read the first prim's `tint`.
  /// Falls back to a static `color.bg` / `visual.color.bg` set in an `@export` /
  /// `@define` hook (the older shape), so a half-migrated corpus still resolves.
  /// `None` if the tile declares no colour any of those ways.
  pub fn tile_color_bg(&self, name: &str) -> Option<u32> {
    if let Some(store) = self.run_hook(name, "visual", "on_create") {
      if let Some(c) = store.read("prims.0.tint") {
        return Some(c.as_int() as u32);
      }
    }
    for hook in ["export", "define"] {
      if let Some(store) = self.run_hook(name, "visual", hook) {
        let c = store.read("color.bg").or_else(|| store.read("visual.color.bg"));
        if let Some(c) = c {
          return Some(c.as_int() as u32);
        }
      }
    }
    None
  }

  /// Same as [`tile_color_bg`] but keyed by `def_id` — the lookup the client does
  /// per cell once it has a zone's packed tiles.
  pub fn color_bg_for_def(&self, def_id: u16) -> Option<u32> {
    self.tile_color_bg(self.tile_name(def_id)?)
  }
}

/// Parse every `(name, source)` into a [`Bundle`], merging each tile's facets
/// across files. Returns the bundle, or every parse error found.
pub fn load(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  let mut errors = Vec::new();
  let mut bundle = Bundle::default();

  for (name, text) in sources {
    match crate::parser::parse(text) {
      Ok(node) => index_tiles(&node, &mut bundle.tiles, &mut bundle.tile_ids),
      Err(e) => errors.push(LoadError { file: name.clone(), message: format!("parse: {e}") }),
    }
  }

  if errors.is_empty() {
    Ok(bundle)
  } else {
    Err(errors)
  }
}

/// Index `<tile>` defs by name into `tiles`, recording first-appearance order in
/// `tile_ids`. A tile seen again (its other facet, authored in a separate file)
/// folds its facets onto the existing node rather than clobbering it — the loader
/// is typically fed `:data` first, then `:visual` merges on. Append-only: a
/// later facet fragment never introduces or renumbers a def.
fn index_tiles(node: &Node, tiles: &mut HashMap<String, Node>, tile_ids: &mut Vec<String>) {
  for bucket in &node.children {
    if bucket.header != Header::Bucket("tile".into()) {
      continue;
    }
    for d in &bucket.children {
      if let Header::Def(id) = &d.header {
        // Strip any inline `::name:facet>` suffix down to the bare tile name.
        let key = id.split(':').next().unwrap_or(id).to_string();
        if let Some(existing) = tiles.get_mut(&key) {
          existing.children.extend(d.children.iter().cloned());
        } else {
          tiles.insert(key.clone(), d.clone());
          tile_ids.push(key);
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

  #[test]
  fn surfaces_parse_errors() {
    let errs = load(&[src("bad.rd", "<tile>\n  ::x>\n  10 &data.cost set\n")]).unwrap_err();
    assert!(errs.iter().any(|e| e.file == "bad.rd" && e.message.starts_with("parse:")), "{errs:?}");
  }
}
