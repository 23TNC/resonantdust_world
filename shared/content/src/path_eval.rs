//! `path_eval` — THE pathfinder (pathfinding F2/F3/F4/F5/F6).
//!
//! One implementation, three consumers: the worker steps authoritative hops with it, the
//! wasm client speculates the SAME detour (or pawns rubber-band — pathfinding I1), and the
//! npc derives trip lengths. A second pathfinder anywhere is the divergence class the
//! one-eval law exists to prevent — determinism here is a CONTRACT: same grid, same
//! endpoints → the same path, on every observer.
//!
//! The rules:
//! - **8-way** grid, every step cost 1 (a diagonal hop spends the same `tics_per_tile`
//!   the chain always has — the Chebyshev metric the greedy `signum` step already walked).
//! - **No corner cutting** (F4): a diagonal is legal only if BOTH orthogonal cells it
//!   clips are pathable.
//! - **The start cell is never probed** (F6): leaving an impathable cell is always legal.
//! - **An impathable destination is `None`** (F5): the caller logs and no-ops.
//! - **Bounded** (I6): [`EXPANSION_CAP`] pops, then `None` — cap exhaustion reads as
//!   unreachable, never a stall.
//!
//! Pathability itself is DERIVED by the caller (F1): the probe closure composes the tile
//! kind's `pathable` with thing occupancy (overlay kind-0 suppresses) however the caller's
//! view holds them — worker mirror, client render state, or a test fixture.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The A* pop bound (I6): an enclosed destination otherwise floods the whole mirrored
/// world per hop, per pawn, at 6 Hz. ~6k cells ≈ a 78×78 disc — generous for zone-scale
/// trips, cheap to exhaust.
pub const EXPANSION_CAP: usize = 6144;

/// The 8 neighbor offsets, FIXED order — part of the determinism contract (ties in the
/// open set resolve by insertion sequence, which this order feeds).
const DIRS: [(i32, i32); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// Chebyshev distance — the admissible heuristic for a diag-cost-1 grid.
fn cheb(a: (i32, i32), b: (i32, i32)) -> u32 {
    ((a.0 - b.0).abs().max((a.1 - b.1).abs())) as u32
}

/// The path from `from` to `to` EXCLUSIVE of `from`, inclusive of `to` — `Some(vec![])`
/// when already there. `None`: destination impathable (F5), unreachable, or the cap
/// exhausted. `pathable(x, y)` answers "may a pawn ENTER (x, y)?" — it is never asked
/// about `from` (F6).
pub fn find_path(
    from: (i32, i32),
    to: (i32, i32),
    pathable: &dyn Fn(i32, i32) -> bool,
) -> Option<Vec<(i32, i32)>> {
    if from == to {
        return Some(Vec::new());
    }
    if !pathable(to.0, to.1) {
        return None;
    }
    // Open set keyed (f, seq): BinaryHeap of Reverse so the SMALLEST f pops first; `seq`
    // (insertion order) breaks f-ties deterministically on every observer.
    let mut open: BinaryHeap<Reverse<(u32, u64, (i32, i32))>> = BinaryHeap::new();
    let mut best_g: HashMap<(i32, i32), u32> = HashMap::new();
    let mut came: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut seq: u64 = 0;
    best_g.insert(from, 0);
    open.push(Reverse((cheb(from, to), seq, from)));
    let mut pops = 0usize;
    while let Some(Reverse((_, _, cur))) = open.pop() {
        pops += 1;
        if pops > EXPANSION_CAP {
            return None; // I6: cap exhaustion = unreachable
        }
        if cur == to {
            let mut path = vec![cur];
            let mut at = cur;
            while let Some(&prev) = came.get(&at) {
                if prev == from {
                    break;
                }
                path.push(prev);
                at = prev;
            }
            path.reverse();
            return Some(path);
        }
        let g = best_g[&cur];
        for (dx, dy) in DIRS {
            let next = (cur.0 + dx, cur.1 + dy);
            if !pathable(next.0, next.1) {
                continue;
            }
            // F4: a diagonal may not clip an impathable corner — BOTH orthogonals must be
            // open. The START cell counts as open here (F6: the pawn is standing on it).
            if dx != 0 && dy != 0 {
                let ok = |x: i32, y: i32| (x, y) == from || pathable(x, y);
                if !ok(cur.0 + dx, cur.1) || !ok(cur.0, cur.1 + dy) {
                    continue;
                }
            }
            let ng = g + 1;
            if best_g.get(&next).is_none_or(|&old| ng < old) {
                best_g.insert(next, ng);
                came.insert(next, cur);
                seq += 1;
                open.push(Reverse((ng + cheb(next, to), seq, next)));
            }
        }
    }
    None
}

/// The FIRST hop of the shared path (F3 — what a stateless `MOVE_STEP` recompute asks
/// for), or `None` when F5 says no-op.
pub fn next_step(
    from: (i32, i32),
    to: (i32, i32),
    pathable: &dyn Fn(i32, i32) -> bool,
) -> Option<(i32, i32)> {
    find_path(from, to, pathable).and_then(|p| p.first().copied())
}

/// The path's hop count (the npc's trip estimate — I7), or `None` when F5 says no-op.
pub fn path_len(
    from: (i32, i32),
    to: (i32, i32),
    pathable: &dyn Fn(i32, i32) -> bool,
) -> Option<usize> {
    find_path(from, to, pathable).map(|p| p.len())
}

// ── chords (chord-movement F5/F7) ───────────────────────────────────────────────────────
//
// The route a pawn WALKS is the string-pulled polyline: octile A* (integer ×5/×7 costs —
// 7/5 approximates √2 with no floats in the heap) finds a geodesic-hugging tile path, then
// greedy line-of-sight smoothing collapses it to the MINIMAL chord sequence. Waypoints are
// TILE coordinates (a chord connects tile CENTERS); the route is integer-exact on every
// observer — only chord LENGTHS (IEEE hypot, correctly rounded) and per-observer progress
// are floats.

/// Octile step costs: straight ×5, diagonal ×7.
const COST_STRAIGHT: u32 = 5;
const COST_DIAG: u32 = 7;

/// The octile heuristic in ×5/×7 units — admissible for [`COST_STRAIGHT`]/[`COST_DIAG`].
fn octile_h(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    COST_STRAIGHT * (hi - lo) + COST_DIAG * lo
}

/// Octile A* — [`find_path`]'s twin with distance-true costs, so the tile path hugs the
/// true geodesic and string-pulls to minimal chords. Same rules: exclusive of `from`,
/// F4 corner law, F5 refusals, F6 start, [`EXPANSION_CAP`], seq-tie determinism.
pub fn find_path_octile(
    from: (i32, i32),
    to: (i32, i32),
    pathable: &dyn Fn(i32, i32) -> bool,
) -> Option<Vec<(i32, i32)>> {
    if from == to {
        return Some(Vec::new());
    }
    if !pathable(to.0, to.1) {
        return None;
    }
    let mut open: BinaryHeap<Reverse<(u32, u64, (i32, i32))>> = BinaryHeap::new();
    let mut best_g: HashMap<(i32, i32), u32> = HashMap::new();
    let mut came: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut seq: u64 = 0;
    best_g.insert(from, 0);
    open.push(Reverse((octile_h(from, to), seq, from)));
    let mut pops = 0usize;
    while let Some(Reverse((_, _, cur))) = open.pop() {
        pops += 1;
        if pops > EXPANSION_CAP {
            return None;
        }
        if cur == to {
            let mut path = vec![cur];
            let mut at = cur;
            while let Some(&prev) = came.get(&at) {
                if prev == from {
                    break;
                }
                path.push(prev);
                at = prev;
            }
            path.reverse();
            return Some(path);
        }
        let g = best_g[&cur];
        for (dx, dy) in DIRS {
            let next = (cur.0 + dx, cur.1 + dy);
            if !pathable(next.0, next.1) {
                continue;
            }
            if dx != 0 && dy != 0 {
                let ok = |x: i32, y: i32| (x, y) == from || pathable(x, y);
                if !ok(cur.0 + dx, cur.1) || !ok(cur.0, cur.1 + dy) {
                    continue;
                }
            }
            let step = if dx != 0 && dy != 0 { COST_DIAG } else { COST_STRAIGHT };
            let ng = g + step;
            if best_g.get(&next).is_none_or(|&old| ng < old) {
                best_g.insert(next, ng);
                came.insert(next, cur);
                seq += 1;
                open.push(Reverse((ng + octile_h(next, to), seq, next)));
            }
        }
    }
    None
}

/// Line of sight between the CENTERS of tiles `a` and `b`: the segment's SUPERCOVER —
/// every tile it passes through, inflated by `radius` (Chebyshev half-width in tiles;
/// 0 = the 1×1 footprint — F7) — must be pathable. An EXACT corner crossing requires
/// both flanking cells open (the F4 no-clip law, generalized). The `a` cell itself is
/// never probed (F6 — leaving is always legal). Integer throughout (Dedu supercover).
pub fn line_of_sight(
    a: (i32, i32),
    b: (i32, i32),
    radius: i32,
    pathable: &dyn Fn(i32, i32) -> bool,
) -> bool {
    let clear = |x: i32, y: i32| -> bool {
        if (x, y) == a {
            return true;
        }
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let (cx, cy) = (x + ox, y + oy);
                if (cx, cy) != a && !pathable(cx, cy) {
                    return false;
                }
            }
        }
        true
    };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (xstep, ystep) = (dx.signum(), dy.signum());
    let (adx, ady) = (dx.abs(), dy.abs());
    let (ddx, ddy) = (2 * adx, 2 * ady);
    let (mut x, mut y) = a;
    if !clear(x, y) {
        return false;
    }
    if ddx >= ddy {
        let mut err = adx;
        let mut errprev = adx;
        for _ in 0..adx {
            x += xstep;
            err += ddy;
            if err > ddx {
                y += ystep;
                err -= ddx;
                match (err + errprev).cmp(&ddx) {
                    std::cmp::Ordering::Less => {
                        if !clear(x, y - ystep) {
                            return false;
                        }
                    }
                    std::cmp::Ordering::Greater => {
                        if !clear(x - xstep, y) {
                            return false;
                        }
                    }
                    std::cmp::Ordering::Equal => {
                        // exact corner: BOTH flanks must be open (F4)
                        if !clear(x, y - ystep) || !clear(x - xstep, y) {
                            return false;
                        }
                    }
                }
            }
            if !clear(x, y) {
                return false;
            }
            errprev = err;
        }
    } else {
        let mut err = ady;
        let mut errprev = ady;
        for _ in 0..ady {
            y += ystep;
            err += ddx;
            if err > ddy {
                x += xstep;
                err -= ddy;
                match (err + errprev).cmp(&ddy) {
                    std::cmp::Ordering::Less => {
                        if !clear(x - xstep, y) {
                            return false;
                        }
                    }
                    std::cmp::Ordering::Greater => {
                        if !clear(x, y - ystep) {
                            return false;
                        }
                    }
                    std::cmp::Ordering::Equal => {
                        if !clear(x - xstep, y) || !clear(x, y - ystep) {
                            return false;
                        }
                    }
                }
            }
            if !clear(x, y) {
                return false;
            }
            errprev = err;
        }
    }
    true
}

/// The MINIMAL chord polyline from `from` to `to` (chord-movement F5): octile A* +
/// greedy LOS string-pulling. Waypoints are tile coordinates (chords connect their
/// CENTERS), exclusive of `from`, terminating at `to`; `Some(vec![])` = already there.
/// `radius` = the footprint's Chebyshev half-width (F7; pawns ship 0). Refusals match
/// [`find_path`] (F5).
pub fn find_chords(
    from: (i32, i32),
    to: (i32, i32),
    radius: i32,
    pathable: &dyn Fn(i32, i32) -> bool,
) -> Option<Vec<(i32, i32)>> {
    // The footprint inflates the PROBE once (F7), so the A* and the string-pull see the
    // same fattened world — a corridor too narrow for the footprint refuses at search
    // time, and adjacent A* steps always hold LOS (the smoothing's loop invariant).
    let fat = |x: i32, y: i32| -> bool {
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                if !pathable(x + ox, y + oy) {
                    return false;
                }
            }
        }
        true
    };
    let probe: &dyn Fn(i32, i32) -> bool = if radius == 0 { pathable } else { &fat };
    let tiles = find_path_octile(from, to, probe)?;
    if tiles.is_empty() {
        return Some(Vec::new());
    }
    let mut pts = Vec::with_capacity(tiles.len() + 1);
    pts.push(from);
    pts.extend(tiles);
    let mut chords = Vec::new();
    let mut anchor = 0usize;
    let mut i = 1usize;
    while i + 1 < pts.len() {
        if line_of_sight(pts[anchor], pts[i + 1], 0, probe) {
            i += 1;
        } else {
            chords.push(pts[i]);
            anchor = i;
            i += 1;
        }
    }
    chords.push(*pts.last().expect("non-empty by construction"));
    Some(chords)
}

/// A polyline's Euclidean length in tiles, from `from` through every waypoint — the
/// tiles/tic schedule's distance input (F6). IEEE hypot: identical on every observer.
pub fn chord_len(from: (i32, i32), chords: &[(i32, i32)]) -> f64 {
    let mut prev = (from.0 as f64, from.1 as f64);
    let mut total = 0.0;
    for &(x, y) in chords {
        let p = (x as f64, y as f64);
        total += (p.0 - prev.0).hypot(p.1 - prev.1);
        prev = p;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe from an ASCII grid: `#` = impathable, anything else open. Off-grid =
    /// CLOSED — the fixtures are sealed boxes so a "wall" cannot be walked around at
    /// infinity. (The unknown-degrades-open law is about unknown KINDS, in the Bundle.)
    fn grid<'a>(rows: &'a [&'a str]) -> impl Fn(i32, i32) -> bool + 'a {
        move |x, y| {
            if y < 0 || x < 0 {
                return false;
            }
            rows.get(y as usize)
                .and_then(|r| r.as_bytes().get(x as usize))
                .map(|&b| b != b'#')
                .unwrap_or(false)
        }
    }

    #[test]
    fn a_water_strait_detours_through_the_gap() {
        // A vertical wall with one gap at y=3: the straight line (len 4) is blocked; the
        // path must thread (2,3).
        let rows = ["..#..", "..#..", "..#..", ".....", "..#.."];
        let g = grid(&rows);
        let p = find_path((0, 1), (4, 1), &g).expect("reachable through the gap");
        assert!(p.contains(&(2, 3)), "threads the gap: {p:?}");
        assert!(p.len() > cheb((0, 1), (4, 1)) as usize, "a detour is LONGER than cheb");
        assert_eq!(p.last(), Some(&(4, 1)));
        assert_eq!(path_len((0, 1), (4, 1), &g), Some(p.len()));
    }

    #[test]
    fn a_full_wall_is_unreachable() {
        let rows = ["..#..", "..#..", "..#..", "..#..", "..#.."];
        assert_eq!(find_path((0, 2), (4, 2), &grid(&rows)), None);
    }

    #[test]
    fn a_diagonal_never_clips_a_corner() {
        // Two water cells touching at a corner between start and dest: the direct
        // diagonal (1,1) → (2,2)-corner slip is illegal; the path must go around.
        let rows = ["...", ".#.", "..."]; // block at (1,1)
        let g = grid(&rows);
        // From (0,1) to (2,1): straight through (1,1) is blocked, and F4 also refuses
        // the corner-hugging diagonals (each clips the rock) — the detour is the FULL
        // way around, 4 hops, not the 3 free diagonals would give.
        let p = find_path((0, 1), (2, 1), &g).expect("around the rock");
        assert_eq!(p.len(), 4, "no corner hug: {p:?}");
        // Now pinch BOTH corners: the diagonal between two blocks may not slip through.
        let rows2 = [".#.", "#.#", ".#."];
        assert_eq!(
            find_path((0, 0), (2, 2), &grid(&rows2)),
            None,
            "corner-to-corner slip refused (F4)"
        );
    }

    #[test]
    fn a_pawn_walks_out_of_a_tree() {
        // F6: the start cell is impathable (a tree grew underfoot) — leaving still works,
        // and the first hop's diagonal corner rule treats the start as open.
        let rows = ["#.", ".."];
        let g = grid(&rows);
        let p = find_path((0, 0), (1, 1), &g).expect("escape is legal");
        assert_eq!(p, vec![(1, 1)]);
    }

    #[test]
    fn the_refusals_are_immediate() {
        let rows = ["..#"];
        let g = grid(&rows);
        assert_eq!(find_path((0, 0), (2, 0), &g), None, "impathable dest = None (F5)");
        assert_eq!(find_path((1, 0), (1, 0), &g), Some(vec![]), "already there");
        assert_eq!(next_step((0, 0), (1, 0), &g), Some((1, 0)), "one hop");
    }

    #[test]
    fn the_cap_reads_as_unreachable() {
        // An open plane with a sealed 1×1 target box: A* floods to the cap and gives up.
        let boxed = |x: i32, y: i32| !((99..=101).contains(&x) && (99..=101).contains(&y) && !(x == 100 && y == 100));
        assert_eq!(find_path((0, 0), (100, 100), &boxed), None, "cap exhaustion (I6)");
    }

    #[test]
    fn the_path_is_deterministic() {
        // Same inputs twice → byte-identical path (the observer-agreement contract).
        let rows = ["....", ".##.", "....", "...."];
        let g = grid(&rows);
        let a = find_path((0, 0), (3, 3), &g).unwrap();
        let b = find_path((0, 0), (3, 3), &g).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn open_ground_is_one_chord() {
        // chord-movement F5: with nothing in the way, ANY slope is a single chord —
        // the minimal-chord claim in its purest form.
        let rows = ["........", "........", "........", "........"];
        let g = grid(&rows);
        assert_eq!(find_chords((0, 0), (7, 3), 0, &g), Some(vec![(7, 3)]));
        assert_eq!(find_chords((0, 2), (7, 2), 0, &g), Some(vec![(7, 2)]));
        assert_eq!(find_chords((3, 1), (3, 1), 0, &g), Some(vec![]), "already there");
        let len = chord_len((0, 0), &[(3, 4)]);
        assert_eq!(len, 5.0, "the 3-4-5 chord");
    }

    #[test]
    fn an_obstacle_bends_the_polyline_at_its_corner() {
        // A wall with its tip at (3,1): the straight (0,3) → (6,3)? no — force a bend:
        // from (0,0) to (6,0) with a wall column at x=3 spanning y=0..=1 — the route
        // must dip under it and come back: more than one chord, all LOS-clean.
        let rows = ["...#...", "...#...", ".......", "......."];
        let g = grid(&rows);
        let chords = find_chords((0, 0), (6, 0), 0, &g).expect("routable");
        assert!(chords.len() >= 2, "a bend is at least two chords: {chords:?}");
        assert_eq!(*chords.last().unwrap(), (6, 0));
        // Every chord must hold LOS — the polyline is self-consistent.
        let mut prev = (0, 0);
        for &w in &chords {
            assert!(line_of_sight(prev, w, 0, &g), "chord {prev:?}→{w:?} clips");
            prev = w;
        }
    }

    #[test]
    fn los_refuses_the_shaved_corner() {
        // The segment (0,1)→(2,1)... straight through the block. And the EXACT corner
        // crossing: (0,0) → (2,2) passes precisely through the corner shared with
        // (1,0)/(0,1) when (1,1) blocks nothing — pinch both flanks and it refuses (F4).
        let rows = [".#.", "#..", "..."];
        let g = grid(&rows);
        assert!(
            !line_of_sight((0, 0), (2, 2), 0, &g),
            "the corner between two blocks may not be slipped"
        );
        let open = grid(&["...", "...", "..."]);
        // (a plain diagonal on open ground is fine, corner crossings and all)
        assert!(line_of_sight((0, 0), (2, 2), 0, &open));
    }

    #[test]
    fn chords_walk_out_of_an_impathable_start() {
        // F6 carries over: the start cell is never probed, by A* or by LOS.
        let rows = ["#..", "...", "..."];
        let g = grid(&rows);
        let chords = find_chords((0, 0), (2, 2), 0, &g).expect("escape is legal");
        assert_eq!(chords, vec![(2, 2)], "one clean chord out of the tree");
    }

    #[test]
    fn the_polyline_is_deterministic() {
        let rows = ["......", "..##..", "..##..", "......"];
        let g = grid(&rows);
        let a = find_chords((0, 1), (5, 2), 0, &g).unwrap();
        let b = find_chords((0, 1), (5, 2), 0, &g).unwrap();
        assert_eq!(a, b, "byte-identical route on every run (I1)");
    }

    #[test]
    fn a_footprint_radius_widens_the_corridor() {
        // F7: radius 1 needs a 3-wide corridor; the 1-wide gap that admits radius 0
        // refuses radius 1.
        let rows = ["..#..", "..#..", ".....", "..#..", "..#.."];
        let g = grid(&rows);
        assert!(find_chords((0, 2), (4, 2), 0, &g).is_some(), "1×1 threads the gap");
        assert_eq!(find_chords((0, 2), (4, 2), 1, &g), None, "radius 1 does not fit");
    }
}
