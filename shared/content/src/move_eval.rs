//! `move_eval` — THE walk (shared-simulation P1).
//!
//! One implementation, four consumers: the worker steps authoritative hops with it, `client/core`
//! advances its mover track between anchors, the npc reads that track, and the webgl client
//! renders the result. Before this module the rule was written **twice** — privately inside
//! `server/worker/src/main.rs`, and again in TypeScript inside `client/webgl`'s `MoverLayer`
//! under a comment claiming it "mirrored EXACTLY" the worker. It did not: measured live on
//! 2026-08-09, anchors landed 2.67 tiles apart where the client's own pace predicted 1.33, and
//! 116 of 214 reseeds in 54 s blew past the render-chase's give-up threshold. That is the
//! divergence class this module exists to make impossible, exactly as [`crate::path_eval`] does
//! for routes: same inputs → the same walk, on every observer.
//!
//! The rules:
//! - **Hops fire at a CADENCE, not per tile** ([`REANCHOR_TICS`]). Every hop PROMOTEs, so each
//!   write is the drift-bounding anchor and the fan stays ≤ one frame per cadence per moving
//!   pawn — never one per tile.
//! - **A hop's stride is that cadence expressed as DISTANCE** at the pawn's pace
//!   ([`hop_stride_tiles`]), capped by [`CHORD_CAP_TILES`] so a single hop's window stays bounded
//!   however fast the pawn.
//! - **Pace is `tics_per_tile`** — the pawn's DERIVED `ground_speed` (via [`crate::stat_eval`]),
//!   never an authored constant. Lower is faster.

/// The re-anchor cadence in tics (chord-movement F4/F8): hops fire — and PROMOTE — at most this
/// far apart, so observer drift without a start anchor stays bounded.
pub const REANCHOR_TICS: f64 = 32.0;

/// The chord-segment cap in tiles (chord-movement I7): bounds a single hop's window.
pub const CHORD_CAP_TILES: f64 = 8.0;

/// One hop's stride in tiles (chord-movement F8): the re-anchor cadence translated to distance at
/// this pawn's pace, clamped to `[1, CHORD_CAP_TILES]`.
///
/// The `max(1.0)` on the pace is a divide-by-zero guard, not a policy: a pawn deriving `0` cannot
/// ground-move at all and is refused upstream by `can_move_ground`, so this only ever sees a pace
/// that already passed that gate.
pub fn hop_stride_tiles(tics_per_tile: f64) -> f64 {
    (REANCHOR_TICS / tics_per_tile.max(1.0)).clamp(1.0, CHORD_CAP_TILES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both clamp ends, which is where a stride stops being a division and starts being a policy.
    #[test]
    fn stride_clamps_at_both_ends() {
        // Slower than one tile per cadence: the floor keeps a hop a real step, never a nudge.
        assert_eq!(hop_stride_tiles(240.0), 1.0);
        assert_eq!(hop_stride_tiles(REANCHOR_TICS), 1.0);
        // Faster than the cap: the window is bounded however quick the pawn.
        assert_eq!(hop_stride_tiles(1.0), CHORD_CAP_TILES);
        assert_eq!(hop_stride_tiles(0.0), CHORD_CAP_TILES); // the divide-by-zero guard
        // The cap's exact boundary — 32/4 = 8 is the fastest pace that is NOT clamped.
        assert_eq!(hop_stride_tiles(4.0), CHORD_CAP_TILES);
    }

    /// The two paces the live corpus actually authors, pinned so a `walks` retune that changes
    /// the fan rate cannot do it silently (`ground_speed add = [24, 12, 6]`).
    #[test]
    fn stride_matches_the_authored_paces() {
        // walks level 1 — the bunny.
        assert!((hop_stride_tiles(24.0) - 4.0 / 3.0).abs() < 1e-12);
        // walks level 2 — the wolf.
        assert!((hop_stride_tiles(12.0) - 8.0 / 3.0).abs() < 1e-12);
        // walks level 3 — the sprint tier, at the cap's edge but not over it.
        assert!((hop_stride_tiles(6.0) - 16.0 / 3.0).abs() < 1e-12);
    }

    /// What the cadence actually promises, stated honestly. A hop costs `stride × pace` tics, and
    /// that is bounded by [`REANCHOR_TICS`] for every pawn EXCEPT one slower than a tile per
    /// cadence — there the one-tile floor wins and the hop necessarily overruns, because a pawn
    /// that would otherwise move a fraction of a tile per anchor must still move a whole one.
    ///
    /// Worth pinning as a test rather than a comment: it is the one case where an anchor gap is
    /// legitimately longer than the cadence, so a divergence probe that assumes 32 tics between
    /// rows will mis-read a slow pawn as a stalled one.
    #[test]
    fn the_cadence_bounds_every_hop_except_a_floored_one() {
        for pace in [1.0, 4.0, 6.0, 12.0, 24.0, REANCHOR_TICS] {
            let tics = hop_stride_tiles(pace) * pace;
            assert!(
                tics <= REANCHOR_TICS + 1e-9,
                "pace {pace} strides {tics} tics, past the {REANCHOR_TICS}-tic cadence",
            );
        }
        // Past the floor, the hop is a whole tile and takes as long as that tile takes.
        for pace in [60.0, 240.0] {
            assert_eq!(hop_stride_tiles(pace), 1.0);
            assert!(hop_stride_tiles(pace) * pace > REANCHOR_TICS);
        }
    }
}
