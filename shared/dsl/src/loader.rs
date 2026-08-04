//! Content loader — parse content sources into one ready-to-query, fully
//! MATERIALIZED [`Bundle`].
//!
//! Two source dialects, ONE bundle (toml-content F2):
//!   - **TOML** (`content/*.toml`) — the go-forward corpus: pure data, explicit ids
//!     (F1), biome rules as declarative tables (F3). Parsed by [`crate::toml_loader`].
//!   - **`.rd`** (the retiring DSL) — parsed + hook-EVALUATED **once, at load**, into
//!     the same materialized tables. Migration-scoped: dies with the golden gate
//!     (`docs/work/2026-08-04-toml-content`).
//!
//! Consumers never see the dialect: every accessor reads precomputed data, and the
//! server links this as an rlib while the client reaches it through the
//! `resonantdust-shared` wasm bundle — same content, same ids, both sides.

use crate::parser::{Header, Node, Stmt};
use crate::vm::{run, Store};
use std::collections::HashMap;

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

/// Readable aliases for the first four rotation indices (`FACING_BY_ROTATION` order).
/// Index 3 (`w`) is the east master mirrored and should carry no art of its own.
const ROTATION_ALIASES: [&str; 4] = ["s", "e", "n", "w"];

/// Band slots per need / need slots per kind — the authoring caps (needs-moodlets P1).
pub const NEED_BANDS: usize = 4;
pub const NEEDS_PER_KIND: usize = 8;

/// One BAND on a need's satisfaction — the conditional moodlet active while
/// `lo <= satisfaction < hi` (needs-moodlets F2: band moodlets are DERIVED by every
/// observer from `(need row, tic, corpus)`; no grant events exist for them). Bands are
/// authored EXCLUSIVE, so at most one band moodlet per need is active.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedBand {
  /// The `<moodlet>` name this band activates (resolved via [`Bundle::moodlet_id`]).
  pub moodlet: String,
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
  /// The conditional-moodlet bands, in authored slot order.
  pub bands: Vec<NeedBand>,
}

/// A `<moodlet>` def's parameters — the DISPLAYED consequence of hidden state.
/// `duration == 0` marks a CONDITIONAL moodlet (alive exactly while its band holds);
/// `> 0` a TIMED one (a stored grant expiring `duration` tics after its `grant_tic`).
#[derive(Debug, Clone, PartialEq)]
pub struct MoodletParams {
  pub label: String,
  /// Mood offset while active, `-1..1` (mood = clamp(base + Σ offsets), F5).
  pub mood: f64,
  /// Lifetime in TICS for a stored grant; `0` = conditional (band-derived).
  pub duration: f64,
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

/// One tile def, fully evaluated. `name` may be `""` for a RETIRED id (an F1 hole).
#[derive(Debug, Default, Clone)]
pub(crate) struct TileDef {
  pub name: String,
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

/// The classifier per dialect. `Hooks` retains the `.rd` VM bodies (migration-scoped —
/// dies with the DSL); `Rules` is the declarative TOML form (F3).
#[derive(Debug, Clone)]
pub(crate) enum BiomeBody {
  Hooks { define: Vec<Stmt>, on_create: Vec<Stmt> },
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
  pub(crate) moodlets: Vec<(String, MoodletParams)>,
  /// Registry name caches (id order) — what the slice-returning accessors serve.
  tile_names: Vec<String>,
  thing_names: Vec<String>,
  biome_names: Vec<String>,
  material_names: Vec<String>,
  need_names: Vec<String>,
  moodlet_names: Vec<String>,
}

impl Bundle {
  /// Finalize the name caches after the def vecs are filled (both loaders call this).
  pub(crate) fn index(mut self) -> Self {
    self.tile_names = self.tiles.iter().map(|d| d.name.clone()).collect();
    self.thing_names = self.things.iter().map(|d| d.name.clone()).collect();
    self.biome_names = self.biomes.iter().map(|d| d.name.clone()).collect();
    self.material_names = self.materials.iter().map(|(n, _)| n.clone()).collect();
    self.need_names = self.needs.iter().map(|(n, _)| n.clone()).collect();
    self.moodlet_names = self.moodlets.iter().map(|(n, _)| n.clone()).collect();
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
    Self::id_of(&self.tile_names, name)
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
    Self::id_of(&self.thing_names, name)
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
  pub fn select_biome(&self, dims: &[f64], seed: u64) -> Option<&str> {
    self.biomes.iter().find_map(|b| self.biome_matches(b, dims, seed).then_some(b.name.as_str()))
  }

  fn biome_matches(&self, b: &BiomeDef, dims: &[f64], seed: u64) -> bool {
    match &b.body {
      BiomeBody::Hooks { define, .. } => {
        let mut store = Store::default();
        store.set_biome(dims.to_vec());
        store.set_seed(seed);
        run(define, &mut store).unwrap_or(0) != 0
      }
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
    let Some(b) = self.biomes.iter().find(|b| self.biome_matches(b, dims, seed)) else {
      return GenTile::default();
    };
    let mut gen = GenTile { biome: Some(b.name.clone()), ..GenTile::default() };
    match &b.body {
      BiomeBody::Hooks { on_create, .. } => {
        let mut store = Store::default();
        store.set_biome(dims.to_vec());
        store.set_seed(seed);
        let _ = run(on_create, &mut store);
        gen.tile = read_sym(&store, "tile");
        gen.thing1 = read_sym(&store, "thing.1");
      }
      BiomeBody::Rules(r) => {
        gen.tile = r.tile.clone();
        // Ordered draws, LAST hit wins — the `.rd` overwrite semantics exactly.
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

  // ---------- needs & moodlets (needs-moodlets P1) ----------

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

  /// Every moodlet name in `moodlet_id` order (index 0 → id 1).
  pub fn moodlet_names(&self) -> &[String] {
    &self.moodlet_names
  }
  /// The 1-based `moodlet_id` for a name (`None` if unknown).
  pub fn moodlet_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.moodlet_names, name)
  }
  /// The moodlet name for a `moodlet_id` (`0` is the empty sentinel).
  pub fn moodlet_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.moodlet_names, id)
  }
  /// A moodlet's [`MoodletParams`], or `None` if unknown.
  pub fn moodlet_params(&self, name: &str) -> Option<MoodletParams> {
    self.moodlets.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole moodlet registry in `moodlet_id` order.
  pub fn moodlet_params_all(&self) -> Vec<MoodletParams> {
    self.moodlets.iter().map(|(_, p)| p.clone()).collect()
  }
}

/// Read a slot as a content name: a `Sym` yields the string; anything else `None`.
fn read_sym(store: &Store, path: &str) -> Option<String> {
  match store.read(path) {
    Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
    _ => None,
  }
}

/// Parse every `(name, source)` into a [`Bundle`]. Dialect dispatch (toml-content):
/// any `.toml` source routes the WHOLE load through the TOML loader (mixing dialects
/// is an error — a cutover is a cutover); otherwise the `.rd` path evaluates hooks
/// once and materializes.
pub fn load(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  let toml_count = sources.iter().filter(|(n, _)| n.ends_with(".toml")).count();
  if toml_count > 0 {
    if toml_count != sources.len() {
      return Err(vec![LoadError {
        file: String::new(),
        message: "mixed corpus: .rd and .toml sources cannot load together (finish the cutover)".into(),
      }]);
    }
    return crate::toml_loader::load_toml(sources);
  }
  rd::load_rd(sources)
}

/// The `.rd` materializer — parse, index defs (merging facets across files), then
/// EVALUATE every hook exactly once into the data the accessors serve. Migration-scoped
/// (toml-content F5): this module + parser + vm die when the golden gate passes.
mod rd {
  use super::*;

  pub(super) fn load_rd(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
    let mut errors = Vec::new();
    let mut tiles: HashMap<String, Node> = HashMap::new();
    let mut tile_ids = Vec::new();
    let mut things: HashMap<String, Node> = HashMap::new();
    let mut thing_ids = Vec::new();
    let mut biomes: HashMap<String, Node> = HashMap::new();
    let mut biome_ids = Vec::new();
    let mut materials: HashMap<String, Node> = HashMap::new();
    let mut material_ids = Vec::new();
    let mut needs: HashMap<String, Node> = HashMap::new();
    let mut need_ids = Vec::new();
    let mut moodlets: HashMap<String, Node> = HashMap::new();
    let mut moodlet_ids = Vec::new();

    for (name, text) in sources {
      match crate::parser::parse(text) {
        Ok(node) => {
          index_defs(&node, "tile", &mut tiles, &mut tile_ids);
          index_defs(&node, "thing", &mut things, &mut thing_ids);
          index_defs(&node, "biome", &mut biomes, &mut biome_ids);
          index_defs(&node, "material", &mut materials, &mut material_ids);
          index_defs(&node, "need", &mut needs, &mut need_ids);
          index_defs(&node, "moodlet", &mut moodlets, &mut moodlet_ids);
        }
        Err(e) => errors.push(LoadError { file: name.clone(), message: format!("parse: {e}") }),
      }
    }
    if !errors.is_empty() {
      return Err(errors);
    }

    let mut b = Bundle::default();

    // Materials FIRST (visuals resolve packed-channel material names against them),
    // needs before things (`&thing.needs` resolves to need ids).
    for n in &material_ids {
      b.materials.push((n.clone(), material_params(&materials[n]).unwrap_or_default()));
    }
    for n in &need_ids {
      b.needs.push((n.clone(), need_params_of(&needs[n], n)));
    }
    for n in &moodlet_ids {
      b.moodlets.push((n.clone(), moodlet_params_of(&moodlets[n], n)));
    }

    let material_id = |name: &str| -> u16 {
      material_ids.iter().position(|n| n == name).map(|i| i as u16 + 1).unwrap_or(0)
    };
    let need_id = |name: &str| -> Option<u16> {
      need_ids.iter().position(|n| n == name).map(|i| i as u16 + 1)
    };

    for n in &tile_ids {
      let node = &tiles[n];
      let data = run_node_hook(node, "data", "define");
      let read = |k: &str| data.as_ref().and_then(|s| s.read(k)).map(|c| c.as_f64()).unwrap_or(0.0);
      b.tiles.push(TileDef {
        name: n.clone(),
        color: node_color_bg(node),
        visual: node_visual(node, &material_id),
        build: data.as_ref().and_then(|s| read_sym(s, "tile.build")),
        height: data.as_ref().and_then(|s| s.read("tile.height")).map(|c| c.as_f64()),
        lanes: [
          read("tile.linked.w"), read("tile.linked.h"), read("tile.padding"),
          read("tile.rotation"), read("tile.cast_shadow"), read("tile.receives_shadows"),
        ],
      });
    }

    for n in &thing_ids {
      let node = &things[n];
      let data = run_node_hook(node, "data", "define");
      let speed = data.as_ref().and_then(|s| s.read("thing.speed")).map(|c| c.as_int() as u16);
      let mut kind_needs = Vec::new();
      if let Some(s) = &data {
        for i in 0..NEEDS_PER_KIND {
          if let Some(name) = read_sym(s, &format!("thing.needs.{i}")) {
            // A typo'd need DROPS (no warn channel) — surfaces as never draining.
            if let Some(id) = need_id(&name) {
              kind_needs.push(id);
            }
          }
        }
      }
      b.things.push(ThingDef {
        name: n.clone(),
        color: node_color_bg(node),
        visual: node_visual(node, &material_id),
        speed,
        needs: kind_needs,
      });
    }

    for n in &biome_ids {
      let node = &biomes[n];
      let subtype = node
        .hook("subtype")
        .map(|h| {
          let mut store = Store::default();
          run(&h.body, &mut store).unwrap_or(0).max(0) as u16
        })
        .unwrap_or(0);
      b.biomes.push(BiomeDef {
        name: n.clone(),
        subtype,
        body: BiomeBody::Hooks {
          define: node.hook("define").map(|h| h.body.clone()).unwrap_or_default(),
          on_create: node.hook("on_create").map(|h| h.body.clone()).unwrap_or_default(),
        },
      });
    }

    Ok(b.index())
  }

  /// Index every `<{bucket}>` def by name, recording first-appearance order. A def
  /// seen again (its other facet, another file) folds its facets onto the existing
  /// node. Append-only: ids stay stable across content edits.
  fn index_defs(node: &Node, bucket_name: &str, defs: &mut HashMap<String, Node>, ids: &mut Vec<String>) {
    for bucket in &node.children {
      if bucket.header != Header::Bucket(bucket_name.into()) {
        continue;
      }
      for d in &bucket.children {
        if let Header::Def(id) = &d.header {
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

  fn run_node_hook(node: &Node, facet: &str, hook: &str) -> Option<Store> {
    let h = node.facet(facet)?.hook(hook)?;
    let mut store = Store::default();
    let _ = run(&h.body, &mut store);
    Some(store)
  }

  /// A def's background colour: the `:visual @on_create` prim tint, falling back to a
  /// static `color.bg` / `visual.color.bg` in an `@export` / `@define` hook.
  fn node_color_bg(node: &Node) -> Option<u32> {
    if let Some(store) = run_node_hook(node, "visual", "on_create") {
      if let Some(c) = store.read("prims.0.tint") {
        return Some(c.as_int() as u32);
      }
    }
    for hook in ["export", "define"] {
      if let Some(store) = run_node_hook(node, "visual", hook) {
        let c = store.read("color.bg").or_else(|| store.read("visual.color.bg"));
        if let Some(c) = c {
          return Some(c.as_int() as u32);
        }
      }
    }
    None
  }

  /// A def's [`VisualParts`] — one `:visual @on_create` run, every field read out.
  fn node_visual(node: &Node, material_id: &dyn Fn(&str) -> u16) -> Option<VisualParts> {
    let store = run_node_hook(node, "visual", "on_create")?;
    let tint = store.read("prims.0.tint")?.as_int() as u32;
    let geo_color = store.read("prims.0.geoColor").map(|c| c.as_int() as u32).unwrap_or(tint);
    let texture = match store.read("prims.0.texture") {
      Some(crate::vm::Cell::Sym(s)) => Some(s.clone()),
      _ => None,
    };
    let read_f = |path: &str, dflt: f64| store.read(path).map(|c| c.as_f64()).unwrap_or(dflt);
    let footprint = (read_f("prims.0.footprint.w", 1.0), read_f("prims.0.footprint.h", 1.0));
    let anchor = (read_f("prims.0.anchor.x", 0.5), read_f("prims.0.anchor.y", 0.5));
    let size = read_f("prims.0.size", 1.0);
    let span = read_f("prims.0.span", 1.0);
    let sprite_scale = (read_f("prims.0.sprite_scale.w", 1.0), read_f("prims.0.sprite_scale.h", 1.0));
    let sprite_anchor = (read_f("prims.0.sprite_anchor.x", 0.5), read_f("prims.0.sprite_anchor.y", 0.5));
    // The two-axis fallback chain, most-specific first, PER COMPONENT (subframe-ingest I10):
    //   v3.r1 → v3.e → v3 → r1 → e → base
    let pick = |field: &str, comp: &str, v: usize, r: usize, dflt: f64| -> f64 {
      let alias = ROTATION_ALIASES.get(r);
      let mut keys: Vec<String> = vec![format!("prims.0.{field}.v{v}.r{r}.{comp}")];
      if let Some(a) = alias {
        keys.push(format!("prims.0.{field}.v{v}.{a}.{comp}"));
      }
      keys.push(format!("prims.0.{field}.v{v}.{comp}"));
      keys.push(format!("prims.0.{field}.r{r}.{comp}"));
      if let Some(a) = alias {
        keys.push(format!("prims.0.{field}.{a}.{comp}"));
      }
      keys.push(format!("prims.0.{field}.{comp}"));
      for k in &keys {
        let got = read_f(k, f64::NAN);
        if !got.is_nan() {
          return got;
        }
      }
      dflt
    };
    // Load-time now, so the old hot-path guard is no longer about render frames — it
    // still short-circuits the ~9k key formats for the (near-universal) unauthored case.
    let authored = store.read("prims.0.subframe").is_some();
    let dir_frames = if authored {
      std::array::from_fn::<_, VARIANTS_PER_DEF, _>(|v| {
        std::array::from_fn::<DirFrame, ROTATIONS_PER_DEF, _>(|r| DirFrame {
          sub: (
            pick("subframe", "x", v, r, 0.0),
            pick("subframe", "y", v, r, 0.0),
            pick("subframe", "w", v, r, 1.0),
            pick("subframe", "h", v, r, 1.0),
          ),
          anchor: (
            pick("sprite_anchor", "x", v, r, 0.5),
            pick("sprite_anchor", "y", v, r, 0.5),
          ),
        })
      })
    } else {
      [[DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: sprite_anchor }; ROTATIONS_PER_DEF];
       VARIANTS_PER_DEF]
    };
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
    let mut packed: [PackedChannel; 4] = Default::default();
    for (i, ch) in packed.iter_mut().enumerate() {
      let mid = match store.read(&format!("prims.0.packed.{i}.material")) {
        Some(crate::vm::Cell::Sym(name)) => material_id(name),
        _ => 0,
      };
      let ctint = store.read(&format!("prims.0.packed.{i}.tint")).map(|c| c.as_int() as u32).unwrap_or(0);
      if mid != 0 || ctint != 0 {
        *ch = PackedChannel { material_id: mid, tint: ctint };
      }
    }
    // Every part slot, in `^prim call` order.
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
        elevation: rf("offset.z", 0.0),
        depth: rf("depth", 0.0),
        size: rf("size", 1.0),
        span: rf("span", 1.0),
        sprite_scale: (rf("sprite_scale.w", 1.0), rf("sprite_scale.h", 1.0)),
        sprite_anchor: (rf("sprite_anchor.x", 0.5), rf("sprite_anchor.y", 0.5)),
        anchor: (rf("anchor.x", 0.5), rf("anchor.y", 0.5)),
        dir_frames: if store.read(&format!("prims.{i}.subframe")).is_none() {
          [[DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: (rf("sprite_anchor.x", 0.5), rf("sprite_anchor.y", 0.5)) };
            ROTATIONS_PER_DEF]; VARIANTS_PER_DEF]
        } else { std::array::from_fn::<_, VARIANTS_PER_DEF, _>(|v| {
          std::array::from_fn::<DirFrame, ROTATIONS_PER_DEF, _>(|r| {
            let pick = |field: &str, comp: &str, dflt: f64| -> f64 {
              let alias = ROTATION_ALIASES.get(r);
              let mut keys: Vec<String> = vec![format!("{field}.v{v}.r{r}.{comp}")];
              if let Some(a) = alias { keys.push(format!("{field}.v{v}.{a}.{comp}")); }
              keys.push(format!("{field}.v{v}.{comp}"));
              keys.push(format!("{field}.r{r}.{comp}"));
              if let Some(a) = alias { keys.push(format!("{field}.{a}.{comp}")); }
              keys.push(format!("{field}.{comp}"));
              for k in &keys {
                let got = rf(k, f64::NAN);
                if !got.is_nan() { return got; }
              }
              dflt
            };
            DirFrame {
              sub: (pick("subframe", "x", 0.0), pick("subframe", "y", 0.0),
                    pick("subframe", "w", 1.0), pick("subframe", "h", 1.0)),
              anchor: (pick("sprite_anchor", "x", 0.5), pick("sprite_anchor", "y", 0.5)),
            }
          })
        }) },
      });
      i += 1;
    }
    Some(VisualParts { tint, geo_color, texture, footprint, anchor, size, span, sprite_scale, sprite_anchor, dir_frames, packed, light, parts })
  }

  /// A material's params — its direct `@on_create` (no facet, like biomes).
  fn material_params(node: &Node) -> Option<MaterialParams> {
    let h = node.hook("on_create")?;
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

  /// A need's params — its direct `@define`'s `&need.*` reads; band slots are
  /// `&need.band.<i>.*`, dense in slot order.
  fn need_params_of(node: &Node, name: &str) -> NeedParams {
    let fallback = NeedParams { label: name.to_string(), deplete: 0.0, bands: Vec::new() };
    let Some(h) = node.hook("define") else { return fallback };
    let mut store = Store::default();
    let _ = run(&h.body, &mut store);
    let mut bands = Vec::new();
    for i in 0..NEED_BANDS {
      let Some(moodlet) = read_sym(&store, &format!("need.band.{i}.moodlet")) else { continue };
      bands.push(NeedBand {
        moodlet,
        lo: store.read(&format!("need.band.{i}.lo")).map(|c| c.as_f64()).unwrap_or(0.0),
        hi: store.read(&format!("need.band.{i}.hi")).map(|c| c.as_f64()).unwrap_or(0.0),
      });
    }
    NeedParams {
      label: read_sym(&store, "need.label").unwrap_or_else(|| name.to_string()),
      deplete: store.read("need.deplete").map(|c| c.as_f64()).unwrap_or(0.0),
      bands,
    }
  }

  /// A moodlet's params — its direct `@define`'s `&moodlet.*` reads.
  fn moodlet_params_of(node: &Node, name: &str) -> MoodletParams {
    let fallback = MoodletParams { label: name.to_string(), mood: 0.0, duration: 0.0 };
    let Some(h) = node.hook("define") else { return fallback };
    let mut store = Store::default();
    let _ = run(&h.body, &mut store);
    MoodletParams {
      label: read_sym(&store, "moodlet.label").unwrap_or_else(|| name.to_string()),
      mood: store.read("moodlet.mood").map(|c| c.as_f64()).unwrap_or(0.0),
      duration: store.read("moodlet.duration").map(|c| c.as_f64()).unwrap_or(0.0),
    }
  }
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

  fn src(name: &str, text: &str) -> (String, String) {
    (name.to_string(), text.to_string())
  }

  /// The two facets of grass/dirt, authored the way content/ splits them.
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
    assert_eq!(b.tile_def_id("grass"), Some(1));
    assert_eq!(b.tile_def_id("dirt"), Some(2));
    assert_eq!(b.tile_name(1), Some("grass"));
    assert_eq!(b.tile_name(2), Some("dirt"));
    assert_eq!(b.tile_name(0), None);
    assert_eq!(b.tile_def_id("stone"), None);
  }

  #[test]
  fn merges_data_and_visual_facets() {
    // Both facets land on ONE def: grass appears once in the registry, and its
    // visual-facet colour resolves — the merge's observable contract.
    let b = load(&corpus()).expect("clean load");
    assert_eq!(b.tile_names().iter().filter(|n| *n == "grass").count(), 1);
    assert_eq!(b.tile_color_bg("grass"), Some(0x4b573e));
  }

  #[test]
  fn extracts_visual_color_bg_fallback_static_shape() {
    let b = load(&corpus()).expect("clean load");
    assert_eq!(b.tile_color_bg("grass"), Some(0x4b573e));
    assert_eq!(b.tile_color_bg("dirt"), Some(0x7d6144));
    assert_eq!(b.color_bg_for_def(1), Some(0x4b573e));
    assert_eq!(b.color_bg_for_def(2), Some(0x7d6144));
  }

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
    let grass = b.visual_for_def(1).unwrap();
    assert_eq!(grass.tint, 0x4b573e);
    assert_eq!(grass.geo_color, 0x4b573e);
    assert_eq!(grass.texture.as_deref(), Some("white"));
    let stone = b.visual_for_def(2).unwrap();
    assert_eq!(stone.tint, 0xffffff);
    assert_eq!(stone.geo_color, 0x6b6b6b);
    assert_eq!(stone.texture.as_deref(), Some("linked/wall_smooth"));
    assert_eq!(b.tile_texture_stems(), vec!["white".to_string(), "linked/wall_smooth".to_string()]);
  }

  #[test]
  fn the_real_repo_corpus_loads_and_the_humans_declare_their_parts() {
    // A REAL-corpus smoke test (human-pawns P2): loads `content/` from the repo checkout
    // when present (docker mounts the workspace; a packaged build without it skips).
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
      assert_eq!(v.parts[0].scale, 0.8, "{name}: body scale (user spec)");
      assert_eq!(v.parts[1].scale, 0.5, "{name}: head scale (user spec)");
    }
    // the wolf stays the 1-part degenerate case
    let wolf = b.visual_for_object(b.thing_object_id("wolf").unwrap()).unwrap();
    assert_eq!(wolf.parts.len(), 1);
    // needs-moodlets P1: thirst registered + wired to the wolf, bands name real moodlets.
    let thirst = b.need_id("thirst").expect("thirst registered");
    let np = b.need_params("thirst").unwrap();
    assert!(np.deplete > 0.0, "thirst drains");
    assert_eq!(np.bands.len(), 2, "Thirsty + Dehydrated");
    assert!(np.bands.iter().all(|band| b.moodlet_id(&band.moodlet).is_some()), "bands name real moodlets");
    assert_eq!(b.thing_needs(b.thing_object_id("wolf").unwrap()), vec![thirst]);
  }

  #[test]
  fn a_two_prim_visual_yields_a_parts_list() {
    // human-pawns P2: slot 0 body + slot 1 head. Mirrors content/visual/pawns.rd.
    let data = "<thing>\n  ::human_female>\n    :data>\n      @define>\n        16 &thing.speed set\n        0 return\n";
    let visual = "\
<thing>
  ::human_female>
    :visual>
      @on_create>
        \"body ^prim call &body export
        \"pawn/human/female &body.texture set
        #ffffff &body.tint set
        0.8 &body.scale set
        \"head ^prim call &head export
        \"pawn/human/female &head.texture set
        1 &head.part set
        0.625 &head.scale set
        0.6 &head.offset.z set
        0 return
";
    let b = load(&[src("data/pawns.rd", data), src("visual/pawns.rd", visual)]).expect("load");
    let v = b.visual_for_object(1).expect("visual");
    assert_eq!(v.parts.len(), 2);
    assert_eq!(v.parts[0].texture.as_deref(), Some("pawn/human/female"));
    assert_eq!(v.parts[0].part, 0);
    assert_eq!(v.parts[0].scale, 0.8);
    assert_eq!(v.parts[1].part, 1);
    assert_eq!(v.parts[1].scale, 0.625);
    assert_eq!(v.parts[1].elevation, 0.6);
    assert_eq!(b.thing_speed(1), Some(16));
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
    assert_eq!(
      b.thing_layout(),
      vec![
        1.0, 1.0, 0.5, 1.0, 3.0, 0.5, 1.0, 2.0, 0.75, 1.0, // tree
        1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0, // shrub (defaults)
      ],
    );
  }

  #[test]
  fn subframe_variant_and_rotation_are_separate_axes() {
    // subframe-ingest I10: one kind can author BOTH axes without collisions.
    let data = "<thing>\n  ::tree>\n    :data>\n      @define>\n        0 return\n  ::shrub>\n    :data>\n      @define>\n        0 return\n";
    let visual = "\
<thing>
  ::tree>
    :visual>
      @on_create>
        \"thing ^prim call &thing export
        \"world/conifer &thing.texture set
        #ffffff &thing.tint set
        0.25 &thing.subframe.x set
        0.5 &thing.subframe.w set
        0.6 &thing.subframe.v2.x set
        0.4 &thing.subframe.n.x set
        0.9 &thing.subframe.v3.r1.x set
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
    let v = b.visual_for_object(1).expect("tree visual");
    let f = &v.dir_frames;
    assert_eq!(f.len(), VARIANTS_PER_DEF);
    assert_eq!(f[0].len(), ROTATIONS_PER_DEF);
    assert_eq!(f[0][0].sub.0, 0.25);
    assert_eq!(f[2][0].sub.0, 0.6);
    assert_eq!(f[2][5].sub.0, 0.6);
    assert_eq!(f[0][2].sub.0, 0.4);
    assert_eq!(f[7][2].sub.0, 0.4);
    assert_eq!(f[2][2].sub.0, 0.6);
    assert_eq!(f[3][1].sub.0, 0.9);
    assert_eq!(f[3][0].sub.0, 0.25, "v3's OTHER rotations still fall back");
    assert!(f.iter().all(|byr| byr.iter().all(|d| d.sub.2 == 0.5)));
    let shrub = b.visual_for_object(2).expect("shrub visual");
    assert!(shrub.dir_frames.iter().all(|byr| byr.iter().all(|d| *d == DirFrame::default())));
    assert_eq!(b.thing_subframe().len(), 2 * VARIANTS_PER_DEF * ROTATIONS_PER_DEF * 6);
  }

  #[test]
  fn thing_speed_from_data_define() {
    let data = "<thing>\n  ::tree>\n    :data>\n      @define>\n        0 return\n  ::wolf>\n    :data>\n      @define>\n        12 &thing.speed set\n        0 return\n";
    let b = load(&[src("data/things.rd", data)]).expect("load");
    assert_eq!(b.thing_speed(2), Some(12));
    assert_eq!(b.thing_speed(1), None);
    assert_eq!(b.thing_speeds(), vec![0.0, 12.0]);
  }

  #[test]
  fn material_registry_and_packed_channels() {
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

    let stone = b.visual_for_def(2).unwrap();
    assert_eq!(stone.packed[0], PackedChannel { material_id: 1, tint: 0x6b6b6b });
    assert_eq!(stone.packed[1], PackedChannel::default());
    let grass = b.visual_for_def(1).unwrap();
    assert_eq!(grass.packed, [PackedChannel::default(); 4]);

    let table = b.tile_packed_channels();
    assert_eq!(table[0], [PackedChannel::default(); 4]);
    assert_eq!(table[1][0], PackedChannel { material_id: 1, tint: 0x6b6b6b });
  }

  #[test]
  fn needs_and_moodlets_registry() {
    let needs = "\
<need>
  ::thirst>
    @define>
      \"Thirst &need.label set
      864000 &need.deplete set
      \"thirsty &need.band.0.moodlet set
      0.10 &need.band.0.lo set
      0.35 &need.band.0.hi set
      \"dehydrated &need.band.1.moodlet set
      0 &need.band.1.lo set
      0.10 &need.band.1.hi set
      0 return
  ::rest>
    @define>
      0 return
<moodlet>
  ::thirsty>
    @define>
      \"Thirsty &moodlet.label set
      -0.15 &moodlet.mood set
      0 return
  ::dehydrated>
    @define>
      \"Dehydrated &moodlet.label set
      -0.40 &moodlet.mood set
      0 return
  ::quenched>
    @define>
      \"Quenched &moodlet.label set
      0.20 &moodlet.mood set
      3600 &moodlet.duration set
      0 return
";
    let b = load(&[src("data/needs.rd", needs)]).expect("clean load");
    assert_eq!(b.need_id("thirst"), Some(1));
    assert_eq!(b.need_name(1), Some("thirst"));
    assert_eq!(b.moodlet_id("dehydrated"), Some(2));
    assert_eq!(b.need_id("nope"), None);
    let t = b.need_params("thirst").unwrap();
    assert_eq!(t.label, "Thirst");
    assert_eq!(t.deplete, 864000.0);
    assert_eq!(t.bands.len(), 2, "two bands coexist on one need");
    assert_eq!(t.bands[0], NeedBand { moodlet: "thirsty".into(), lo: 0.10, hi: 0.35 });
    assert_eq!(t.bands[1], NeedBand { moodlet: "dehydrated".into(), lo: 0.0, hi: 0.10 });
    let r = b.need_params("rest").unwrap();
    assert_eq!((r.label.as_str(), r.deplete, r.bands.len()), ("rest", 0.0, 0));
    let thirsty = b.moodlet_params("thirsty").unwrap();
    assert_eq!((thirsty.label.as_str(), thirsty.mood, thirsty.duration), ("Thirsty", -0.15, 0.0));
    let quenched = b.moodlet_params("quenched").unwrap();
    assert_eq!((quenched.mood, quenched.duration), (0.20, 3600.0));
    assert_eq!(b.need_params_all().len(), 2);
    assert_eq!(b.moodlet_params_all().len(), 3);
  }

  #[test]
  fn thing_needs_from_data_define() {
    let needs = "<need>\n  ::thirst>\n    @define>\n      0 return\n";
    let things = "\
<thing>
  ::tree>
    :data>
      @define>
        0 return
  ::wolf>
    :data>
      @define>
        \"thirst &thing.needs.0 set
        \"typo_need &thing.needs.1 set
        0 return
";
    let b = load(&[src("data/needs.rd", needs), src("data/things.rd", things)]).expect("load");
    assert_eq!(b.thing_needs(2), vec![1], "thirst resolves, the typo drops");
    assert_eq!(b.thing_needs(1), Vec::<u16>::new());
    let table = b.thing_needs_table();
    assert_eq!(table.len(), 2 * NEEDS_PER_KIND);
    assert_eq!(table[NEEDS_PER_KIND], 1.0, "wolf slot 0 → need id 1");
    assert_eq!(table[0], 0.0, "tree slot 0 empty");
  }

  #[test]
  fn unset_material_params_default_to_identity() {
    let material = "<material>\n  ::blank>\n    @on_create>\n      0 return\n";
    let b = load(&[src("material/materials.rd", material)]).expect("load");
    let mp = b.material_params("blank").unwrap();
    assert_eq!(mp, MaterialParams::default());
    assert_eq!(mp.sample_space, "uv");
    assert_eq!(mp.hue_swing, 0.0);
  }

  fn biome_corpus() -> Vec<(String, String)> {
    let biomes = "\
<biome>
  ::ocean>
    @subtype>
      1 return
    @define>
      ^biome call &biome set
      *biome.2 0.32 lt return
    @on_create>
      water &tile set
      0 return
  ::forest>
    @subtype>
      6 return
    @define>
      ^biome call &biome set
      *biome.1 0.55 ge return
    @on_create>
      grass &tile set
      6 ^rand call 0.99 lt if tree &thing.1 set
      0 return
";
    vec![src("biome/biomes.rd", biomes)]
  }

  #[test]
  fn generate_reads_tile_and_scattered_thing() {
    let b = load(&biome_corpus()).expect("clean load");
    let g = b.generate(&[0.5, 0.7, 0.6], 42);
    assert_eq!(g.biome.as_deref(), Some("forest"));
    assert_eq!(g.tile.as_deref(), Some("grass"));
    assert_eq!(g.thing1.as_deref(), Some("tree"));
    let o = b.generate(&[0.5, 0.7, 0.2], 42);
    assert_eq!(o.tile.as_deref(), Some("water"));
    assert_eq!(o.thing1, None);
    // subtype ids are explicit, decoupled from evaluation order
    assert_eq!(b.biome_subtype_id("ocean"), Some(1));
    assert_eq!(b.biome_subtype_id("forest"), Some(6));
  }

  #[test]
  fn mixed_dialects_refuse_to_load() {
    let e = load(&[src("data/tiles.rd", "<tile>\n"), src("tiles.toml", "")]).unwrap_err();
    assert!(e[0].message.contains("mixed corpus"));
  }
}
