//! Movement speed — the DEFAULT + resolution rule, nothing more. **Speed is CONTENT, authored
//! in TICS PER TILE** (user, 2026-07-28 — supersedes first-pawns F7's wall-time authoring):
//! each kind's `:data` facet authors `speed` in the DSL corpus (wolf = 12 → 2 s/tile at 6 Hz),
//! and every consumer — the worker's continuation spacing, the client's speculation rate, the
//! npc's trip deadline — resolves the SAME per-kind value through the corpus/bundle. This
//! module holds only what codec may own (codec is wire/math, never content): the default for
//! unauthored kinds and [`resolve`]. Accepted consequence: a [`crate::tic::TIC_HZ`] change
//! changes wall-clock movement speed — the game's time unit IS the tic.

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
