//! Worldgen — drive the biome DSL to build a zone's packed terrain.
//!
//! This is the server's `:data`-side use of the DSL: load the content corpus
//! once at startup, then answer "what does a fresh zone look like?" as the dense
//! `tiles` (`Vec<u16>`, one `kind_reference` per cell, indexed by `tile_reference`) and the sparse
//! `things` (`Vec<u32>` of `kind_pos_reference`s, `kind:16 | tile:8 | data:8`) a zone's baseline is
//! seeded from — held in separate cold shards, keyed by `macro_position_reference`. The edge owns
//! generation because the DSL is pure Rust it links as an
//! rlib and it already reads `content/` off disk — a SpacetimeDB module is wasm
//! with neither, so it just stores what we hand it. (The cold-zone seeding path
//! itself is a follow-up in the merged pipeline — see docs/archive/gaps.md.)
//!
//! Generation is per tile and independent. For each cell we sample N **biome
//! dimensions** (temperature / humidity / elevation) from noise at the cell's
//! world position, then let the DSL classify it: [`Bundle::generate`] walks the
//! biomes in content order, takes the first whose `@define` matches, and runs
//! that biome's `@on_create` to pick the ground tile and scatter any primary-
//! layer thing (a tree, a shrub). We resolve those names to `def_id`s through the
//! same bundle the client loads — so the terrain the server writes and the
//! terrain the client paints agree by id — and pack them.
//!
//! Sampling in **world** tile coordinates (not per-zone) keeps biomes coherent
//! across zone seams: the cell on each side of a boundary reads adjacent world
//! coordinates from the same continuous noise, so blobs tile seamlessly. Purely a
//! function of world position + a per-tile seed, so a re-seed reproduces the zone.

use std::path::Path;

use resonantdust_codec::biome::{biome_dims, tile_seed};
use resonantdust_codec::object::{
    macro_world_origin, pack_kind_pos_reference, pack_kind_reference, pack_tile_reference, ZONE_DIM,
    ZONE_TILES,
};
use resonantdust_content::Bundle;

/// The tile a cell falls back to when its biome named no ground (or an unknown
/// one). Grass is the neutral floor; the corpus must define it.
const DEFAULT_TILE: &str = "grass";

/// A loaded content corpus plus the fallback tile id. Built once at startup and
/// shared (read-only) across every zone seed.
pub struct Worldgen {
    bundle: Bundle,
    default_tile: u16,
}

/// A freshly-loaded worldgen plus its corpus fingerprint — the unit the server's
/// content hot-reload works with, so a poll can skip rebuilding when the
/// fingerprint hasn't moved (see [`Worldgen::load_versioned`]).
pub struct LoadedWorldgen {
    /// [`resonantdust_content::content::content_version`] of the loaded corpus.
    pub version: u64,
    pub worldgen: Worldgen,
}

impl Worldgen {
    /// Load the content tree under `content_dir` into a worldgen plus the corpus
    /// fingerprint the hot-reload compares across polls. Errors (as a single
    /// joined string) if the corpus won't parse, defines no biomes, or is missing
    /// the default tile.
    pub fn load_versioned(content_dir: &Path) -> Result<LoadedWorldgen, String> {
        let sources = resonantdust_content::content::read_content_dir(content_dir)
            .map_err(|e| format!("read content {}: {e}", content_dir.display()))?;
        let version = resonantdust_content::content::content_version(&sources);
        let worldgen = Self::from_sources(&sources)?;
        Ok(LoadedWorldgen { version, worldgen })
    }

    /// Whether swapping to `self` is safe for zones already generated with `prev`:
    /// tile and thing ids must be **append-compatible** — every one of `prev`'s
    /// names still present, in the same order (new ones may follow). A reorder or
    /// removal would renumber ids, so a stored zone's packed tiles/things would be
    /// misread — the hot-reload refuses that and asks for a restart instead.
    pub fn is_append_compatible_with(&self, prev: &Worldgen) -> bool {
        is_prefix(prev.bundle().tile_names(), self.bundle().tile_names())
            && is_prefix(prev.bundle().thing_names(), self.bundle().thing_names())
    }

    /// Build worldgen from already-read `(name, source)` pairs — the core of
    /// [`load`], split out so it's testable without touching the filesystem.
    pub fn from_sources(sources: &[(String, String)]) -> Result<Worldgen, String> {
        let bundle = resonantdust_content::load(sources).map_err(|errs| {
            errs.iter().map(|e| format!("{}: {}", e.file, e.message)).collect::<Vec<_>>().join("; ")
        })?;
        if bundle.biome_names().is_empty() {
            return Err("content defines no biomes (content/biome/*.rd)".into());
        }
        let default_tile = bundle
            .tile_def_id(DEFAULT_TILE)
            .ok_or_else(|| format!("content defines no default tile {DEFAULT_TILE:?}"))?;
        Ok(Worldgen { bundle, default_tile })
    }

    /// The loaded corpus, for callers that want other tile queries.
    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    /// A fresh zone's cold layers for the split `tile` / `thing` shards
    /// ([`docs/intent/world-storage/`]), **grouped by biome `subtype`** — one row per biome present in
    /// the zone (`cold_row_reference` = `macro | subtype | layer_id`; `type_id` is the shard). The
    /// **dense ground** is 256 `kind_reference`s per subtype (cells outside that biome `0`); the
    /// **sparse scatter** is that biome's `kind_pos_reference`s. Deterministic in world position +
    /// subtype order, so a re-seed reproduces the zone. Worldgen writes `layer_id 0` (the primary
    /// layer); the edge supplies it at `seed`.
    pub fn zone_cold(&self, macro_position: u16) -> ColdLayers {
        use std::collections::BTreeMap;
        let (ox, oy) = macro_world_origin(macro_position);
        let mut tiles: BTreeMap<u16, Vec<u16>> = BTreeMap::new(); // subtype → dense 256
        let mut things: BTreeMap<u16, Vec<u32>> = BTreeMap::new(); // subtype → sparse
        for y in 0..ZONE_DIM {
            for x in 0..ZONE_DIM {
                let (wx, wy) = (ox + x as i32, oy + y as i32);
                let seed = tile_seed(wx, wy);
                let gen = self.bundle.generate(&biome_dims(wx, wy), seed);
                let tref = pack_tile_reference(x, y);
                // The cell's biome = the row's `subtype` (0/default if it names none).
                let subtype =
                    gen.biome.as_deref().and_then(|b| self.bundle.biome_subtype_id(b)).unwrap_or(0);

                // Ground — every cell, into its biome's dense row (allocated on first sight).
                let tile_kind =
                    gen.tile.as_deref().and_then(|n| self.bundle.tile_def_id(n)).unwrap_or(self.default_tile);
                let tile_variant = (seed >> 13) as u8 & 0x0F;
                tiles.entry(subtype).or_insert_with(|| vec![0u16; ZONE_TILES])[tref as usize] =
                    pack_kind_reference(tile_kind, tile_variant);

                // Scatter — sparse, into its biome's row, only where the biome placed a thing.
                if let Some(thing_kind) = gen.thing1.as_deref().and_then(|n| self.bundle.thing_object_id(n)) {
                    let thing_variant = (seed >> 21) as u8 & 0x0F;
                    things.entry(subtype).or_default().push(pack_kind_pos_reference(
                        pack_kind_reference(thing_kind, thing_variant),
                        tref,
                        0,
                    ));
                }
            }
        }
        ColdLayers { tiles: tiles.into_iter().collect(), things: things.into_iter().collect() }
    }
}

/// A fresh zone's cold rows, grouped by biome `subtype` — the [`Worldgen::zone_cold`] result. Each
/// entry is `(subtype_id, payload)`; the edge seeds one `cold_tile` / `cold_thing` row per entry.
/// Ordered by `subtype` (deterministic).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColdLayers {
    /// Per-biome dense ground: `(subtype_id, 256 kind_references)` — cells outside the biome `0`.
    pub tiles: Vec<(u16, Vec<u16>)>,
    /// Per-biome sparse scatter: `(subtype_id, kind_pos_references)`.
    pub things: Vec<(u16, Vec<u32>)>,
}

/// Whether `prefix` is a leading sub-slice of `full` (same items, same order) —
/// the id append-compatibility test for a content hot-reload.
fn is_prefix(prefix: &[String], full: &[String]) -> bool {
    full.len() >= prefix.len() && full[..prefix.len()] == *prefix
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use resonantdust_codec::object::{pack_macro_position, REGION_DIM};

    /// A region-0 `macro_position_reference` for zone `(zx, zy)` — what `zone_cold` keys on.
    fn zone_macro(zx: u8, zy: u8) -> u16 {
        pack_macro_position(0, pack_tile_reference(zx, zy))
    }

    /// Overlay a zone's per-biome dense ground rows into one 256-array — each cell from whichever
    /// biome's row populated it (rows are disjoint by cell, so order doesn't matter).
    fn merged_tiles(layers: &ColdLayers) -> Vec<u16> {
        let mut out = vec![0u16; ZONE_TILES];
        for (_subtype, tiles) in &layers.tiles {
            for (i, &k) in tiles.iter().enumerate() {
                if k != 0 {
                    out[i] = k;
                }
            }
        }
        out
    }

    /// Every biome-row's scatter, flattened.
    fn all_things(layers: &ColdLayers) -> Vec<u32> {
        layers.things.iter().flat_map(|(_s, t)| t.iter().copied()).collect()
    }

    /// Hermetic worldgen over a small biome corpus shaped like the real
    /// content/{data,visual,biome} tree — no filesystem, so it runs anywhere.
    /// Elevation bands (ocean/mountain) plus a humid forest and a plains
    /// catch-all, so a whole region exercises several biomes.
    fn worldgen() -> Worldgen {
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
  ::stone>
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
";
        let biome = "\
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
  ::mountains>
    @subtype>
      3 return
    @define>
      ^biome call &biome set
      *biome.2 0.82 ge return
    @on_create>
      stone &tile set
      0 return
  ::forest>
    @subtype>
      6 return
    @define>
      ^biome call &biome set
      *biome.1 0.55 ge return
    @on_create>
      grass &tile set
      1 ^rand call 0.5 lt if tree &thing.1 set
      0 return
  ::plains>
    @subtype>
      7 return
    @define>
      1 return
    @on_create>
      grass &tile set
      0 return
";
        Worldgen::from_sources(&[
            ("data/tiles.rd".into(), data.into()),
            ("data/things.rd".into(), things.into()),
            ("biome/biomes.rd".into(), biome.into()),
        ])
        .expect("load content")
    }

    #[test]
    fn rejects_a_corpus_with_no_biomes() {
        let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n";
        match Worldgen::from_sources(&[("data/tiles.rd".into(), data.into())]) {
            Err(err) => assert!(err.contains("no biomes"), "{err}"),
            Ok(_) => panic!("a biome-less corpus should be rejected"),
        }
    }

    /// Build a worldgen from bare tile / thing name lists (a trivial always-true
    /// `plains` biome painting grass). `tiles` must include `grass` (the default).
    fn wg(tiles: &[&str], things: &[&str]) -> Worldgen {
        let mut data = String::from("<tile>\n");
        for t in tiles {
            data += &format!("  ::{t}>\n    :data>\n      @define>\n        0 return\n");
        }
        let mut th = String::from("<thing>\n");
        for t in things {
            th += &format!("  ::{t}>\n    :data>\n      @define>\n        0 return\n");
        }
        let biome = "<biome>\n  ::plains>\n    @define>\n      1 return\n    @on_create>\n      grass &tile set\n      0 return\n";
        Worldgen::from_sources(&[
            ("data/tiles.rd".into(), data),
            ("data/things.rd".into(), th),
            ("biome/biomes.rd".into(), biome.into()),
        ])
        .expect("load content")
    }

    #[test]
    fn append_compatible_allows_appends_only() {
        let base = wg(&["grass", "dirt"], &["tree"]);
        // appending tiles and things keeps every existing id → safe to hot-swap
        assert!(wg(&["grass", "dirt", "sand"], &["tree", "shrub"]).is_append_compatible_with(&base));
        // reordering renumbers existing ids → refused
        assert!(!wg(&["dirt", "grass"], &["tree"]).is_append_compatible_with(&base));
        // removing a tile renumbers the rest → refused
        assert!(!wg(&["grass"], &["tree"]).is_append_compatible_with(&base));
        // removing a thing is caught too (separate id namespace)
        assert!(!wg(&["grass", "dirt"], &[]).is_append_compatible_with(&base));
        // identical corpus is trivially compatible
        assert!(wg(&["grass", "dirt"], &["tree"]).is_append_compatible_with(&base));
    }

    #[test]
    fn every_cell_gets_a_known_tile() {
        use resonantdust_codec::object::kind_ref_kind_id;
        let w = worldgen();
        let tiles = merged_tiles(&w.zone_cold(0));
        assert_eq!(tiles.len(), ZONE_TILES);
        // every dense ground slot resolves to one of the corpus tile-kinds (never 0/empty)
        let known: Vec<u16> = ["grass", "dirt", "water", "stone"]
            .iter()
            .filter_map(|n| w.bundle.tile_def_id(n).map(|d| d as u16))
            .collect();
        for &tile in &tiles {
            let kind = kind_ref_kind_id(tile);
            assert!(known.contains(&kind), "unexpected tile-kind {kind}");
        }
    }

    #[test]
    fn things_scatter_kind_tree_only() {
        // The test forest is the only biome that scatters, and it scatters trees;
        // every sparse thing must therefore carry kind `tree`.
        use resonantdust_codec::object::kind_pos_ref_kind_id;
        let w = worldgen();
        let tree = w.bundle.thing_object_id("tree").unwrap();
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                for t in all_things(&w.zone_cold(zone_macro(zx, zy))) {
                    assert_eq!(kind_pos_ref_kind_id(t), tree);
                }
            }
        }
    }

    #[test]
    fn a_region_shows_variety() {
        // A whole region (16×16 zones) should surface more than one biome's
        // ground — at minimum some non-grass tile from an elevation band.
        use resonantdust_codec::object::kind_ref_kind_id;
        let w = worldgen();
        let grass = w.bundle.tile_def_id("grass").unwrap() as u16;
        let mut saw_grass = false;
        let mut saw_other = false;
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let tiles = merged_tiles(&w.zone_cold(zone_macro(zx, zy)));
                for &tile in &tiles {
                    if kind_ref_kind_id(tile) == grass {
                        saw_grass = true;
                    } else {
                        saw_other = true;
                    }
                }
            }
        }
        assert!(saw_grass && saw_other, "region should show grass and at least one other biome");
    }

    #[test]
    fn deterministic_reseed() {
        let w = worldgen();
        let m = zone_macro(3, 5);
        assert_eq!(w.zone_cold(m), w.zone_cold(m));
    }

    #[test]
    fn seamless_across_zone_boundary() {
        // The east edge of a zone and the west edge of its neighbour read
        // adjacent world columns, so the ground must not break at the seam: the
        // noise is continuous and both columns sample the same world function.
        // This is the property the transposition bug broke — the dense index must
        // be read with the same `tile_reference` (x high nibble) worldgen wrote.
        use resonantdust_codec::object::kind_ref_kind_id;
        let w = worldgen();
        let west = merged_tiles(&w.zone_cold(zone_macro(0, 0)));
        let east = merged_tiles(&w.zone_cold(zone_macro(1, 0)));
        let ground = |wx: i32, wy: i32| {
            let g = w.bundle.generate(&biome_dims(wx, wy), tile_seed(wx, wy));
            g.tile.and_then(|n| w.bundle.tile_def_id(&n)).unwrap_or(w.default_tile) as u16
        };
        for y in 0..ZONE_DIM as i32 {
            // west zone's east edge column (tile_x = 15) → world column 15;
            // east zone's west edge column (tile_x = 0) → world column 16. Adjacent.
            let west_edge = pack_tile_reference(ZONE_DIM - 1, y as u8) as usize;
            let east_edge = pack_tile_reference(0, y as u8) as usize;
            assert_eq!(kind_ref_kind_id(west[west_edge]), ground(15, y));
            assert_eq!(kind_ref_kind_id(east[east_edge]), ground(16, y));
        }
    }
}
