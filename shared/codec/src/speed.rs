//! Movement speed — the DEFAULT + resolution rule, nothing more. **Speed is CONTENT, authored
//! in TICS PER TILE** (user, 2026-07-28 — supersedes first-pawns F7's wall-time authoring):
//! each kind's `:data` facet authors `speed` in the DSL corpus (wolf = 12 → 2 s/tile at 6 Hz),
//! and every consumer — the worker's continuation spacing, the client's speculation rate, the
//! npc's trip deadline — resolves the SAME per-kind value through the corpus/bundle. This
//! module holds only what codec may own (codec is wire/math, never content): the default for
//! unauthored kinds and [`resolve`]. Accepted consequence: a [`crate::tic::TIC_HZ`] change
//! changes wall-clock movement speed — the game's time unit IS the tic.

/// Tics one tile-hop takes for a kind that authors no `speed` — pinned in TICS (not derived
/// from `TIC_HZ`: deriving would smuggle the retired wall-time authoring back in through the
/// default). 3 tics = 0.5 s/tile at 6 Hz.
pub const DEFAULT_TICS_PER_TILE: u16 = 3;

/// Resolve a kind's authored speed (`corpus.thing_speed(kind)` / the bundle's `thingSpeed`
/// table, where `0`/absent = unauthored) to the tics one hop takes. Clamps to ≥ 1 — a hop can
/// never take zero tics (the continuation would land on its own tic).
pub fn resolve(authored: Option<u16>) -> u16 {
    match authored {
        Some(t) if t > 0 => t,
        _ => DEFAULT_TICS_PER_TILE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_authored_default_and_zero() {
        assert_eq!(resolve(Some(12)), 12); // the wolf
        assert_eq!(resolve(None), DEFAULT_TICS_PER_TILE);
        assert_eq!(resolve(Some(0)), DEFAULT_TICS_PER_TILE); // 0 = unauthored in the bundle table
        assert!(DEFAULT_TICS_PER_TILE >= 1);
    }
}
