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

/// A grid pathability probe: `true` = a pawn may occupy that tile.
pub type Pathable<'a> = &'a dyn Fn(i32, i32) -> bool;

/// **THE pace** — a pawn's derived `ground_speed` in tics per tile (lower is faster).
///
/// It belongs here, beside the walk that consumes it, because it was the second mirrored pair in
/// this system: the worker had `ground_speed_tics` and the wasm bundle had `pawn_ground_speed`,
/// each assembling the same trait rows and calling the same `stat_eval` in its own words. They
/// happened to agree — but "happened to" is the whole problem, and a pace that drifts is
/// indistinguishable from a walk that drifts until you split the measurement by kind.
///
/// `kind` is the content object id; `payload` the pawn's raw opcode stream; `needs` its
/// `(row, set_tic)` sub-table rows. Returns `0.0` when the pawn derives no ground speed at all —
/// callers must treat that as "cannot ground-move", never as "use a default": a wrong default is
/// an 8x error on the pawns it is wrong about.
pub fn ground_speed(
    bundle: &crate::loader::Bundle,
    kind: u16,
    payload: &[u32],
    needs: &[(u64, u16)],
    now: u16,
) -> f64 {
    let raw_traits = resonantdust_codec::payload::payload_traits(payload);
    let conditions = resonantdust_codec::payload::payload_conditions(payload);
    let traits = bundle.object_trait_rows(kind, &raw_traits);
    let active = crate::needs_eval::active_conditions(bundle, &traits, needs, &conditions, now);
    crate::stat_eval::stat_value(bundle, "ground_speed", &traits, &active)
}

/// The content `kind` id carried by a `definition_reference`. A CREATE-minted pawn packs
/// `TYPE | species | kind | variant` (a nonzero high half tells it apart); a legacy raw
/// object-id def is the id itself.
pub fn def_kind_id(definition_reference: u32) -> u16 {
    if definition_reference >> 16 != 0 {
        ((definition_reference & 0xffff) >> 4) as u16
    } else {
        definition_reference as u16
    }
}

/// The tile a fractional world point sits in. The half-open edge rule (chord-movement I2):
/// subtile 0 belongs to the tile, so this is a floor — matching `codec::object::position_to_tile`
/// on any point that came from a `position_reference`.
pub fn tile_of(point: (f64, f64)) -> (i32, i32) {
    (point.0.floor() as i32, point.1.floor() as i32)
}

/// One hop's outcome: where the pawn lands, which way it ends up facing, and what the hop costs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hop {
    pub point: (f64, f64),
    /// `0` south, `1` east, `2` north, `3` west — the rotation encoding pawns store.
    pub facing: u8,
    /// Tics until the landing, `ceil(distance × pace)`. Never 0: a hop that landed on its own
    /// tic would make the continuation collide with the event that queued it.
    pub tics: u16,
}

/// The first chord's leg from `from` toward `dest_tile`, and the pathable fraction of it.
///
/// The chord's line-of-sight was validated on the LATTICE (tile centres); the pawn walks from its
/// SUBTILE point, so the real segment can clip a corner the validated one did not — hence the
/// separate [`crate::path_eval::clear_point_fraction`] on the actual segment. `None` = no route
/// this hop; the caller steps nothing and re-routes next time.
pub fn first_leg(
    from: (f64, f64),
    dest_tile: (i32, i32),
    pathable: Pathable,
) -> Option<((f64, f64), f64)> {
    let chords = crate::path_eval::find_chords(tile_of(from), dest_tile, 0, pathable)?;
    let &(wx, wy) = chords.first()?;
    let leg = (wx as f64 - from.0, wy as f64 - from.1);
    let clear = crate::path_eval::clear_point_fraction(from, (wx as f64, wy as f64), pathable);
    Some((leg, clear))
}

/// Facing from a movement delta, east/west dominant on a tie — the rule the worker's hop and the
/// client's initial aim both need, in one place so a pawn cannot face two ways at once.
pub fn facing_of(dx: f64, dy: f64) -> u8 {
    if dx.abs() >= dy.abs() {
        if dx > 0.0 { 1 } else { 3 }
    } else if dy > 0.0 { 0 } else { 2 }
}

/// **THE hop** (worker `MOVE_STEP`, chord-movement F2/F3/F8): from `from`, one stride along the
/// first chord toward `dest`.
///
/// `None` means step nothing — either the route is blocked this hop (the world changed mid-trip;
/// the chain re-queues and resumes when it clears) or the leg is degenerate.
///
/// Landing on the destination TILE returns `dest` exactly rather than a point fractionally short
/// of it: a stride-capped hop otherwise leaves a pawn permanently a sixteenth from its goal, and
/// arrival is an equality test.
pub fn next_hop(
    from: (f64, f64),
    dest: (f64, f64),
    pace: f64,
    pathable: Pathable,
) -> Option<Hop> {
    let dest_tile = tile_of(dest);
    if tile_of(from) == dest_tile {
        let (dx, dy) = (dest.0 - from.0, dest.1 - from.1);
        return Some(Hop { point: dest, facing: facing_of(dx, dy), tics: hop_tics(dx.hypot(dy), pace) });
    }
    let ((mut dx, mut dy), clear) = first_leg(from, dest_tile, pathable)?;
    // A fully-blocked direct segment RECENTERS: step toward this tile's own lattice anchor. A
    // segment inside one tile is always legal, and from the anchor the validated corridor is
    // exact. (Without this a hop near a shoreline floors a tile into water — seen live.)
    if clear <= f64::EPSILON {
        let (tx, ty) = tile_of(from);
        dx = tx as f64 - from.0;
        dy = ty as f64 - from.1;
    }
    let len = dx.hypot(dy);
    if len <= f64::EPSILON {
        return None;
    }
    let mut f = (hop_stride_tiles(pace) / len).min(1.0);
    if clear > f64::EPSILON {
        f = f.min(clear);
    }
    Some(Hop {
        point: (from.0 + dx * f, from.1 + dy * f),
        facing: facing_of(dx, dy),
        tics: hop_tics(len * f, pace),
    })
}

/// A hop's tic cost: `ceil(distance × pace)`, floored at 1 so a continuation never lands on the
/// tic that queued it.
fn hop_tics(distance: f64, pace: f64) -> u16 {
    ((distance * pace).ceil() as i64).clamp(1, u16::MAX as i64) as u16
}

/// **WHERE A WALKING PAWN IS** at `now` (worker `resolve_walk_position_for`, and the function the
/// TypeScript speculation was independently re-deriving — shared-simulation's whole point).
///
/// Interpolates along the first chord from the pawn's last authoritative point at the tic that
/// point was stamped. `base_tic` in the FUTURE reads as elapsed 0, never as ancient — the wrapping
/// u16 tic ring makes "slightly ahead" and "half a ring behind" the same bits, and the needs-eval
/// rule resolves that ambiguity toward zero everywhere.
///
/// Returns `from` unchanged when the pawn is not really walking: no route, already on the
/// destination tile, a degenerate pace, or a leg the clamp closes to nothing.
pub fn position_at(
    from: (f64, f64),
    dest: (f64, f64),
    base_tic: u16,
    now: u16,
    pace: f64,
    pathable: Pathable,
) -> (f64, f64) {
    let dest_tile = tile_of(dest);
    if tile_of(from) == dest_tile || pace < 1.0 {
        return from;
    }
    let Some((leg, clear)) = first_leg(from, dest_tile, pathable) else { return from };
    advance_along(from, leg, clear, base_tic, now, pace)
}

/// Walk `from` along a PRE-COMPUTED leg for the time elapsed since `base_tic`.
///
/// The tail of [`position_at`], split out so a consumer that already knows the leg — the client's
/// mover track computes it once when an intent arms, rather than re-pathfinding every frame — gets
/// bit-identical motion instead of an "equivalent" reimplementation. That distinction is the
/// entire subject of this module: the cheap version and the correct version must be the same code.
///
/// `clear` is the pathable fraction of the leg (`1.0` = wholly clear).
pub fn advance_along(
    from: (f64, f64),
    leg: (f64, f64),
    clear: f64,
    base_tic: u16,
    now: u16,
    pace: f64,
) -> (f64, f64) {
    let len = leg.0.hypot(leg.1);
    if len <= f64::EPSILON || pace < 1.0 {
        return from;
    }
    // A future `base_tic` reads as elapsed ZERO, never as most of a ring: on the wrapping u16 the
    // two are the same bits, and guessing wrong teleports the pawn (the needs-eval rule).
    let raw = now.wrapping_sub(base_tic);
    let elapsed = if raw > u16::MAX / 2 { 0 } else { raw } as f64;
    let f = (elapsed / pace / len).min(1.0).min(clear);
    if f <= f64::EPSILON {
        return from;
    }
    (from.0 + leg.0 * f, from.1 + leg.1 * f)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An open field — every tile walkable.
    fn open(_x: i32, _y: i32) -> bool {
        true
    }

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

    /// THE property the two functions exist to share: sampling the walk at the hop's own tic must
    /// land exactly where the hop lands. If these ever disagree, an observer interpolating and a
    /// server stepping are back to being two implementations — which is the bug this module was
    /// created to delete.
    #[test]
    fn position_at_the_hop_tic_equals_the_hop_landing() {
        let cases: &[((f64, f64), (f64, f64), f64)] = &[
            ((10.0, 10.0), (20.0, 10.0), 24.0),   // due east, bunny pace
            ((10.0, 10.0), (10.0, 20.0), 12.0),   // due south, wolf pace
            ((10.0, 10.0), (18.0, 18.0), 24.0),   // pure diagonal
            ((10.5, 10.25), (17.0, 13.0), 12.0),  // off-lattice start
            ((10.0, 10.0), (40.0, 11.0), 6.0),    // long trip at the stride cap
            ((10.0, 10.0), (11.0, 10.0), 24.0),   // one tile
        ];
        for &(from, dest, pace) in cases {
            let hop = next_hop(from, dest, pace, &open).expect("open field always routes");
            let at = position_at(from, dest, 0, hop.tics, pace, &open);
            let err = (at.0 - hop.point.0).hypot(at.1 - hop.point.1);
            assert!(
                err < 0.05,
                "from {from:?} dest {dest:?} pace {pace}: hop lands {:?} but position_at({}) says {at:?} ({err} apart)",
                hop.point, hop.tics,
            );
        }
    }

    /// The pace contract, stated as distance over time rather than as a stride: a pawn covers one
    /// tile per `pace` tics. This is the number the client speculates on, so it is the number that
    /// must hold — a stride that is right but paced wrong looks identical until the anchor lands.
    #[test]
    fn a_walk_covers_one_tile_per_pace_tics() {
        for pace in [6.0, 12.0, 24.0] {
            for tics in [8_u16, 16, 24, 32] {
                let at = position_at((10.0, 10.0), (60.0, 10.0), 0, tics, pace, &open);
                let expected = 10.0 + f64::from(tics) / pace;
                assert!(
                    (at.0 - expected).abs() < 1e-9,
                    "pace {pace}, {tics} tics: walked to {} not {expected}", at.0,
                );
            }
        }
    }

    /// A hop never overshoots its destination, and arriving is exact — an equality test decides
    /// arrival, so a landing a sixteenth short would strand the pawn a hop from every goal.
    #[test]
    fn arrival_is_exact_and_never_overshoots() {
        let hop = next_hop((10.0, 10.0), (10.75, 10.25), 24.0, &open).unwrap();
        assert_eq!(hop.point, (10.75, 10.25));
        assert!(hop.tics >= 1);
        // Sampled past the end of the trip, the walk clamps AT the waypoint, never beyond.
        let at = position_at((10.0, 10.0), (12.0, 10.0), 0, 60_000, 24.0, &open);
        assert!(at.0 <= 12.0 + 1e-9, "walked to {} past the destination", at.0);
    }

    /// A future-stamped base tic reads as elapsed zero, not as almost a whole ring of travel.
    /// The wrapping u16 makes those two cases the same bits; guessing wrong teleports a pawn.
    #[test]
    fn a_future_base_tic_reads_as_no_elapsed_time() {
        let at = position_at((10.0, 10.0), (30.0, 10.0), 100, 90, 24.0, &open);
        assert_eq!(at, (10.0, 10.0));
        // And the ring wrap itself is ordinary elapsed time, not a jump.
        let at = position_at((10.0, 10.0), (30.0, 10.0), u16::MAX - 3, 4, 24.0, &open);
        assert!((at.0 - (10.0 + 8.0 / 24.0)).abs() < 1e-9, "wrapped walk went to {}", at.0);
    }

    /// A pace below 1 is degenerate (`can_move_ground` refuses such a pawn at the front door);
    /// the walk must hold position rather than divide its way to infinity.
    #[test]
    fn a_degenerate_pace_does_not_move_the_pawn() {
        assert_eq!(position_at((10.0, 10.0), (30.0, 10.0), 0, 500, 0.0, &open), (10.0, 10.0));
    }

    /// Leaving an impathable cell is always legal (`path_eval` F6 — the start cell is never
    /// probed), including from a point off its centre. A pawn that ended up in water walks out.
    ///
    /// This began as an attempt to reach the RECENTER branch and instead showed why that branch
    /// is hard to reach: `clear_point_fraction` does not count the START cell, so standing inside
    /// a wall does NOT drive `clear` to 0 — the hop leaves eastward, correctly. The recenter
    /// therefore needs a segment whose first crossed cell is blocked while the lattice corridor
    /// is not, and no test reaches it today. See [I10].
    #[test]
    fn a_pawn_inside_a_wall_can_still_walk_out() {
        // The pawn is STANDING IN a blocked cell — legal, and the case this exists for (a pawn
        // that ended up in water). Leaving an impathable cell is always allowed (path_eval F6),
        // so a route exists; but every segment OUT of it starts inside the wall, so
        // `clear_point_fraction` is 0 and the direct step is refused.
        let world = |x: i32, y: i32| !(x == 11 && y != 10);
        let from = (11.5, 11.5); // inside the wall column, off-centre
        let hop = next_hop(from, (14.0, 10.0), 24.0, &world).expect("leaving a wall is legal");
        assert!(hop.point.0 > from.0, "did not leave the wall eastward: {:?}", hop.point);
        assert!(hop.point.0 <= 14.0, "overshot the destination: {:?}", hop.point);
    }

    /// The sub-1 `clear` CLAMP: the segment is partly legal, so the hop lands on the pathable
    /// prefix rather than the full stride. Also previously unreachable from any test.
    #[test]
    fn a_partly_blocked_segment_lands_on_its_clear_prefix() {
        // Open everywhere except a wall far along the eastward line.
        let world = |x: i32, _y: i32| x < 14;
        let from = (10.0, 10.0);
        let far = next_hop(from, (13.0, 10.0), 6.0, &world).expect("a route exists");
        // pace 6 => stride 5.33 tiles, which would carry it to 15.33 — past the wall at x=14.
        assert!(far.point.0 < 14.0, "walked into the wall at x=14: {:?}", far.point);
        assert!(far.point.0 > from.0, "did not move at all: {:?}", far.point);
    }

    /// An impathable world routes nowhere: the hop reports "step nothing" rather than guessing.
    #[test]
    fn a_blocked_world_yields_no_hop() {
        let walled = |x: i32, _y: i32| x <= 10;
        assert!(next_hop((10.0, 10.0), (30.0, 10.0), 24.0, &walled).is_none());
        assert_eq!(position_at((10.0, 10.0), (30.0, 10.0), 0, 32, 24.0, &walled), (10.0, 10.0));
    }
}
