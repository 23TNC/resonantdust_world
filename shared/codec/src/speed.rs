//! Movement pace — the degenerate-fallback DEFAULT, nothing more. **Pace is the DERIVED
//! `ground_speed` stat in TICS PER TILE** (input-rework F8, superseding the authored
//! per-kind `speed` field): a pawn's `walks` level contributes its authored tics/tile
//! through `shared/content::stat_eval`, and every consumer — the worker's continuation
//! spacing, the client's speculation rate, the npc's trip deadline — derives the SAME value
//! from the pawn's rows + corpus. This module holds only what codec may own (codec is
//! wire/math, never content): the fallback for a degenerate 0-derived pace, which the
//! `can_move_ground` gate makes unreachable for real orders. Accepted consequence: a
//! [`crate::tic::TIC_HZ`] change changes wall-clock movement speed — the game's time unit
//! IS the tic.

/// Tics one tile-hop takes when the derived pace is degenerate (< 1) — pinned in TICS (not
/// derived from `TIC_HZ`: deriving would smuggle wall-time authoring back in through the
/// default). 3 tics = 0.5 s/tile at 6 Hz. A hop can never take zero tics (the continuation
/// would land on its own tic).
pub const DEFAULT_TICS_PER_TILE: u16 = 3;
