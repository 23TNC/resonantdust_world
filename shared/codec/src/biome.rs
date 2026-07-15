//! Biome-dimension noise + classification — the shared, content-free half of
//! worldgen.
//!
//! The edge's DSL worldgen samples three biome DIMENSIONS (temperature / humidity /
//! elevation) from noise at a cell's world position and lets the biome corpus
//! classify it. That noise is pure math (no content runtime), so it lives here where
//! anything can reach it — in particular the `npc` bot, which drives wolves and needs
//! to know which zones are FOREST without loading the DSL. The edge's `worldgen.rs`
//! samples through these same functions, so the bot's classification and the server's
//! generation agree by construction on the noise. Only the forest PREDICATE
//! ([`is_forest`]) mirrors the ordered biome cascade in `content/biome/biomes.rd` and
//! must be kept in step with it — there is no way to evaluate the DSL here.

use crate::packed::ZONE_DIM;

/// Feature size of each biome-dimension noise field, in world tiles. Distinct per
/// dimension so temperature, humidity and elevation vary at different scales.
const TEMPERATURE_SCALE: f64 = 48.0;
const HUMIDITY_SCALE: f64 = 34.0;
const ELEVATION_SCALE: f64 = 26.0;

/// Per-dimension lattice offsets — each field samples a different region of the hash
/// lattice, so the three dimensions are independent rather than correlated copies.
const TEMPERATURE_OFFSET: f64 = 131.0;
const HUMIDITY_OFFSET: f64 = 517.0;
const ELEVATION_OFFSET: f64 = 911.0;

/// The three biome dimensions at a world tile — temperature, humidity, elevation,
/// each in `[0, 1)`. Independent noise fields; a cell's climate is a point in that
/// 3-space the biomes partition.
pub fn biome_dims(wx: i32, wy: i32) -> [f64; 3] {
    let sample =
        |scale: f64, off: f64| value_noise(wx as f64 / scale + off, wy as f64 / scale + off);
    [
        sample(TEMPERATURE_SCALE, TEMPERATURE_OFFSET),
        sample(HUMIDITY_SCALE, HUMIDITY_OFFSET),
        sample(ELEVATION_SCALE, ELEVATION_OFFSET),
    ]
}

/// A per-tile RNG seed from world coordinates — what the DSL's `^rand` salts and
/// finalizes. Deterministic in `(wx, wy)`, so a cell's scatter reproduces on a re-seed.
pub fn tile_seed(wx: i32, wy: i32) -> u64 {
    let mut h = (wx as u32 as u64) | ((wy as u32 as u64) << 32);
    h ^= h >> 33;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 29;
    h
}

/// World-tile coordinate of a zone's top-left cell — the geographic realm ⊃ region ⊃ zone
/// nesting (each 16 per axis). Equals `global_tile(zone_id, 0)`.
pub fn zone_world_origin(zone_id: u32) -> (i32, i32) {
    crate::packed::global_tile(zone_id, 0)
}

/// Whether a cell with these biome `dims` (`[temperature, humidity, elevation]`)
/// classifies as **forest**, mirroring the ordered cascade in
/// `content/biome/biomes.rd`: forest is reached only after the earlier bands fail.
///
/// **Keep in step with `biomes.rd`** — this is the one place the DSL rule is
/// duplicated (the bot can't run the DSL). A cell is forest iff it is none of
/// ocean/beach/mountains/wetland/desert AND matches the forest climate window.
pub fn is_forest(dims: [f64; 3]) -> bool {
    let [temp, humidity, elevation] = dims;
    // Earlier, higher-priority bands that pre-empt forest (first-match-wins):
    let ocean_or_beach = elevation < 0.38; // ocean <0.32 ⊂ beach <0.38
    let mountains = elevation >= 0.82;
    let wetland = humidity >= 0.60 && elevation < 0.46;
    let desert = temp >= 0.68 && humidity < 0.32;
    if ocean_or_beach || mountains || wetland || desert {
        return false;
    }
    // The forest climate window itself.
    humidity >= 0.55 && temp >= 0.35 && temp <= 0.80
}

/// Whether `zone_id` is a **forest zone** — sampled at the zone's centre cell. Biome
/// blobs span more world tiles than a zone is wide (16), so the centre is
/// representative; a forest zone is where the npc drops a wolf pack.
pub fn zone_is_forest(zone_id: u32) -> bool {
    let (ox, oy) = zone_world_origin(zone_id);
    let half = (ZONE_DIM / 2) as i32;
    is_forest(biome_dims(ox + half, oy + half))
}

// ----- noise internals (the sampler the edge's worldgen shares) -----

/// Smooth value noise in `[0, 1)` at a continuous `(x, y)`: hash the four integer
/// lattice corners and smoothstep-interpolate between them.
fn value_noise(x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (ix, iy) = (x0 as i32, y0 as i32);
    let sx = smoothstep(x - x0);
    let sy = smoothstep(y - y0);
    let nx0 = lerp(lattice(ix, iy), lattice(ix + 1, iy), sx);
    let nx1 = lerp(lattice(ix, iy + 1), lattice(ix + 1, iy + 1), sx);
    lerp(nx0, nx1, sy)
}

/// Deterministic pseudo-random value in `[0, 1)` for an integer lattice point.
fn lattice(x: i32, y: i32) -> f64 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h as f64 / (u32::MAX as f64 + 1.0)
}

/// Hermite smoothstep `3t² − 2t³`.
fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_forest_matches_the_cascade_window() {
        // In-window, temperate & humid, mid elevation → forest.
        assert!(is_forest([0.5, 0.6, 0.5]));
        // Too dry (below the humidity floor) → not forest (falls to plains).
        assert!(!is_forest([0.5, 0.50, 0.5]));
        // Under water / beach elevation → pre-empted by ocean/beach.
        assert!(!is_forest([0.5, 0.6, 0.30]));
        // High elevation → mountains.
        assert!(!is_forest([0.5, 0.6, 0.90]));
        // Very humid + low elevation → wetland pre-empts.
        assert!(!is_forest([0.5, 0.65, 0.40]));
        // Hot + arid → desert pre-empts.
        assert!(!is_forest([0.7, 0.20, 0.5]));
        // Too hot for the forest window (temp > 0.80).
        assert!(!is_forest([0.85, 0.6, 0.5]));
    }

    #[test]
    fn zone_is_forest_is_deterministic() {
        let z = 0x0000_0033;
        assert_eq!(zone_is_forest(z), zone_is_forest(z));
    }
}
