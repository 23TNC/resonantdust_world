//! Worldgen — drive the biome DSL to build a zone's packed terrain.
//!
//! This is the server's `:data`-side use of the DSL: load the content corpus
//! once at startup, then answer "what does a fresh zone look like?" as the packed
//! `tiles` (`Vec<u16>`) and `things` (`Vec<u32>`) a zone's baseline is seeded
//! from. The edge owns generation because the DSL is pure Rust it links as an
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

use resonantdust_codec::packed::{
    cell, pack_thing_at, pack_tile, zone_region_x, zone_region_y, zone_x, zone_y, REGION_DIM,
    ZONE_DIM, ZONE_TILES,
};
use resonantdust_dsl::Bundle;

/// The tile a cell falls back to when its biome named no ground (or an unknown
/// one). Grass is the neutral floor; the corpus must define it.
const DEFAULT_TILE: &str = "grass";

/// Feature size of each biome-dimension noise field, in world tiles — roughly the
/// lattice spacing, so a band spans a couple of these. Distinct per dimension so
/// temperature, humidity, and elevation vary at different scales and don't stripe
/// together.
const TEMPERATURE_SCALE: f64 = 48.0;
const HUMIDITY_SCALE: f64 = 34.0;
const ELEVATION_SCALE: f64 = 26.0;

/// Per-dimension lattice offsets — each field samples a different region of the
/// integer hash lattice, so the three dimensions are independent rather than
/// correlated copies of one noise field.
const TEMPERATURE_OFFSET: f64 = 131.0;
const HUMIDITY_OFFSET: f64 = 517.0;
const ELEVATION_OFFSET: f64 = 911.0;

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
    pub fn zone_terrain(&self, zone_id: u32) -> (Vec<u16>, Vec<u32>) {
        let (ox, oy) = zone_world_origin(zone_id);
        let mut tiles = vec![0u16; ZONE_TILES];
        let mut things = Vec::new();
        for y in 0..ZONE_DIM {
            for x in 0..ZONE_DIM {
                let (wx, wy) = (ox + x as i32, oy + y as i32);
                let gen = self.bundle.generate(&biome_dims(wx, wy), tile_seed(wx, wy));
                let loc = cell(x, y);

                let tile_id = gen
                    .tile
                    .as_deref()
                    .and_then(|name| self.bundle.tile_def_id(name))
                    .unwrap_or(self.default_tile);
                tiles[loc as usize] = pack_tile(tile_id, 0);

                if let Some(obj) = gen.thing1.as_deref().and_then(|n| self.bundle.thing_object_id(n)) {
                    things.push(pack_thing_at(loc, 0, obj));
                }
            }
        }
        (tiles, things)
    }
}

/// The three biome dimensions at a world tile: temperature, humidity, elevation,
/// each in `[0, 1)`. Independent noise fields (distinct scale + lattice offset),
/// so a cell's climate is a point in that 3-space the biomes partition.
fn biome_dims(wx: i32, wy: i32) -> [f64; 3] {
    let sample = |scale: f64, off: f64| {
        value_noise(wx as f64 / scale + off, wy as f64 / scale + off)
    };
    [
        sample(TEMPERATURE_SCALE, TEMPERATURE_OFFSET),
        sample(HUMIDITY_SCALE, HUMIDITY_OFFSET),
        sample(ELEVATION_SCALE, ELEVATION_OFFSET),
    ]
}

/// A per-tile RNG seed from its world coordinates — what `^rand` salts and
/// finalizes. Deterministic in `(wx, wy)`, so a cell's scatter reproduces on a
/// re-seed; the VM's `rand` does the salt-mixing, this just spreads the coords.
fn tile_seed(wx: i32, wy: i32) -> u64 {
    let mut h = (wx as u32 as u64) | ((wy as u32 as u64) << 32);
    h ^= h >> 33;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 29;
    h
}

/// World-tile coordinate of a zone's top-left cell. The world is a grid of
/// `REGION_DIM × REGION_DIM` zones per region, each `ZONE_DIM` tiles square, so a
/// zone's origin is `(region * REGION_DIM + zone) * ZONE_DIM` on each axis.
/// (Surface isn't a spatial axis — different surfaces share the same plane.)
fn zone_world_origin(zone_id: u32) -> (i32, i32) {
    let span = REGION_DIM as i32 * ZONE_DIM as i32;
    let ox = zone_region_x(zone_id) as i32 * span + zone_x(zone_id) as i32 * ZONE_DIM as i32;
    let oy = zone_region_y(zone_id) as i32 * span + zone_y(zone_id) as i32 * ZONE_DIM as i32;
    (ox, oy)
}

/// Smooth value noise in `[0, 1)` at a continuous `(x, y)`: hash the four integer
/// lattice corners and smoothstep-interpolate between them. One octave — cheap,
/// smooth, and enough for coherent biome blobs.
fn value_noise(x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (ix, iy) = (x0 as i32, y0 as i32);
    let sx = smoothstep(x - x0);
    let sy = smoothstep(y - y0);
    let nx0 = lerp(lattice(ix, iy), lattice(ix + 1, iy), sx);
    let nx1 = lerp(lattice(ix, iy + 1), lattice(ix + 1, iy + 1), sx);
    lerp(nx0, nx1, sy)
}

/// Deterministic pseudo-random value in `[0, 1)` for an integer lattice point —
/// an integer hash (xorshift-multiply mix) normalised to a fraction.
fn lattice(x: i32, y: i32) -> f64 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h as f64 / (u32::MAX as f64 + 1.0)
}

/// Hermite smoothstep `3t² − 2t³`, easing the lattice interpolation so biome
/// edges are smooth rather than linearly creased.
fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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
    use resonantdust_codec::packed::{pack_zone_id, tile_def};

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
    @define>
      ^biome call &biome set
      *biome.2 0.32 lt return
    @on_create>
      water &tile set
      0 return
  ::mountains>
    @define>
      ^biome call &biome set
      *biome.2 0.82 ge return
    @on_create>
      stone &tile set
      0 return
  ::forest>
    @define>
      ^biome call &biome set
      *biome.1 0.55 ge return
    @on_create>
      grass &tile set
      1 ^rand call 0.5 lt if tree &thing.1 set
      0 return
  ::plains>
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
        // every ground def resolves to one of the corpus tiles (never 0/empty)
        let known: Vec<u16> = ["grass", "dirt", "water", "stone"]
            .iter()
            .filter_map(|n| w.bundle.tile_def_id(n))
            .collect();
        for &slot in &tiles {
            assert!(known.contains(&tile_def(slot)), "unexpected tile def {}", tile_def(slot));
        }
    }

    #[test]
    fn things_land_on_grass_cells_only() {
        // The test forest scatters trees on grass; a thing must never sit on a
        // water/stone cell (those biomes place nothing).
        let w = worldgen();
        let grass = w.bundle.tile_def_id("grass").unwrap();
        let tree = w.bundle.thing_object_id("tree").unwrap();
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let zone_id = pack_zone_id(0, 0, 0, zx, zy);
                let (tiles, things) = w.zone_terrain(zone_id);
                for &t in &things {
                    use resonantdust_codec::packed::{thing_location, thing_object_id};
                    assert_eq!(thing_object_id(t), tree);
                    assert_eq!(tile_def(tiles[thing_location(t) as usize]), grass);
                }
            }
        }
    }

    #[test]
    fn a_region_shows_variety() {
        // A whole region (16×16 zones) should surface more than one biome's
        // ground — at minimum some non-grass tile from an elevation band.
        let w = worldgen();
        let grass = w.bundle.tile_def_id("grass").unwrap();
        let mut saw_grass = false;
        let mut saw_other = false;
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let (tiles, _) = w.zone_terrain(pack_zone_id(0, 0, 0, zx, zy));
                for &slot in &tiles {
                    if tile_def(slot) == grass {
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
        let zone_id = pack_zone_id(0, 0, 0, 3, 5);
        assert_eq!(w.zone_terrain(zone_id), w.zone_terrain(zone_id));
    }

    #[test]
    fn seamless_across_zone_boundary() {
        // The east edge of a zone and the west edge of its neighbour read
        // adjacent world columns, so the ground must not break at the seam: the
        // noise is continuous and both columns sample the same world function.
        let w = worldgen();
        let (west, _) = w.zone_terrain(pack_zone_id(0, 0, 0, 0, 0));
        let (east, _) = w.zone_terrain(pack_zone_id(0, 0, 0, 1, 0));
        let ground = |wx: i32, wy: i32| {
            let g = w.bundle.generate(&biome_dims(wx, wy), tile_seed(wx, wy));
            g.tile.and_then(|n| w.bundle.tile_def_id(&n)).unwrap_or(w.default_tile)
        };
        for y in 0..ZONE_DIM as i32 {
            assert_eq!(tile_def(west[cell(ZONE_DIM - 1, y as u8) as usize]), ground(15, y));
            assert_eq!(tile_def(east[cell(0, y as u8) as usize]), ground(16, y));
        }
    }
}
