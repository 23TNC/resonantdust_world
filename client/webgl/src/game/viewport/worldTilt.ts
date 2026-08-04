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
//! **Two different quantities land in `unit.z`, and only one of them needs converting.**
//!
//! 1. **A light's height is ALREADY a world height.** `SquareCache.height` is documented as
//!    *"world px above the ground plane"*, and `RecordSync` stores it straight:
//!    `unitZ = L.height / SQUARE * UNITS_PER_TILE`. A torch's authored `2.5` means 2.5 tiles up in
//!    the world. **No coefficient.**
//! 2. **A DRAWN extent is a screen quantity** — a card `subH` units tall on screen, or a point part
//!    way up it. Converting one to a world elevation takes **`sin(θ)`**, not `tan(θ)`.
//!
//! The second is not a guess. The retired `shadowGather.ts` used exactly this, and
//! `content/things.toml` still records the contract in a comment written against it:
//!
//! > *"`shadowCover` projects the caster's card top (elevation `Zt = H·sin(WORLD_TILT)`) from the
//! > light onto the ground … A 2-tile tree tops out at 32·sin55° ≈ 26 units, so a light must clear
//! > that to cast against anything."*
//!
//! ```
//!         ^ world.z (true height)
//!         |   ,-'|
//!         | ,-'  | H*sin(theta)     H = the card's DRAWN height, in units
//!         |,-----+
//!   ------+--------------> the ground
//!          )theta
//! ```
//!
//! **This corrects what P0a first recorded.** I derived `tan(θ)` from the premise that `unit.z` was
//! an up-screen shift needing to be lifted into the world. It is not — for lights it is already a
//! world height, and for drawn extents the factor is `sin(θ)`. The mistake was reasoning from the
//! coordinate model in the abstract instead of reading what the code and the corpus already stored.
//!
//! The coefficient is **metric**: it scales world heights, which feed the caster card's extent, the
//! falloff distance and `N·L`. A wrong value does not merely move a shadow — it changes how bright
//! things are.

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

/** `sin(θ)` — converts a **drawn extent** (units of card, up the screen) into a world elevation.
 *  ≈ 0.9063 at 65°. This is the factor the retired `shadowGather.ts` used and the one
 *  `content/things.toml` still documents. */
export const TILT_SIN = Math.sin((WORLD_TILT_DEG * Math.PI) / 180);

/** `tan(θ)` — **depth perpendicular to the screen**, for z-ordering only (user: *"screen.z is
 *  perpendicular to the screen … would make a decent z-ordering metric"*). Never a height. */
export const TILT_TAN = Math.tan((WORLD_TILT_DEG * Math.PI) / 180);

/** World elevation, in units, for a DRAWN extent — a card `drawnUnits` tall reaches this high.
 *  A light's stored height needs no conversion; it is already a world height. */
export function worldHeightForDrawn(drawnUnits: number): number {
  return drawnUnits * TILT_SIN;
}

/** The inverse — the drawn extent that reaches a given world height. */
export function drawnForWorldHeight(height: number): number {
  return height / TILT_SIN;
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
 *  **Only ever compared, never measured** — sorting is invariant under any positive scale, so this
 *  is the one place the exact coefficient does not have to be right to be useful.
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
const float TILT_SIN = ${TILT_SIN};
const float TILT_TAN = ${TILT_TAN};
const float UPT_TILT = ${UNITS_PER_TILE}.0;

// A DRAWN extent (units up a card) -> a world elevation. The factor is sin(theta), the same one
// the retired shadowGather.ts used and content/things.toml still documents:
//   "the caster's card top (elevation Zt = H*sin(WORLD_TILT))".
// A LIGHT's stored unit.z needs no conversion -- SquareCache.height is already world px above the
// ground plane. Two different quantities share the lane; only the drawn one converts.
float worldHeightForDrawn(float drawnUnits) { return drawnUnits * TILT_SIN; }

// game -> world. The ground planes COINCIDE (this renderer does not foreshorten -- the 3/4 view is
// in the art, not the projection), so only the height term carries the tilt.
vec3 gameToWorld(vec2 unitXY, float worldZ) { return vec3(unitXY.x, unitXY.y, worldZ); }

// game -> screen. No coefficient: the shift IS the elevation.
vec2 gameToScreen(vec2 unitXY, float elevation) {
  return vec2(unitXY.x, unitXY.y - elevation);
}

// screen -> game, for a point known to lie on a receiver at 'elevation' whose ground row is
// 'groundY'. A billboard card is a VERTICAL PLANE AT ONE GROUND ROW, so every texel on it shares
// that row -- the x passes through and the y is the card's, not the texel's (F8).
vec2 screenToGame(vec2 P, float groundY) { return vec2(P.x, groundY); }
`;
