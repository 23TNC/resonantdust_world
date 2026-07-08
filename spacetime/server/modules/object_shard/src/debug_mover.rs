//! Debug mover — a single player-controllable loose thing, for exercising the
//! event-log motion model end to end (`docs/sync.md`, player-control slice).
//!
//! Not autonomous and not scheduled: it sits still until a [`move_debug_mover`]
//! call (driven by a client click, relayed through the gate) commits a path. A
//! path is emitted ALL AT ONCE as future-stamped per-tile `free_things` rows —
//! departure semantics: each row's `location` is the destination tile and its
//! `valid_at` is when the move to it begins. The client projects along them
//! (`WorldBridge.projectMotion`). Because the whole finite path commits in one
//! direct reducer call, there's no scheduler — so it's immune to the scheduler
//! stalls that killed position-streaming.
//!
//! Development aid: one hard-coded object at a fixed id. Real movement (many
//! things, A* around obstacles, selection) generalizes [`move_debug_mover`] later.

use spacetimedb::{reducer, ReducerContext};

use resonantdust_codec::packed::{global_tile, pack_offset, zone_and_location, OFFSET_STEPS};

use crate::free_things::{self, latest};
use crate::time::now_ms;

/// Fixed instance handle for the debug mover — `u64::MAX`, clear of the real
/// allocator ([`free_things::next_object_id`], counting up from 1).
const MOVER_OBJECT_ID: u64 = u64::MAX;
/// What-kind def id the mover renders as (the first thing def; tune to taste).
const MOVER_DEF_ID: u16 = 1;
/// Where the mover spawns (global tile), near the world origin so it's in view.
const START_GX: i32 = 2;
const START_GY: i32 = 2;
/// Surface the mover lives on.
const SURFACE: u8 = 0;
/// Travel time per tile, ms. **Must match the client's `TILE_TRAVEL_MS`** in
/// `WorldBridge.ts` (see `docs/sync.md`) — the projector needs the move speed to
/// separate moving from resting. A real thing carries this as a movement stat;
/// the debug mover hard-codes it on both sides, like `SQUARE = 64`.
const TILE_TRAVEL_MS: u64 = 500;
/// Cap a single move's path length so a far click can't emit a huge batch.
const MAX_PATH_STEPS: usize = 128;

/// A centred sub-tile offset — the tile-to-tile tween carries the motion, so the
/// offset is constant.
fn centre_offset() -> u8 {
    pack_offset(OFFSET_STEPS / 2, OFFSET_STEPS / 2)
}

/// Spawn the mover at its start tile if it doesn't exist yet. Called from the
/// module `init`.
pub fn seed(ctx: &ReducerContext) {
    if latest(ctx, MOVER_OBJECT_ID).is_some() {
        return;
    }
    let (zone_id, location) = zone_and_location(START_GX, START_GY, SURFACE);
    let _ = free_things::place(
        ctx,
        MOVER_OBJECT_ID,
        zone_id,
        location,
        0, // rotation — south
        MOVER_DEF_ID,
        centre_offset(),
        now_ms(ctx),
    );
}

/// Straight-line (Chebyshev) tile path from `(x0,y0)` to `(x1,y1)`, inclusive of
/// both ends — one diagonal-or-orthogonal step per tile, capped at
/// [`MAX_PATH_STEPS`]. A stand-in for real A* until obstacles exist.
fn line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut path = vec![(x0, y0)];
    let (mut x, mut y) = (x0, y0);
    while (x, y) != (x1, y1) && path.len() <= MAX_PATH_STEPS {
        x += (x1 - x).signum();
        y += (y1 - y).signum();
        path.push((x, y));
    }
    path
}

/// Move the debug mover to global tile `(dest_gx, dest_gy)`: pathfind from its
/// current tile and commit the path as future-stamped move rows. A redirect
/// supersedes any pending path (future rows are purged first); clicking the tile
/// it's already on just stops it. Called by the gate on a client move intent.
#[reducer]
pub fn move_debug_mover(
    ctx: &ReducerContext,
    now: u64,
    dest_gx: i32,
    dest_gy: i32,
) -> Result<(), String> {
    let Some(cur) = latest(ctx, MOVER_OBJECT_ID) else {
        return Ok(()); // not spawned yet — nothing to move
    };
    // Clamp the destination into the non-negative world. Negative global tiles
    // wrap (region byte is a `u8`) to the far edge of the map, which would fling
    // the mover across it and generate a giant path — and a player clicking
    // up-left of the origin produces negative tiles too. (Proper in-world bounds
    // validation with rejection is a later validation pass.)
    let (dest_gx, dest_gy) = (dest_gx.max(0), dest_gy.max(0));
    let (cx, cy) = global_tile(cur.zone_id, cur.location);
    let path = line(cx, cy, dest_gx, dest_gy);

    // Reset to just the current row before committing the new path: this both
    // supersedes any pending path (redirect) and clears completed-path history, so
    // the mover never accumulates cruft between GC sweeps. `cur` survives as the
    // tween's "from" tile.
    free_things::retain_only(ctx, MOVER_OBJECT_ID, cur.valid_at);

    // Emit the new path, skipping the current tile (`path[0]`); each step departs
    // one travel time after the previous. `valid_at` = departure time.
    let offset = centre_offset();
    let mut depart = now;
    for &(gx, gy) in path.iter().skip(1) {
        let (zone_id, location) = zone_and_location(gx, gy, SURFACE);
        free_things::place(ctx, MOVER_OBJECT_ID, zone_id, location, 0, MOVER_DEF_ID, offset, depart)?;
        depart += TILE_TRAVEL_MS;
    }
    Ok(())
}
