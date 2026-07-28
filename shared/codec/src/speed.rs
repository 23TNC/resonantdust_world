//! Movement speed — the ONE seam the worker AND the speculating clients read (first-pawns F7),
//! so the server steps and the client walks at exactly the same rate. Authored in **wall-time**
//! (tiles/second) and converted through [`crate::tic::TIC_HZ`], so a tic-rate change never
//! rescales the world's movement. Per-kind speeds are CONTENT and plumb in behind
//! [`tics_per_tile`] when the corpus reaches the worker (the same gap torch-thing I2 recorded);
//! until then every pawn walks at the default.

use crate::tic::TIC_HZ;

/// The default walk speed, tiles per second (wall-time — the authored unit).
pub const WALK_TILES_PER_SEC: f64 = 2.0;

/// Tics one tile-hop takes for `definition_reference` — `MOVE_TO`'s continuation spacing and
/// the client's speculation rate. Currently the default for every kind (see module docs).
pub fn tics_per_tile(_definition_reference: u32) -> u16 {
    ((TIC_HZ as f64 / WALK_TILES_PER_SEC).ceil() as u16).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hop_is_never_zero_tics() {
        assert!(tics_per_tile(0) >= 1);
        // 2 tiles/sec at 6 Hz = 3 tics/tile = 0.5 s per tile.
        assert_eq!(tics_per_tile(7), 3);
    }
}
