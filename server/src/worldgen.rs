//! Worldgen — turn DSL tile names into a zone's packed terrain.
//!
//! This is the server's `:data`-side use of the DSL: load the content corpus
//! once at startup, then answer "what does a fresh zone's 16×16 terrain look
//! like?" as a `Vec<u16>` of packed tile slots the shard's `seed_cold_zone`
//! stores. Each slot is `def_id:12 | reserved:4`
//! ([`resonantdust_codec::packed::pack_tile`]); the `def_id` is the content-
//! derived id the client maps back to a colour through the same corpus, so the
//! terrain the server writes and the terrain the client paints agree by id.
//!
//! Terrain is a basic two-biome split: deterministic value-noise sampled in
//! **world** tile coordinates picks grass or dirt per cell. Sampling in world
//! space (not per-zone) means biomes are coherent blobs many tiles across that
//! tile seamlessly over zone boundaries — the cell on each side of a seam reads
//! adjacent world coordinates, so no discontinuity. Real terrain (multi-octave
//! noise, more biomes, features) is a later layer; this is the floor that gives
//! grass/dirt coherent regions instead of a mechanical checker.

use std::path::Path;

use resonantdust_codec::packed::{
    cell, pack_tile, zone_region_x, zone_region_y, zone_x, zone_y, REGION_DIM, ZONE_DIM, ZONE_TILES,
};
use resonantdust_dsl::Bundle;

/// The two tiles the current content defines. Worldgen needs their ids resolved
/// once; missing either is a content error surfaced at load.
const GRASS: &str = "grass";
const DIRT: &str = "dirt";

/// Biome feature size, in world tiles: roughly the lattice spacing of the noise,
/// so a biome blob spans a couple of these. ~20 tiles ≈ a bit over one zone, so a
/// region (256 tiles) holds a handful of biomes.
const BIOME_SCALE: f64 = 20.0;

/// Noise cutoff splitting the two biomes. Value noise is centred near 0.5, so
/// this gives a roughly even grass/dirt split.
const BIOME_THRESHOLD: f64 = 0.5;

/// A loaded content corpus plus the tile ids worldgen paints with. Built once at
/// startup and shared (read-only) across every zone seed.
pub struct Worldgen {
    bundle: Bundle,
    grass: u16,
    dirt: u16,
}

impl Worldgen {
    /// Load the content tree under `content_dir` and resolve the tile ids. Errors
    /// (as a single joined string) if the corpus won't parse or is missing a tile
    /// worldgen needs.
    pub fn load(content_dir: &Path) -> Result<Worldgen, String> {
        let sources = resonantdust_dsl::content::read_content_dir(content_dir)
            .map_err(|e| format!("read content {}: {e}", content_dir.display()))?;
        Self::from_sources(&sources)
    }

    /// Build worldgen from already-read `(name, source)` pairs — the core of
    /// [`load`], split out so it's testable without touching the filesystem.
    pub fn from_sources(sources: &[(String, String)]) -> Result<Worldgen, String> {
        let bundle = resonantdust_dsl::load(sources)
            .map_err(|errs| errs.iter().map(|e| format!("{}: {}", e.file, e.message)).collect::<Vec<_>>().join("; "))?;
        let id = |name: &str| {
            bundle
                .tile_def_id(name)
                .ok_or_else(|| format!("content defines no tile {name:?}"))
        };
        let grass = id(GRASS)?;
        let dirt = id(DIRT)?;
        Ok(Worldgen { bundle, grass, dirt })
    }

    /// The loaded corpus, for callers that want other tile queries.
    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    /// The full 16×16 packed terrain for a fresh zone — `ZONE_TILES` slots,
    /// row-major (`cell(x, y)`). Each cell samples the biome noise at its world
    /// position and lands in grass or dirt by [`BIOME_THRESHOLD`]. Purely a
    /// function of world coordinates, so it's deterministic (a re-seed reproduces
    /// the zone) and seamless across zone boundaries.
    pub fn zone_tiles(&self, zone_id: u32) -> Vec<u16> {
        let grass = pack_tile(self.grass, 0);
        let dirt = pack_tile(self.dirt, 0);
        let (ox, oy) = zone_world_origin(zone_id);
        let mut tiles = vec![0u16; ZONE_TILES];
        for y in 0..ZONE_DIM {
            for x in 0..ZONE_DIM {
                let wx = (ox + x as i32) as f64 / BIOME_SCALE;
                let wy = (oy + y as i32) as f64 / BIOME_SCALE;
                let slot = if value_noise(wx, wy) >= BIOME_THRESHOLD { grass } else { dirt };
                tiles[cell(x, y) as usize] = slot;
            }
        }
        tiles
    }
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

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use resonantdust_codec::packed::{pack_zone_id, tile_def};

    /// Hermetic worldgen over a grass/dirt corpus shaped like the real
    /// content/{data,visual} split — no filesystem, so the test runs anywhere.
    fn worldgen() -> Worldgen {
        let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n  ::dirt>\n    :data>\n      @define>\n        0 return\n";
        let visual = "<tile>\n  ::grass>\n    :visual>\n      @define>\n        #4b573e &visual.color.bg set\n        0 return\n  ::dirt>\n    :visual>\n      @define>\n        #7d6144 &visual.color.bg set\n        0 return\n";
        Worldgen::from_sources(&[
            ("data/tiles.rd".into(), data.into()),
            ("visual/tiles.rd".into(), visual.into()),
        ])
        .expect("load content")
    }

    #[test]
    fn resolves_grass_and_dirt_ids() {
        let w = worldgen();
        // first-appearance order in content/data: grass=1, dirt=2.
        assert_eq!(w.grass, 1);
        assert_eq!(w.dirt, 2);
    }

    #[test]
    fn every_cell_is_grass_or_dirt() {
        let w = worldgen();
        let tiles = w.zone_tiles(0);
        assert_eq!(tiles.len(), ZONE_TILES);
        for &slot in &tiles {
            let def = tile_def(slot);
            assert!(def == w.grass || def == w.dirt, "unexpected tile def {def}");
        }
    }

    #[test]
    fn both_biomes_appear_across_region_00() {
        // A single zone can sit entirely in one biome, but a whole region
        // (16×16 zones) must show both grass and dirt.
        let w = worldgen();
        let mut saw_grass = false;
        let mut saw_dirt = false;
        for zy in 0..REGION_DIM {
            for zx in 0..REGION_DIM {
                let zone_id = pack_zone_id(0, 0, 0, zx, zy);
                for &slot in &w.zone_tiles(zone_id) {
                    saw_grass |= tile_def(slot) == w.grass;
                    saw_dirt |= tile_def(slot) == w.dirt;
                }
            }
        }
        assert!(saw_grass && saw_dirt, "region should contain both biomes");
    }

    #[test]
    fn deterministic_reseed() {
        let w = worldgen();
        let zone_id = pack_zone_id(0, 0, 0, 3, 5);
        assert_eq!(w.zone_tiles(zone_id), w.zone_tiles(zone_id));
    }

    #[test]
    fn seamless_across_zone_boundary() {
        // The east edge of a zone and the west edge of its neighbour read
        // adjacent world columns, so the biome must not break at the seam: the
        // noise is continuous and both columns sample the same world function.
        let w = worldgen();
        let west = w.zone_tiles(pack_zone_id(0, 0, 0, 0, 0));
        let east = w.zone_tiles(pack_zone_id(0, 0, 0, 1, 0));
        // Reconstruct the world-space split at the seam (world x = 15 vs 16) and
        // confirm the packed tiles match that, on both sides, for every row.
        for y in 0..ZONE_DIM {
            let pick = |wx: i32| {
                if value_noise(wx as f64 / BIOME_SCALE, y as f64 / BIOME_SCALE) >= BIOME_THRESHOLD {
                    w.grass
                } else {
                    w.dirt
                }
            };
            assert_eq!(tile_def(west[cell(ZONE_DIM - 1, y) as usize]), pick(15));
            assert_eq!(tile_def(east[cell(0, y) as usize]), pick(16));
        }
    }
}
