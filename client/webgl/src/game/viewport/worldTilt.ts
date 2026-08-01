//! The world tilt, and the three coordinate systems it relates — **one source**, TS and GLSL.
//!
//! Work `2026-08-01-z-positioning` P0a. The tilt used to live in `shadowGather.ts` and went out
//! with the 2026-07-31 strip; nothing replaced it, so `worldTiltDeg`, `__tilt()` and the `uTilt`
//! uniform were all absent from the codebase when this stream began (I7). This file is the
//! replacement, and it is the ONLY place the angle appears.
//!
//! ## The three systems ([F7](../../../../../docs/work/2026-08-01-z-positioning/forks.md#f7))
//!
//! | system | is | held by |
//! |---|---|---|
//! | **game** `unit.x/y/z` | the tile the billboard physically occupies, plus its **elevation** | the CPU and `prim_data` |
//! | **world** `world.x/y/z` | true 3D — where shadows and light directions are solved | derived, in the shaders |
//! | **screen** `screen.x/y/z` | what is drawn; `z` is perpendicular to the screen | derived, for placement and z-order |
//!
//! Game is the source. World and screen are both projections of it, and they are projections of
//! *different* kinds — which is the whole reason for keeping three names. Mixing them is what put
//! the pawn's head and its shadow in two different places.
//!
//! ## Why the ground transform is the identity
//!
//! This renderer **does not foreshorten**. `P = (vec2(tileX, tileY) + inTile) * UPT` is a square
//! grid, {@link UNITS_PER_TILE} on both axes, and the art draws every tile as a square — the
//! RimWorld convention, where the 3/4 view lives in the *sprites* rather than in the projection
//! (`docs/art-style.md`). So a drawn tile of northing IS a world tile of northing, and:
//!
//! ```
//! world.x = unit.x        screen.x = unit.x
//! world.y = unit.y        screen.y = unit.y - elevation
//! ```
//!
//! The ground planes coincide. That is a **property of the art choice, not a coincidence** — if the
//! projection ever foreshortens, this file is where that stops being true, and it is written out
//! rather than left implicit precisely so the assumption is findable.
//!
//! ## Where the tilt DOES enter: height
//!
//! Elevation is stored as the **up-screen component** of the elevation ray, not as the ray's length
//! and not as a true height. That is forced by the user's own placement rule — *"the screen-north
//! shift IS the elevation, 1:1"* ([F6](../../../../../docs/work/2026-08-01-z-positioning/forks.md#f6)) —
//! and it is what makes the draw path free: a part at elevation `E` is drawn `E` units north, with
//! no coefficient at all.
//!
//! The ray rises at {@link WORLD_TILT_DEG} above the ground, so a northing component of `E`
//! corresponds to a **true vertical height of `E·tan(θ)`**:
//!
//! ```
//!         ^ world.z (true height)
//!         |        ,-'
//!         |    ,-''  <- the elevation ray, at theta above the ground
//!         |,-''  )theta
//!   ------+--------------> world.y (northing)
//!         |<- E ->|            E = unit.z, the DRAWN up-screen shift
//!                              height = E * tan(theta)
//! ```
//!
//! This is the one coefficient in the file, and it is **metric**: it scales `world.z`, which feeds
//! the caster card's extent, the falloff distance and `N·L`. Getting it wrong does not merely move
//! a shadow — it biases how bright things are.

import { UNITS_PER_TILE } from "./squareMath";

/** The world tilt in degrees, measured **from the ground plane** — 0° would look along the ground,
 *  90° straight down. 65° is the value the deleted `shadowGather.ts` carried and the one the user
 *  reaffirmed for this stream (*"whatever use 65 then … regardless of world angle our math works"*).
 *
 *  **The reference axis is named on purpose.** The same number means a nearly-horizontal camera if
 *  read from the vertical, which would invert every height in the scene. The plan's acceptance for
 *  this item asks for the axis by name for exactly that reason.
 *
 *  **Changing this requires a reload**, not just a rebake: the shaders bake it through a template
 *  literal, the same way {@link INTENSITY_MAX} and `REACH_MAX_TILES` are baked. That is deliberate —
 *  a uniform would let the CPU's copy and the GPU's copy drift, which is the failure this file
 *  exists to prevent. */
export const WORLD_TILT_DEG = 65;

/** `tan(θ)` — the ONE coefficient. Converts the stored elevation (an up-screen shift, in units)
 *  into a true world height. ≈ 2.1445 at 65°. */
export const TILT_TAN = Math.tan((WORLD_TILT_DEG * Math.PI) / 180);

/** True world height, in units, for a stored elevation.
 *  The inverse of {@link elevationForHeight}; the two round-trip. */
export function worldHeightForElevation(elevation: number): number {
  return elevation * TILT_TAN;
}

/** Stored elevation (the up-screen shift the draw path applies) for a true world height.
 *  This is the direction CONTENT authors in — a hat is *"this high"*, and the shift follows. */
export function elevationForHeight(height: number): number {
  return height / TILT_TAN;
}

/** The drawn position for a game position — `screen.y = unit.y − elevation`, no coefficient.
 *  There is no draw-side constant; the shift IS the elevation
 *  ([F6](../../../../../docs/work/2026-08-01-z-positioning/forks.md#f6)). */
export function screenYForGame(unitY: number, elevation: number): number {
  return unitY - elevation;
}

/** Depth perpendicular to the screen, for z-ordering
 *  ([F4](../../../../../docs/work/2026-08-01-z-positioning/forks.md#f4)).
 *
 *  **Only ever compared, never measured**, so it is safe against the one uncertainty left in this
 *  file: sorting is invariant under any positive scale, and this and {@link worldHeightForElevation}
 *  currently share a coefficient. If that turns out to be two different coefficients wearing one
 *  hat, the ordering this produces does not change — only `world.z` would
 *  ([I8](../../../../../docs/work/2026-08-01-z-positioning/issues.md#i8)).
 *
 *  Callers MUST pass a scene-wide reference row, not the prim's own origin: depths measured from
 *  different origins do not compare, which is the condition the user attached to using this as the
 *  ordering key. */
export function screenDepth(unitY: number, elevation: number, referenceY: number): number {
  return (referenceY - unitY) + elevation * TILT_TAN;
}

/** The GLSL half — the same constants and the same three conversions, interpolated from the TS
 *  values above so the two CANNOT disagree ([F2](../../../../../docs/work/2026-08-01-z-positioning/forks.md#f2)).
 *
 *  A writer and a reader disagreeing about one rule has cost this project four separate bugs; the
 *  `LIGHT_LANES_GLSL` pattern next door exists for the same reason and has already paid for itself. */
export const WORLD_TILT_GLSL = /* glsl */ `
const float TILT_TAN = ${TILT_TAN};
const float UPT_TILT = ${UNITS_PER_TILE}.0;

// game -> world. The ground planes COINCIDE (this renderer does not foreshorten -- the 3/4 view is
// in the art, not the projection), so only the height term carries the tilt. z-positioning P0a.
vec3 gameToWorld(vec2 unitXY, float elevation) {
  return vec3(unitXY.x, unitXY.y, elevation * TILT_TAN);
}

// The true world height for a stored elevation -- the ONE coefficient in the system. The stored
// value is the UP-SCREEN SHIFT, not the ray length and not a height, because the draw path applies
// it 1:1 (F6: "the screen-north shift IS the elevation").
float worldHeightForElevation(float elevation) { return elevation * TILT_TAN; }

// game -> screen. No coefficient: the shift IS the elevation.
vec2 gameToScreen(vec2 unitXY, float elevation) {
  return vec2(unitXY.x, unitXY.y - elevation);
}

// screen -> game, for a point known to lie on a receiver at 'elevation' whose ground row is
// 'groundY'. A billboard card is a VERTICAL PLANE AT ONE GROUND ROW, so every texel on it shares
// that row -- the x passes through and the y is the card's, not the texel's (F8).
vec2 screenToGame(vec2 P, float groundY) { return vec2(P.x, groundY); }
`;
