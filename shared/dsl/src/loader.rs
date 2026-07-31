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
/// All-zero swings = identity (smooth tintable, today's behaviour).
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
  /// The tiles this prim OCCUPIES — `(w, h)`, default `(1, 1)`. Logical grid extent
  /// (movement / hit-testing), authored in the object's own frame; the object's
  /// position is its TOP-LEFT tile. Rotation swaps `w`/`h` for an n/s facing (a 3×2
  /// object becomes 2×3). **Server-side occupancy is deferred** (`docs/components/shared/codec/design/object-model.md`);
  /// today the client uses `footprint` only to resolve the `anchor` and the z-row.
  pub footprint: (f64, f64),
  /// The prim's logical ANCHOR within its footprint — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)` (centre). `(0,0)` is the footprint's top-left tile corner, `(1,1)`
  /// its bottom-right; `(0.5, 1.0)` is bottom-centre (a thing that stands on its
  /// cell's front edge). The `sprite_anchor` point of the art is pinned here.
  pub anchor: (f64, f64),
  /// The sprite's on-screen scale in TILES (square canvas → one value), default `1.0`
  /// (one tile). Masters are square pow2 canvases with the subject letterboxed +
  /// centred (via `bin/art`), so drawing `size × size` shows the sprite at its true
  /// shape through the transparent padding — a `size 3` conifer draws 3 tiles tall.
  /// SUPERSEDED by `span` + `sprite_scale` (def-frame-anchors P5) — kept while legacy
  /// consumers migrate.
  pub size: f64,
  /// The sprite frame's WORLD SPAN in tiles — pow2, ≤ one zone — default `1.0`. The
  /// frame maps onto `span × span` tiles, fixing its px-per-unit (`2^lod / (16·span)`,
  /// a whole pow2 — the def-frame-anchors model); prim width/height then DERIVE from
  /// the sprite's opaque bbox, they are no longer authored.
  pub span: f64,
  /// Pre-atlas sprite scale `(w, h)`, default `(1, 1)`: applied to the decoded sprite
  /// BEFORE it is packed (clipped to the same pow2 frame, transparent-filled,
  /// re-centred on its surface presence). Corrects art proportions (e.g. a head
  /// sprite too large for its body) without breaking whole-px-per-unit.
  pub sprite_scale: (f64, f64),
  /// The pivot ON THE SPRITE that aligns to `anchor` — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)`. `(0.5, 1.0)` = the art's bottom-centre (its "feet"), which pinned
  /// to a bottom-centre `anchor` reproduces the old bottom-anchored placement. A
  /// west (mirrored-east) facing mirrors `x → 1 − x` so the pin stays put.
  pub sprite_anchor: (f64, f64),
  /// The prim's up-to-4 packed-map channel material bindings (`&prim.packed.<i>`),
  /// indexed by packed RGBA channel. Default (all zero) = no material system in
  /// play, so the renderer paints the flat albedo exactly as before.
  pub packed: [PackedChannel; 4],
  /// The LIGHT this kind emits (`&thing.light.*`), or `None` when it emits nothing.
  /// A primitive presents as a billboard, a light, or **both** — a torch is one placed
  /// object with a sprite and a glow (work `2026-07-25-primitive-graph`). Authored
  /// per KIND, not per instance: every torch shines identically, so the client reads
  /// one row by `kind_id` and never pays for it on the wire.
  pub light: Option<LightParts>,
  /// EVERY prim the visual hook built, in `^prim call` order — the pawn PARTS list
  /// (human-pawns P2). `parts[0]` mirrors the flat prim-0 fields above (the wolf's whole
  /// visual = one entry); a human's `@on_create` calls `^prim` twice, so `parts[1]` is
  /// its head. Which STATE feeds each slot's sprite is the payload's business
  /// (`PART(slot, def)`), not the DSL's — the DSL declares only the skeleton.
  pub parts: Vec<VisualPart>,
}

/// One part SLOT of a kind's visual skeleton — the per-prim fields of `prims.N`
/// (human-pawns P2). Slot index = `^prim call` order; slot 0 boxes the carrier.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualPart {
  pub tint: u32,
  pub geo_color: u32,
  /// The sprite STEM (`None` = a flat tint rect / the built-in `white`).
  pub texture: Option<String>,
  /// Which `<part>` files of the resolved leaf this slot draws (`&prim.part`, default 0
  /// — the body files; a human head draws part 1).
  pub part: u32,
  /// The slot's PRE-ATLAS sprite scale (`&prim.scale`, default 1): the art is scaled inside the
  /// slot's own `span × span` frame at atlas ingest, about its `sprite_anchor` pivot. ABSOLUTE —
  /// on a `span 1` slot it reads as tiles of art (the human authors body 0.8, head 0.5).
  ///
  /// It was a multiplier against slot 0's DRAWN size until 2026-07-30. That made the drawn box
  /// disagree with the def's pow2 frame span, and the lighting/shadow card is sized from the
  /// def — so the silhouette came out `1/scale` too big (pawn-part-placement I6). Keep the scale
  /// pre-atlas: it is the only stage where the albedo, the normal, the surface and the opaque
  /// bbox all move together.
  pub scale: f64,
  /// Placement offset in TILES relative to slot 0's anchor (`&prim.offset.x/y`, default 0).
  pub offset: (f64, f64),
  /// The slot's own frame fields, same semantics as the flat prim-0 copies above.
  pub size: f64,
  pub span: f64,
  pub sprite_scale: (f64, f64),
  pub sprite_anchor: (f64, f64),
  pub anchor: (f64, f64),
}

/// A kind's emitted light, authored under `&thing.light.*`. Colour is `0..1`; `reach`
/// and `height` are TILES; `radius` (the area-light emitter, driving penumbra softness)
/// is tiles too. `reach <= 0` means "no light" and the whole struct stays `None`.
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
  /// Animates per frame (flicker, motion) ⇒ the HOT class, re-baked every frame.
  /// Default false: a static torch bakes once, which is what makes many of them cheap.
  pub hot: bool,
  /// Emits DECAY-LIGHTMAP flicker particles (lighting-feel P2). Orthogonal to `hot`:
  /// the base light stays static (cold) while particles carry the motion for ~free.
  pub flicker: bool,
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
  /// Material defs by name — the `<material>` registry (`@on_create` sets its
  /// [`MaterialParams`]). Referenced by a prim's packed channel, resolved to a
  /// 1-based `material_id`; never packed into a zone (a client-render concern).
  materials: HashMap<String, Node>,
  /// Material names ordered by `material_id` (1-based; `0` = none). First-appearance
  /// order, append-stable, same discipline as `tile_ids` — so the client's atlas /
  /// registry lookups stay valid as materials are added.
  material_ids: Vec<String>,
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
    // Spatial layout — footprint (tiles, default 1×1), logical anchor + sprite pivot
    // (0..1, default centre), sprite size (tiles, default 1). Read component-wise so a
    // def sets only what it overrides (`64 &thing.footprint.w set`, `1.0 &thing.anchor.y set`).
    let read_f = |path: &str, dflt: f64| store.read(path).map(|c| c.as_f64()).unwrap_or(dflt);
    let footprint = (read_f("prims.0.footprint.w", 1.0), read_f("prims.0.footprint.h", 1.0));
    let anchor = (read_f("prims.0.anchor.x", 0.5), read_f("prims.0.anchor.y", 0.5));
    let size = read_f("prims.0.size", 1.0);
    // def-frame-anchors P5: `span` (frame world span, pow2 tiles) + `sprite_scale` (pre-atlas
    // scale, re-centred on surface presence at ingest). `span` defaults to 1 tile.
    let span = read_f("prims.0.span", 1.0);
    let sprite_scale = (read_f("prims.0.sprite_scale.w", 1.0), read_f("prims.0.sprite_scale.h", 1.0));
    let sprite_anchor = (read_f("prims.0.sprite_anchor.x", 0.5), read_f("prims.0.sprite_anchor.y", 0.5));
    // The kind's emitted light (`&thing.light.*`). `reach` is the discriminator: a def that
    // never sets it emits nothing and the whole struct stays `None`, so every existing kind is
    // untouched and the client sees exactly what it saw before.
    let light = match read_f("prims.0.light.reach", 0.0) {
      r if r > 0.0 => Some(LightParts {
        color: (
          read_f("prims.0.light.r", 1.0),
          read_f("prims.0.light.g", 1.0),
          read_f("prims.0.light.b", 1.0),
        ),
        intensity: read_f("prims.0.light.intensity", 1.0),
        reach: r,
        radius: read_f("prims.0.light.radius", 0.25),
        height: read_f("prims.0.light.height", 0.5),
        cast: read_f("prims.0.light.cast", 1.0) != 0.0,
        hot: read_f("prims.0.light.hot", 0.0) != 0.0,
        flicker: read_f("prims.0.light.flicker", 0.0) != 0.0,
      }),
      _ => None,
    };
    // Up to 4 packed-map channels: `&prim.packed.<i>.tint` is the channel's base colour
    // (what `split_layers` subtracted into the residual — the canonical reconstruction
    // `residual + Σ packedᵢ·jitter(tintᵢ)` re-adds it, so EVERY produced channel must set
    // its tint or that region loses its colour). `.material` optionally names a registry
    // material whose noise jitter perturbs the tint's hue/chroma. A channel with neither
    // stays `PackedChannel::default()` (material 0, tint 0 = contributes nothing).
    let mut packed: [PackedChannel; 4] = Default::default();
    for (i, ch) in packed.iter_mut().enumerate() {
      let material_id = match store.read(&format!("prims.0.packed.{i}.material")) {
        Some(crate::vm::Cell::Sym(name)) => self.material_id(name).unwrap_or(0),
        _ => 0,
      };
      let tint = store.read(&format!("prims.0.packed.{i}.tint")).map(|c| c.as_int() as u32).unwrap_or(0);
      if material_id != 0 || tint != 0 {
        *ch = PackedChannel { material_id, tint };
      }
    }
    // EVERY prim the hook built, in `^prim call` order — the parts list (human-pawns P2).
    // A prim exists iff `prims.{i}.kind` does (`prims_push` stamps it at creation).
    let mut parts = Vec::new();
    let mut i = 0usize;
    while store.read(&format!("prims.{i}.kind")).is_some() {
      let rf = |field: &str, dflt: f64| {
        store.read(&format!("prims.{i}.{field}")).map(|c| c.as_f64()).unwrap_or(dflt)
      };
      let p_tint = store.read(&format!("prims.{i}.tint")).map(|c| c.as_int() as u32).unwrap_or(tint);
      let p_geo =
        store.read(&format!("prims.{i}.geoColor")).map(|c| c.as_int() as u32).unwrap_or(p_tint);
      let p_texture = match store.read(&format!("prims.{i}.texture")) {
        Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
        _ => None,
      };
      parts.push(VisualPart {
        tint: p_tint,
        geo_color: p_geo,
        texture: p_texture,
        part: rf("part", 0.0) as u32,
        scale: rf("scale", 1.0),
        offset: (rf("offset.x", 0.0), rf("offset.y", 0.0)),
        size: rf("size", 1.0),
        span: rf("span", 1.0),
        sprite_scale: (rf("sprite_scale.w", 1.0), rf("sprite_scale.h", 1.0)),
        sprite_anchor: (rf("sprite_anchor.x", 0.5), rf("sprite_anchor.y", 0.5)),
        anchor: (rf("anchor.x", 0.5), rf("anchor.y", 0.5)),
      });
      i += 1;
    }
    Some(VisualParts { tint, geo_color, texture, footprint, anchor, size, span, sprite_scale, sprite_anchor, packed, light, parts })
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

  /// A tile's BUILD CATEGORY (`"wall"`), authored in its `:data @define` hook
  /// (`"wall &tile.build set` — build-walls D3: the build menu populates from content,
  /// so adding a kind here is the whole registration). `None` = not buildable.
  pub fn tile_build(&self, def_id: u16) -> Option<String> {
    let node = self.tile(self.tile_name(def_id)?)?;
    let store = self.run_node_hook(node, "data", "define")?;
    match store.read("tile.build") {
      Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
      _ => None,
    }
  }

  /// Every tile's build category in `def_id` order (index 0 → def_id 1); the empty
  /// string = not buildable. The bundle table the build menu scans (build-walls D3).
  pub fn tile_builds(&self) -> Vec<String> {
    (1..=self.tile_ids.len() as u16)
      .map(|id| self.tile_build(id).unwrap_or_default())
      .collect()
  }

  /// A tile's LIGHTING + LINKED lanes (texture-generalization P0), flat stride-6 per
  /// def_id: `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]` —
  /// zeros where unauthored. `linked_w/h` = the autotile atlas's tile count; `padding` =
  /// the INTERNAL between-cell inset (units); `rotation` = the per-type tile MODE
  /// (0 plain / 1 autotile / 2 world); the shadow lanes feed the def bits + presence flags.
  pub fn tile_lighting_lanes(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.tile_ids.len() * 6);
    for name in &self.tile_ids {
      let store = self.tile(name).and_then(|n| self.run_node_hook(n, "data", "define"));
      let read = |k: &str| store.as_ref().and_then(|s| s.read(k)).map(|c| c.as_f64()).unwrap_or(0.0);
      out.extend_from_slice(&[
        read("tile.linked.w"), read("tile.linked.h"), read("tile.padding"),
        read("tile.rotation"), read("tile.cast_shadow"), read("tile.receives_shadows"),
      ]);
    }
    out
  }

  /// A tile's HEIGHT in tiles (`1 &tile.height set` — tile-lighting F2: > 0 opts the tile
  /// into the cold lighting class as receiver + caster). `None`/0 = flat ground.
  pub fn tile_height(&self, def_id: u16) -> Option<f64> {
    let node = self.tile(self.tile_name(def_id)?)?;
    let store = self.run_node_hook(node, "data", "define")?;
    Some(store.read("tile.height")?.as_f64())
  }

  /// Every tile's height in `def_id` order (0 = flat/unauthored) — the lighting
  /// participation gate's bundle table (tile-lighting F2).
  pub fn tile_heights(&self) -> Vec<f64> {
    (1..=self.tile_ids.len() as u16)
      .map(|id| self.tile_height(id).unwrap_or(0.0))
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

  /// Every thing's spatial LAYOUT in `object_id` order, flattened **stride-10** per def
  /// (index 0 → object_id 1): `[footprint.w, footprint.h, anchor.x, anchor.y, size,
  /// sprite_anchor.x, sprite_anchor.y, span, sprite_scale.w, sprite_scale.h]` — all in
  /// the units of [`VisualParts`] (footprint + size + span in tiles, anchors in `0..1`,
  /// scales unitless). A def that builds no prim gets the default row
  /// `[1, 1, 0.5, 0.5, 1, 0.5, 0.5, 1, 1, 1]`, so the host never special-cases "unset".
  /// The host (`WorldBridge`/`MoverLayer`) resolves it into a world-px box + z-row.
  pub fn thing_layout(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.thing_ids.len() * 10);
    for name in &self.thing_ids {
      match self.thing(name).and_then(|n| self.node_visual(n)) {
        Some(v) => out.extend_from_slice(&[
          v.footprint.0, v.footprint.1, v.anchor.0, v.anchor.1, v.size, v.sprite_anchor.0, v.sprite_anchor.1,
          v.span, v.sprite_scale.0, v.sprite_scale.1,
        ]),
        None => out.extend_from_slice(&[1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0]),
      }
    }
    out
  }

  /// Every thing's emitted LIGHT in `object_id` order, flattened **stride-8** per def
  /// (index 0 → object_id 1): `[r, g, b, intensity, reach, radius, height, flags]`, where
  /// `flags` is bit 0 = `cast_shadows`, bit 1 = `hot`, bit 2 = `flicker`. Colour is `0..1`; `reach`, `radius`
  /// and `height` are TILES. A kind that emits nothing gets an all-zero row, and
  /// **`reach == 0` IS the "no light" test** the host uses — so a caller never has to know
  /// which kinds were authored with a light. Sibling of [`thing_layout`]; per KIND, so a
  /// torch's light costs nothing per placed instance (work `2026-07-25-primitive-graph`).
  pub fn thing_light(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.thing_ids.len() * 8);
    for name in &self.thing_ids {
      match self.thing(name).and_then(|n| self.node_visual(n)).and_then(|v| v.light) {
        Some(l) => out.extend_from_slice(&[
          l.color.0, l.color.1, l.color.2, l.intensity, l.reach, l.radius, l.height,
          f64::from(u8::from(l.cast) | (u8::from(l.hot) << 1) | (u8::from(l.flicker) << 2)),
        ]),
        None => out.extend_from_slice(&[0.0; 8]),
      }
    }
    out
  }

  /// A thing's movement speed in **tics per tile**, authored in its `:data @define` hook
  /// (`12 &thing.speed set` — pawn-movement F1: speed is content, measured in tics; the
  /// wall-time unit is retired). `None` when the kind authors none — the caller resolves
  /// the default (`codec::speed`); the corpus never invents one.
  pub fn thing_speed(&self, object_id: u16) -> Option<u16> {
    let node = self.thing(self.thing_name(object_id)?)?;
    let store = self.run_node_hook(node, "data", "define")?;
    Some(store.read("thing.speed")?.as_int() as u16)
  }

  /// Every thing's speed in `object_id` order (index 0 → object_id 1), `0` = unauthored
  /// (the client resolves the default — `0` is never a real speed, a hop can't take no
  /// tics). Sibling of [`thing_layout`] / [`thing_light`]: the bundle table the wasm
  /// boundary ships so speculation walks at the kind's authored rate.
  pub fn thing_speeds(&self) -> Vec<f64> {
    (1..=self.thing_ids.len() as u16)
      .map(|id| f64::from(self.thing_speed(id).unwrap_or(0)))
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

  /// A biome's stable `subtype_id` — the value its `@subtype>` hook returns. A
  /// `biome-tile`/`biome-thing` object carries its biome in `subtype` (see
  /// `docs/components/shared/codec/design/object-model.md`), so this is the biome's identity in an
  /// `object_type_reference`. It is authored EXPLICITLY per biome (a small
  /// constant) rather than derived from file order, because this file's order is
  /// evaluation PRIORITY (retunable) and a stored zone's `subtype_id` must never
  /// renumber. `None` if the biome or its `@subtype` hook is absent, or it returns
  /// `0` (the reserved/`default` subtype).
  pub fn biome_subtype_id(&self, name: &str) -> Option<u16> {
    let h = self.biome(name)?.hook("subtype")?;
    let mut store = Store::default();
    let id = run(&h.body, &mut store).unwrap_or(0);
    (id > 0).then_some(id as u16)
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

  // ---------- materials ----------

  /// Every tile's 4 packed-channel material bindings in `def_id` order (index 0 →
  /// def_id 1) — the per-def table the client fetches once and indexes by the
  /// `def_id` its prim expansion emits, exactly like [`tile_texture_stems`]. A def
  /// with no packed channels yields four default (empty) channels.
  pub fn tile_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self
      .tile_ids
      .iter()
      .map(|name| self.tile(name).and_then(|n| self.node_visual(n)).map(|v| v.packed).unwrap_or_default())
      .collect()
  }

  /// Every thing's 4 packed-channel material bindings in `object_id` order — the
  /// thing-layer sibling of [`tile_packed_channels`].
  pub fn thing_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self
      .thing_ids
      .iter()
      .map(|name| self.thing(name).and_then(|n| self.node_visual(n)).map(|v| v.packed).unwrap_or_default())
      .collect()
  }

  /// Every material name in `material_id` order (index 0 → id 1).
  pub fn material_names(&self) -> &[String] {
    &self.material_ids
  }
  /// The material def `name` (its `@on_create` hook), or `None`.
  pub fn material(&self, name: &str) -> Option<&Node> {
    self.materials.get(name)
  }
  /// The 1-based `material_id` for a name (`None` if unknown; the client treats a
  /// missing binding as `0` = no material).
  pub fn material_id(&self, name: &str) -> Option<u16> {
    self.material_ids.iter().position(|n| n == name).map(|i| i as u16 + 1)
  }
  /// The material name for a `material_id` (`0` is the empty sentinel).
  pub fn material_name(&self, id: u16) -> Option<&str> {
    (id != 0).then(|| self.material_ids.get(id as usize - 1)).flatten().map(String::as_str)
  }

  /// A material's [`MaterialParams`] — run its `@on_create` hook (materials sit
  /// directly under the `::def`, no facet, like biomes) into a fresh store and read
  /// the fields. `None` if the material or its hook is absent; unset fields fall to
  /// [`MaterialParams::default`] (identity).
  pub fn material_params(&self, name: &str) -> Option<MaterialParams> {
    let h = self.material(name)?.hook("on_create")?;
    let mut store = Store::default();
    let _ = run(&h.body, &mut store);
    let def = MaterialParams::default();
    Some(MaterialParams {
      noise_field: read_sym(&store, "noiseField").unwrap_or(def.noise_field),
      hue_swing: store.read("hueSwing").map(|c| c.as_f64()).unwrap_or(def.hue_swing),
      chroma_swing: store.read("chromaSwing").map(|c| c.as_f64()).unwrap_or(def.chroma_swing),
      warm_cool_bias: store.read("warmCoolBias").map(|c| c.as_f64()).unwrap_or(def.warm_cool_bias),
      sample_space: read_sym(&store, "sampleSpace").unwrap_or(def.sample_space),
      detail_field: read_sym(&store, "detailField").unwrap_or(def.detail_field),
      detail_amp: store.read("detailAmp").map(|c| c.as_f64()).unwrap_or(def.detail_amp),
      detail_scale: store.read("detailScale").map(|c| c.as_f64()).unwrap_or(def.detail_scale),
    })
  }

  /// The whole material registry in `material_id` order — the client fetches this
  /// once and indexes it by the `material_id` a packed channel carries.
  pub fn material_params_all(&self) -> Vec<MaterialParams> {
    self.material_ids.iter().map(|n| self.material_params(n).unwrap_or_default()).collect()
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
        index_defs(&node, "material", &mut bundle.materials, &mut bundle.material_ids);
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
  fn the_real_repo_corpus_loads_and_the_humans_declare_their_parts() {
    // A REAL-corpus smoke test (human-pawns P2): loads `content/` from the repo checkout
    // when present (docker mounts the workspace; a packaged build without it skips). This
    // is the check that would have caught the stale manifests — the corpus must parse and
    // the human kinds must resolve their skeletons.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    if !root.exists() {
      return;
    }
    let sources = crate::content::read_content_dir(&root).expect("read content/");
    let b = load(&sources).expect("the repo corpus loads clean");
    for name in ["human_female", "human_male"] {
      let id = b.thing_object_id(name).expect(name);
      let v = b.visual_for_object(id).expect("visual");
      assert_eq!(v.parts.len(), 2, "{name}: body + head");
      assert_eq!(v.parts[1].part, 1, "{name}: head draws part-1 files");
      // pawn-part-placement F4: `scale` is the PRE-ATLAS art scale, so the user's two numbers
      // are absolute — body 0.8 tiles of art, head 0.5, both in `span 1` frames.
      assert_eq!(v.parts[0].scale, 0.8, "{name}: body scale (user spec)");
      assert_eq!(v.parts[1].scale, 0.5, "{name}: head scale (user spec)");
    }
    // the wolf stays the 1-part degenerate case
    let wolf = b.visual_for_object(b.thing_object_id("wolf").unwrap()).unwrap();
    assert_eq!(wolf.parts.len(), 1);
  }

  #[test]
  fn a_two_prim_visual_yields_a_parts_list() {
    // human-pawns P2: the human shape — slot 0 body + slot 1 head (`part 1`, `scale
    // 0.625`, an upward tile offset). Mirrors content/visual/pawns.rd.
    let data = "<thing>\n  ::human_female>\n    :data>\n      @define>\n        16 &thing.speed set\n        0 return\n";
    let visual = "\
<thing>
  ::human_female>
    :visual>
      @on_create>
        \"thing ^prim call &body export
        \"pawn/human/female &body.texture set
        #ffffff &body.tint set
        #7a6a5a &body.geoColor set
        1.5 &body.size set
        2 &body.span set
        1.0 &body.anchor.y set
        1.0 &body.sprite_anchor.y set
        \"thing ^prim call &head export
        \"pawn/human/female &head.texture set
        1 &head.part set
        0.625 &head.scale set
        -1.15 &head.offset.y set
        2 &head.span set
        0 return
";
    let b = load(&[src("data/things.rd", data), src("visual/pawns.rd", visual)]).expect("load");
    let v = b.visual_for_object(1).unwrap();
    assert_eq!(v.parts.len(), 2, "two ^prim calls → two part slots");
    let body = &v.parts[0];
    assert_eq!(body.part, 0);
    assert_eq!(body.scale, 1.0);
    assert_eq!(body.texture.as_deref(), Some("pawn/human/female"));
    assert_eq!((body.size, body.span), (1.5, 2.0));
    assert_eq!(body.anchor.1, 1.0);
    let head = &v.parts[1];
    assert_eq!(head.part, 1);
    assert_eq!(head.scale, 0.625);
    assert_eq!(head.offset, (0.0, -1.15));
    assert_eq!(head.texture.as_deref(), Some("pawn/human/female"));
    // a 1-prim kind (the wolf shape) still yields exactly one slot
    let one = "<thing>\n  ::wolfish>\n    :data>\n      @define>\n        0 return\n";
    let onev = "<thing>\n  ::wolfish>\n    :visual>\n      @on_create>\n        \"thing ^prim call &thing export\n        \"pawn/animal/wolf &thing.texture set\n        #ffffff &thing.tint set\n        0 return\n";
    let b1 = load(&[src("data/things.rd", one), src("visual/things.rd", onev)]).expect("load");
    let w = b1.visual_for_object(1).unwrap();
    assert_eq!(w.parts.len(), 1);
    assert_eq!(w.parts[0].part, 0);
  }

  #[test]
  fn thing_stem_and_layout_tables() {
    let data = "<thing>\n  ::tree>\n    :data>\n      @define>\n        0 return\n  ::shrub>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<thing>
  ::tree>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        \"world/conifer &thing.texture set
        #ffffff &thing.tint set
        3.0 &thing.size set
        2.0 &thing.span set
        0.75 &thing.sprite_scale.w set
        1.0 &thing.anchor.y set
        1.0 &thing.sprite_anchor.y set
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
    assert_eq!(b.thing_texture_stems(), vec!["world/conifer".to_string(), "white".to_string()]);
    // tree: default 1×1 footprint, bottom anchor (y=1) + bottom sprite pivot (y=1),
    // size 3 tiles, span 2, sprite_scale (0.75, 1). shrub authors none → the all-default row.
    assert_eq!(
      b.thing_layout(),
      vec![
        1.0, 1.0, 0.5, 1.0, 3.0, 0.5, 1.0, 2.0, 0.75, 1.0, // tree
        1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0, // shrub (defaults)
      ],
    );
  }

  #[test]
  fn thing_speed_from_data_define() {
    // wolf authors 12 tics/tile in its `:data @define`; tree authors none → `None`
    // (the CALLER resolves the default — the corpus never invents a speed).
    let data = "<thing>\n  ::tree>\n    :data>\n      @define>\n        0 return\n  ::wolf>\n    :data>\n      @define>\n        12 &thing.speed set\n        0 return\n";
    let b = load(&[src("data/things.rd", data)]).expect("load");
    assert_eq!(b.thing_speed(2), Some(12));
    assert_eq!(b.thing_speed(1), None);
    assert_eq!(b.thing_speeds(), vec![0.0, 12.0]); // bundle table: 0 = unauthored
  }

  #[test]
  fn material_registry_and_packed_channels() {
    // A material registry (mottle=1) + a stone tile binding its packed.0 channel to
    // it; grass binds no material (identity default).
    let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n  ::stone>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<tile>
  ::grass>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        #4b573e &tile.tint set
        0 return
  ::stone>
    :visual>
      @on_create>
        \"tile ^prim call &tile export
        \"linked/wall_smooth &tile.texture set
        #6b6b6b &tile.tint set
        \"mottle &tile.packed.0.material set
        #6b6b6b &tile.packed.0.tint set
        0 return
";
    let material = "\
<material>
  ::mottle>
    @on_create>
      mottle &noiseField set
      10 &hueSwing set
      0.03 &chromaSwing set
      0.2 &warmCoolBias set
      world &sampleSpace set
      0 return
";
    let b = load(&[
      src("data/tiles.rd", data),
      src("visual/tiles.rd", visual),
      src("material/materials.rd", material),
    ])
    .expect("clean load");

    // registry: mottle is the first (and only) material → id 1.
    assert_eq!(b.material_id("mottle"), Some(1));
    assert_eq!(b.material_name(1), Some("mottle"));
    assert_eq!(b.material_id("nope"), None);
    let mp = b.material_params("mottle").unwrap();
    assert_eq!(mp.noise_field, "mottle");
    assert_eq!(mp.hue_swing, 10.0);
    assert_eq!(mp.chroma_swing, 0.03);
    assert_eq!(mp.warm_cool_bias, 0.2);
    assert_eq!(mp.sample_space, "world");
    assert_eq!(b.material_params_all().len(), 1);

    // stone (def_id 2) binds packed.0 → material 1, tint 0x6b6b6b; other channels
    // stay empty. grass (def_id 1) binds nothing → all-default.
    let stone = b.visual_for_def(2).unwrap();
    assert_eq!(stone.packed[0], PackedChannel { material_id: 1, tint: 0x6b6b6b });
    assert_eq!(stone.packed[1], PackedChannel::default());
    let grass = b.visual_for_def(1).unwrap();
    assert_eq!(grass.packed, [PackedChannel::default(); 4]);

    // the per-def table the client fetches (def_id order).
    let table = b.tile_packed_channels();
    assert_eq!(table[0], [PackedChannel::default(); 4]);
    assert_eq!(table[1][0], PackedChannel { material_id: 1, tint: 0x6b6b6b });
  }

  #[test]
  fn unset_material_params_default_to_identity() {
    // A material that sets nothing (or is absent) yields identity params — zero
    // swings, uv space — so binding it changes nothing until it's authored.
    let material = "<material>\n  ::blank>\n    @on_create>\n      0 return\n";
    let b = load(&[src("material/materials.rd", material)]).expect("load");
    let mp = b.material_params("blank").unwrap();
    assert_eq!(mp, MaterialParams::default());
    assert_eq!(mp.sample_space, "uv");
    assert_eq!(mp.hue_swing, 0.0);
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
  fn biome_carries_its_stable_subtype_id() {
    let biome = "\
<biome>
  ::forest>
    @subtype>
      6 return
    @define>
      1 return
    @on_create>
      grass &tile set
      0 return
  ::plains>
    @define>
      1 return
";
    let b = load(&[src("biome/biomes.rd", biome)]).expect("load");
    // forest's @subtype returns its explicit, stable id — its biome identity in
    // an object_type_reference, independent of file order.
    assert_eq!(b.biome_subtype_id("forest"), Some(6));
    // plains authors no @subtype → None (the reserved/`default` subtype 0).
    assert_eq!(b.biome_subtype_id("plains"), None);
    // an unknown biome → None.
    assert_eq!(b.biome_subtype_id("nope"), None);
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

  #[test]
  fn thing_light_is_authored_per_kind() {
    // A kind that emits light (`&thing.light.*`) vs one that does not. `reach` is the
    // discriminator: omit it and the kind stays dark, so every pre-existing def is untouched.
    // NOTE: a visual hook MUST set `tint` — `node_visual` bails (`?`) without it and the whole
    // VisualParts comes back None, so every attr silently reads its default. And the embedded DSL
    // is INDENTATION-SENSITIVE, must start at column 0 — indenting
    // it to match the Rust around it silently pushes the hook body to a structural level and
    // the instructions never run (they parse, they just attribute nothing).
    let data = "\
<thing>
  ::lamp>
    :data>
      @define>
        0 return
  ::rock>
    :data>
      @define>
        0 return
";
    let visual = "\
<thing>
  ::lamp>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        #ffffff &thing.tint set
        2 &thing.size set
        0.45 &thing.light.r set
        0.95 &thing.light.g set
        0.70 &thing.light.b set
        0.8 &thing.light.intensity set
        4 &thing.light.reach set
        0.3 &thing.light.radius set
        0.4 &thing.light.height set
        0 &thing.light.cast set
        0 return
  ::rock>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        #ffffff &thing.tint set
        0 return
";
    let b = load(&[src("data/things.rd", data), src("visual/things.rd", visual)]).expect("clean load");
    // A known-good attr through the same harness first, so a failure below is the light path
    // and not the fixture.
    assert_eq!(
      (b.thing_texture_stems().len(), b.thing_layout().len()),
      (2, 20), "fixture: 2 kinds registered"
    );
    assert_eq!(b.thing_layout()[4], 2.0, "`size` must parse (stride-10, index 4)");
    let t = b.thing_light();
    assert_eq!(t.len(), 16, "stride-8 per kind, 2 kinds");
    assert_eq!(&t[0..7], &[0.45, 0.95, 0.70, 0.8, 4.0, 0.3, 0.4]);
    assert_eq!(t[7], 0.0, "cast 0 + hot unset => flags 0");
    // rock emits nothing → an all-zero row; `reach == 0` is the host's "no light" test.
    assert_eq!(&t[8..16], &[0.0; 8]);
  }

}
