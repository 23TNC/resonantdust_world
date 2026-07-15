//! Worldgen — drive the biome DSL to build a zone's packed terrain.
//!
//! This is the server's `:data`-side use of the DSL: load the content corpus
//! once at startup, then answer "what does a fresh zone look like?" as the dense
//! `tiles` (`Vec<u8>`, one tile-kind per cell) and the sparse `things` (`Vec<u64>`,
//! `kind:16|x:4|y:4|data:5|layer:3|variant:5|reserved:27`) a zone's baseline is seeded from — held in
//! separate shards. The edge owns generation because the DSL is pure Rust it links as an
//! rlib and it already reads `content/` off disk — a SpacetimeDB module is wasm
//! with neither, so it just stores what we hand it. (The cold-zone seeding path
//! itself is a follow-up in the merged pipeline — see docs/gaps.md.)
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

use resonantdust_codec::biome::{biome_dims, tile_seed, zone_world_origin};
use resonantdust_codec::object::{
    pack_cold_entry, pack_kind_reference, pack_position_reference, pack_type_reference,
    TYPE_BIOME_THING, TYPE_BIOME_TILE,
};
use resonantdust_codec::packed::{cell, pack_thing_at, ZONE_DIM, ZONE_TILES};
use resonantdust_dsl::Bundle;

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
    /// [`resonantdust_dsl::content::content_version`] of the loaded corpus.
    pub version: u64,
    pub worldgen: Worldgen,
}

/// One cold-storage row of a zone: every cold object that shares an
/// `object_type_reference` (type / subtype = biome / `layer`), with its members as
/// `object_kind_reference`s. This is the cold-table row shape from
/// `docs/object-model.md` §4 — keyed upstream by the zone's `region_zone_reference`;
/// the shared type half is amortised across the `Vec`. Both `biome-tile` (the dense
/// ground, one per cell) and `biome-thing` (sparse scatter) rows use this same
/// shape — a zone is just N such rows, split by (type, subtype, layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColdRow {
    /// The shared `object_type_reference` (type / subtype = biome / layer).
    pub type_reference: u32,
    /// One `object_kind_reference` per member (kind / subkind / variant / x / y / data).
    pub kinds: Vec<u32>,
}

impl Worldgen {
    /// Load the content tree under `content_dir` into a worldgen plus the corpus
    /// fingerprint the hot-reload compares across polls. Errors (as a single
    /// joined string) if the corpus won't parse, defines no biomes, or is missing
    /// the default tile.
    pub fn load_versioned(content_dir: &Path) -> Result<LoadedWorldgen, String> {
        let sources = resonantdust_dsl::content::read_content_dir(content_dir)
            .map_err(|e| format!("read content {}: {e}", content_dir.display()))?;
        let version = resonantdust_dsl::content::content_version(&sources);
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
        let bundle = resonantdust_dsl::load(sources).map_err(|errs| {
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

    /// A fresh zone's packed terrain: the `ZONE_TILES` ground slots (row-major,
    /// `cell(x, y)`) and the things scattered across it. Each cell samples its
    /// biome dimensions, lets the DSL classify it, and resolves the chosen tile /
    /// thing names to `def_id`s. Deterministic in the zone's world position, so a
    /// re-seed reproduces it and it's seamless across zone boundaries.
    pub fn zone_terrain(&self, zone_id: u32) -> (Vec<u8>, Vec<u64>) {
        let (ox, oy) = zone_world_origin(zone_id);
        let mut tiles = vec![0u8; ZONE_TILES];
        let mut things: Vec<u64> = Vec::new();
        for y in 0..ZONE_DIM {
            for x in 0..ZONE_DIM {
                let (wx, wy) = (ox + x as i32, oy + y as i32);
                let seed = tile_seed(wx, wy);
                let gen = self.bundle.generate(&biome_dims(wx, wy), seed);
                let loc = cell(x, y);

                let tile_id = gen
                    .tile
                    .as_deref()
                    .and_then(|name| self.bundle.tile_def_id(name))
                    .unwrap_or(self.default_tile);
                // Tile-kind is a plain u8 now (256 kinds; corpus ids are small).
                tiles[loc as usize] = tile_id as u8;

                if let Some(kind) = gen.thing1.as_deref().and_then(|n| self.bundle.thing_object_id(n))
                {
                    // Worldgen scatters one primary-layer thing per cell: data
                    // (rotation/count) 0 for flora, layer 0. A deterministic per-cell
                    // sprite variant (0..31) from a high slice of the same seed (kept off
                    // the low bits the scatter draw uses); the client renders it modulo the
                    // kind's actual variant count. Sparse — only occupied cells are pushed.
                    let variant = (seed >> 21) as u8 & 0x1F;
                    things.push(pack_thing_at(loc, kind, 0, 0, variant));
                }
            }
        }
        (tiles, things)
    }

    /// A fresh zone's cold objects as `object_reference`s, grouped into cold rows
    /// (`docs/object-model.md` §4). Same generation as [`zone_terrain`], but every
    /// cell contributes a **`biome-tile`** ground object (dense — one per cell) and,
    /// where the biome scattered one, a **`biome-thing`** object (sparse). Each is an
    /// `object_kind_reference` (its `kind` + in-zone `x`/`y` + `variant`) filed under
    /// the `object_type_reference` for its `(type, biome subtype, layer 0)`. So a
    /// zone is **N rows** split by (type, subtype, layer) — a `biome-tile` row per
    /// biome present (which is how the tile layer carries its biome, since a zone
    /// spans several), plus a `biome-thing` row per biome that scattered. Rows are
    /// ordered by `type_reference` (a `BTreeMap`), so the result is deterministic.
    ///
    /// The NEW cold representation, built **alongside** `zone_terrain`'s legacy
    /// `Vec<u8>` / `Vec<u64>` while the wire / shard / client migrate over. The biome
    /// becomes the `subtype` (its `@subtype` id; `0`/`default` if it carries none),
    /// which is what lets a cold row *be* the zone's biome. `variant` is `u4` (the
    /// client renders modulo the kind's real variant count).
    pub fn zone_cold_objects(&self, zone_id: u32) -> Vec<ColdRow> {
        use std::collections::BTreeMap;
        let (ox, oy) = zone_world_origin(zone_id);
        // object_type_reference (the shared type half) → its member kind refs.
        let mut rows: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for y in 0..ZONE_DIM {
            for x in 0..ZONE_DIM {
                let (wx, wy) = (ox + x as i32, oy + y as i32);
                let seed = tile_seed(wx, wy);
                let gen = self.bundle.generate(&biome_dims(wx, wy), seed);
                // subtype = the cell's biome (0/default if it carries no @subtype).
                let subtype =
                    gen.biome.as_deref().and_then(|b| self.bundle.biome_subtype_id(b)).unwrap_or(0);

                // Ground tile — dense (every cell), a biome-tile object at layer 0.
                // Falls back to the default tile when the biome named none. Its
                // variant comes from a seed slice distinct from the thing's, so the
                // ground and its scatter vary independently.
                let tile_kind =
                    gen.tile.as_deref().and_then(|n| self.bundle.tile_def_id(n)).unwrap_or(self.default_tile);
                let tile_type = pack_type_reference(TYPE_BIOME_TILE, subtype);
                let tile_variant = (seed >> 13) as u8 & 0x0F;
                rows.entry(tile_type).or_default().push(pack_cold_entry(
                    pack_kind_reference(tile_kind, tile_variant),
                    pack_position_reference(x, y),
                    0,
                ));

                // Scattered primary-layer thing — sparse, a biome-thing at layer 0.
                if let Some(thing_kind) = gen.thing1.as_deref().and_then(|n| self.bundle.thing_object_id(n)) {
                    let thing_type = pack_type_reference(TYPE_BIOME_THING, subtype);
                    let thing_variant = (seed >> 21) as u8 & 0x0F;
                    rows.entry(thing_type).or_default().push(pack_cold_entry(
                        pack_kind_reference(thing_kind, thing_variant),
                        pack_position_reference(x, y),
                        0,
                    ));
                }
            }
        }
        rows.into_iter().map(|(type_reference, kinds)| ColdRow { type_reference, kinds }).collect()
    }
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
    use resonantdust_codec::packed::{pack_zone_id, REGION_DIM};

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
        let w = worldgen();
        let (tiles, _) = w.zone_terrain(0);
        assert_eq!(tiles.len(), ZONE_TILES);
        // every ground def resolves to one of the corpus tile-kinds (never 0/empty)
        let known: Vec<u8> = ["grass", "dirt", "water", "stone"]
            .iter()
            .filter_map(|n| w.bundle.tile_def_id(n).map(|d| d as u8))
            .collect();
        for &tile in &tiles {
            assert!(known.contains(&tile), "unexpected tile-kind {tile}");
        }
    }

    #[test]
    fn things_land_on_grass_cells_only() {
        // The test forest scatters trees on grass; a thing must never sit on a
        // water/stone cell (those biomes place nothing).
        let w = worldgen();
        let grass = w.bundle.tile_def_id("grass").unwrap() as u8;
        let tree = w.bundle.thing_object_id("tree").unwrap();
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let zone_id = pack_zone_id(0, 0, 0, 0, zx, zy);
                let (tiles, things) = w.zone_terrain(zone_id);
                for &t in &things {
                    use resonantdust_codec::packed::{thing_kind, thing_location};
                    assert_eq!(thing_kind(t), tree);
                    assert_eq!(tiles[thing_location(t) as usize], grass);
                }
            }
        }
    }

    #[test]
    fn a_region_shows_variety() {
        // A whole region (16×16 zones) should surface more than one biome's
        // ground — at minimum some non-grass tile from an elevation band.
        let w = worldgen();
        let grass = w.bundle.tile_def_id("grass").unwrap() as u8;
        let mut saw_grass = false;
        let mut saw_other = false;
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let (tiles, _) = w.zone_terrain(pack_zone_id(0, 0, 0, 0, zx, zy));
                for &tile in &tiles {
                    if tile == grass {
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
        let zone_id = pack_zone_id(0, 0, 0, 0, 3, 5);
        assert_eq!(w.zone_terrain(zone_id), w.zone_terrain(zone_id));
    }

    #[test]
    fn cold_objects_carry_biome_subtype_and_kind() {
        use resonantdust_codec::object::{kind_ref_kind_id, kind_ref_x, kind_ref_y, type_ref_subtype_id, type_ref_type_id};
        // The test corpus scatters trees ONLY in forest (@subtype 6); ground tiles
        // are dense (one biome-tile per cell) across whatever biomes a zone spans.
        let w = worldgen();
        let tree = w.bundle.thing_object_id("tree").unwrap();
        let forest = w.bundle.biome_subtype_id("forest").unwrap();
        assert_eq!(forest, 6);
        let mut saw_tree = false;
        let mut saw_tile = false;
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                for row in w.zone_cold_objects(pack_zone_id(0, 0, 0, 0, zx, zy)) {
                    for k in &row.kinds {
                        assert!(kind_ref_x(*k) < 16 && kind_ref_y(*k) < 16, "in-zone position");
                    }
                    match type_ref_type_id(row.type_reference) {
                        // things only scatter in forest → subtype 6, kind tree.
                        t if t == TYPE_BIOME_THING => {
                            assert_eq!(type_ref_subtype_id(row.type_reference), forest);
                            for k in &row.kinds {
                                assert_eq!(kind_ref_kind_id(*k), tree);
                                saw_tree = true;
                            }
                        }
                        // a biome-tile row's subtype is a real biome (its @subtype id).
                        t if t == TYPE_BIOME_TILE => saw_tile = true,
                        other => panic!("unexpected cold type {other}"),
                    }
                }
            }
        }
        assert!(saw_tree, "some zone should scatter a tree");
        assert!(saw_tile, "every zone has ground tiles");
    }

    #[test]
    fn cold_objects_match_the_legacy_terrain() {
        use resonantdust_codec::object::type_ref_type_id;
        // The new object_reference path must reproduce the legacy terrain: one
        // biome-tile per cell (dense), and exactly the legacy thing scatter.
        let w = worldgen();
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let zone_id = pack_zone_id(0, 0, 0, 0, zx, zy);
                let (legacy_tiles, legacy_things) = w.zone_terrain(zone_id);
                let rows = w.zone_cold_objects(zone_id);
                let count = |t: u8| -> usize {
                    rows.iter().filter(|r| type_ref_type_id(r.type_reference) == t).map(|r| r.kinds.len()).sum()
                };
                assert_eq!(count(TYPE_BIOME_TILE), legacy_tiles.len(), "one biome-tile per cell");
                assert_eq!(count(TYPE_BIOME_THING), legacy_things.len(), "same thing scatter as legacy");
            }
        }
    }

    #[test]
    fn seamless_across_zone_boundary() {
        // The east edge of a zone and the west edge of its neighbour read
        // adjacent world columns, so the ground must not break at the seam: the
        // noise is continuous and both columns sample the same world function.
        let w = worldgen();
        let (west, _) = w.zone_terrain(pack_zone_id(0, 0, 0, 0, 0, 0));
        let (east, _) = w.zone_terrain(pack_zone_id(0, 0, 0, 0, 1, 0));
        let ground = |wx: i32, wy: i32| {
            let g = w.bundle.generate(&biome_dims(wx, wy), tile_seed(wx, wy));
            g.tile.and_then(|n| w.bundle.tile_def_id(&n)).unwrap_or(w.default_tile) as u8
        };
        for y in 0..ZONE_DIM as i32 {
            assert_eq!(west[cell(ZONE_DIM - 1, y as u8) as usize], ground(15, y));
            assert_eq!(east[cell(0, y as u8) as usize], ground(16, y));
        }
    }
}
