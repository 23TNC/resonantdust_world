//! shadowGather (webgl) — the per-light **shadow bitfield** via a fragment **gather**
//! (`2026-07-21-shadow-bitfield`). Each cold light is one bit in a world-space toroidal `RGBA32UI`
//! `shadow-cold` buffer; a fragment computes its texel's full 128-bit mask by looping the reaching
//! lights, testing point-in-projected-silhouette against each light's casters, and OR-ing bits in a
//! register — one write, no ping-pong (F1). Data comes from `ColdShadowData` (`light_data` / `prim_data`
//! / `prim_definition_data`) via `texelFetch`. Layouts authoritative in `docs/VARIABLES.md`.
//!
//! Build order (deviation D-1): this slice is **P4-core** — full recompute each frame, **all** lights
//! (no presence cull yet), the exact fan region (roles 0–6 → 5 triangles) as the per-fragment predicate
//! (no silhouette mask yet). Presence cull (P2), dirty-tile gating (P3) and the silhouette mask layer on
//! next as invisible optimisations. The projected-region maths is a direct port of `shadowCaster.ts`.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Renderer, Program, Geometry, RenderTarget, Texture } from "../../gl";
import type { Camera } from "./Camera";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { ColdShadowData, N_LIGHTS } from "./coldShadowData";
import { SQUARE, UNIT, TEXTILE_UNIT, TEXTILE_SQUARE } from "./squareMath";

/** lightmap P1 Step B: the fine lightmap is TEXTILE_SQUARE/tile; the shadow map stays TEXTILE_UNIT/tile.
 *  This ratio maps a fine light texel to its coarse shadow texel (`fc / FINE_RATIO`) — the per-light shadow
 *  upsample ([forks.md#f4]). */
const FINE_RATIO = TEXTILE_SQUARE / TEXTILE_UNIT;

/** Lights this iteration — a ring of debug lights around the seed tile. `number`-typed so the
 *  isolate-one-light debug path (`MAX_LIGHTS = 1`) below isn't flagged as a constant comparison. */
const MAX_LIGHTS: number = 3;
/** Physical source RADIUS (world px); `/UNIT` → units in the light record. The AREA-LIGHT disk radius the
 *  penumbra samples over (bigger = softer). Per-light (see the config below); `__emit(px)` tunes live. */
const LIGHT_EMITTER = 20;            // 5 units — a modest area light for the emitter-sampled penumbra
/** Debug lights placed at fixed world TILES beyond the one at the seed tile:
 *  `[tileX, tileY, dynamic, emitterPx]`. A `dynamic` light orbits its home every frame (the moving-light
 *  case); static ones never move. `emitterPx` is that light's own area-light radius (world px; per-light,
 *  not a global) — bigger = softer penumbra. Seed = red static (few trees, clean debug); green = dynamic
 *  (in the forest, surfaces motion bugs); blue = static, placed to overlap BOTH so interactions show. */
const EXTRA_LIGHT_TILES: ReadonlyArray<[number, number, boolean, number]> = [
  [34, 25, true, LIGHT_EMITTER],   // green — dynamic, dense forest
  [45, 40, false, LIGHT_EMITTER],  // blue  — static, interacts with red + green
];
/** World-px radius a dynamic light orbits its home. */
const MOTION_RADIUS = 3 * SQUARE;
/** Light height: 40 units = 160 world px = 2.5 tiles — above the tree billboard (2 tiles / 32 units).
 *  Lower = longer shadows. */
const LIGHT_Z = 40 * UNIT;
const LIGHT_REACH = 12 * SQUARE;     // illumination range (world px) — how far the light throws (12 tiles)
const RING_RADIUS = 2 * SQUARE;
// The shadow map is the TEXTILE_UNIT map (map-model.md): 16 textiles/tile, 1 textile = 1 unit
// (= UNIT px). Sized `cols·TEXTILE_UNIT × rows·TEXTILE_UNIT`, toroidal like the cold cache window.
/** GLSL literals for the world constants (a tile is `SQUARE` world px; `1 unit = SQUARE/16` px). */
const SQF = SQUARE.toFixed(1);
const UNITF = UNIT.toFixed(4);
/** THE world ground tilt — the ground plane meets the view plane at this angle (docs/work/2026-07-23-world-geometry).
 *  A billboard is drawn parallel to the view; a point Δ up it has fictional height `sin(WORLD_TILT)·Δ`. One
 *  place, so the world tilt is never a magic `65.0` scattered through the shaders. */
const WORLD_TILT_DEG = 55;                    // DEFAULT ground tilt — 55° felt right for the projection (user,
                                             // 2026-07-24; 65° oval too squished, 45° ground too steep).
                                             // LIVE value rides the data map; `__tilt(deg)` sweeps it live.
/** shadows-onto-prims: a billboard pixel drawn Δ units above its own base has fictional height
 *  `sin(WORLD_TILT)·Δ` (the ratified world-geometry model) — the DEFAULT receiver-elevation gain that makes
 *  a cast shadow climb the sprite. `__elevk(k)` tunes the climb by eye; 0 collapses it to the flat ground
 *  shadow. Empirical because the caster card runs its own internal ratio (the documented 0.5 north offset). */
const ELEV_K_DEFAULT = Math.sin(WORLD_TILT_DEG * Math.PI / 180);
/** world-space-lighting (P1): the N–S un-foreshorten factor = 1/cos(WORLD_TILT). Screen N–S is compressed
 *  by cos65 (the ground is a 65° plane), so true world N–S = screen·(1/cos65). Derived in
 *  work/2026-07-24-world-space-lighting/model.md; sin65-vs-cos65 is the flagged risk, so it's live-tunable. */
const INV_COS_TILT_DEFAULT = 1 / Math.cos(WORLD_TILT_DEG * Math.PI / 180);
/** Lift the rendered shadow up (toward smaller world-y) by this many world units — a fragment shows
 *  shadow if the point this far BELOW it is shadowed, so the whole silhouette slides up. Closes the ~1
 *  unit gap between the shadow base and the sprite's drawn base (a fixed anchor discrepancy: the sprite
 *  bottom sits ~1 unit south of the prim base-centre the shadow projects from). `__lift(u)` tunes live. */
const SHADOW_LIFT = 3.0;             // units the shadow slides up to seat on the sprite base (sprite bottom
                                    // sits ~3 units south of the prim base-centre the shadow projects from)
/** shadows-onto-prims: the receiver MASK (receiverCover) cuts the sprite-shaped hole slightly OFF from the
 *  drawn albedo — the def tight-bbox anchor (`Ac`, from coldShadowData) doesn't land exactly where the
 *  sprite is drawn (thingPlacement). Empirically it sits ~2 units too far SOUTH and ~0.5 unit too far EAST,
 *  so shift the mask NORTH-WEST by this to seat the cut on the sprite. NOT a shadow-field shift — it's where
 *  we CUT (user, 2026-07-24). Root cause (tight-bbox anchoring vs draw placement) not chased — see
 *  issues.md#i4; if the alignment ever drifts, this pair is the knob. */
// COHERENCE (2026-07-24): the def now samples the FULL square frame (== the composite's coordinate system),
// so the receiver mask lands on the drawn albedo with NO correction — the old NW shift was band-aiding the
// per-map min-bbox offset that's now gone. Kept as a knob at 0; if a residual seat is ever needed it's here.
const RECV_ALIGN_X = 0.0;
const RECV_ALIGN_Y = 0.0;
const RECV_ALIGN_XF = RECV_ALIGN_X.toFixed(1);
const RECV_ALIGN_YF = RECV_ALIGN_Y.toFixed(1);
/** shadows-onto-prims: the seen-face cull excludes casters that aren't strictly SOUTH of the receiver base
 *  — widened into a UNIT band so a caster within this many units of the receiver's base (nearly the same y,
 *  i.e. self / same-object) is also excluded, killing near-self shadow. Measured in world units (user). */
const SELF_BAND = 4.0;
const SELF_BANDF = SELF_BAND.toFixed(1);
/** shadows-onto-prims: extend the shadow quad's BASE (contact) edge this many units SOUTH — tucking the
 *  bottom of the shadow into the caster's own footprint/trunk (which reads dark / is under the sprite, so
 *  it's not visibly coloured) to close the seam at the shadow base (user nit). */
const SHADOW_BASE_PUSH = 0.75;
const SHADOW_BASE_PUSHF = SHADOW_BASE_PUSH.toFixed(2);
/** shadows-onto-prims: fade the receiver MASK to GROUND over this many units at the sprite BASE. Without it
 *  the mask claims the trunk base as a thing texel, whose thing-path self-excludes the caster → the ground
 *  contact shadow (incl. the pushed base) gets CULLED there, leaving a lit seam. Fading the bottom band to
 *  ground lets that contact shadow show (thing ≈ ground at the base anyway). Units (user nit). */
const MASK_BASE_FADE = 1.5;
const MASK_BASE_FADEF = MASK_BASE_FADE.toFixed(1);
const SHADOW_LIFTF = SHADOW_LIFT.toFixed(1);
/** Lights per tile in presence + shadow-cold: 14 (7 per presence set), each a u8 coverage in the
 *  128-bit shadow-cold texel (slot i at channel i>>2, bits (i&3)·8). */
const PRES_SLOTS = 14;

interface Light {
  x: number;
  y: number;
  z: number;
  reach: number;         // illumination range (world px)
  emitterRadius: number; // physical source size (world px) — penumbra softness
  hx: number;            // home (world px) — a dynamic light orbits this
  hy: number;
  dynamic: boolean;      // moves every frame (around its home); static lights never move
}

/** The cold cache's toroidal tile window (from `SquareCache.window`) — shadow-cold aligns to it. */
export interface TileWindow {
  winCol: number;
  winRow: number;
  cols: number;
  rows: number;
}

/** Shared GLSL: packed-position decode + ground projection + the fan region predicate (roles → tris). */
const GATHER_COMMON = /* glsl */ `
const float UNIT = ${UNITF};      // SQUARE/16 (compile-time; px per unit)
const float SQ = ${SQF};          // SQUARE world px per tile
const float UPT = SQ / UNIT;      // world UNITS per tile (= TEXTILE_UNIT = 16)
const uint  ZD = 16u, RD = 16u;  // ZONE_DIM, REGION_DIM
// THE unified data texture (1024×1024): linear index → texel; 64-row bands (VARIABLES.md).
const int DEF_BASE = 0, PRIM_BASE = 65536, LIGHT_BASE = 131072, CONST_BASE = 1047552; // row 1023
const int PRESENCE_BASE = 196608, CASTER_BASE = 262144, PRESENCE_HI_BASE = 327680; // tile-keyed sets 3, 4, 5
uvec4 fetchLin(highp usampler2D t, int i) { return texelFetch(t, ivec2(i & 1023, i >> 10), 0); }
// Region-torus zone-strip fold: world tile → in-set id (matches TS foldTile). World tiles ≥ 0.
int fmod16(int v) { return ((v % 16) + 16) % 16; }
int foldTile(int wc, int wr) {
  int zx = fmod16(wc >> 4), zy = fmod16(wr >> 4);        // in-region zone (÷16 then mod 16)
  int tx = fmod16(wc), ty = fmod16(wr);                  // in-zone tile
  return ((zx >> 2) + zy * 4) * 1024 + (zx & 3) * 256 + ty * 16 + tx;
}
// Slot i (0..6) of a tile-keyed texel: slot0 = R.low, slots 1–6 = G/B/A high|low. Static lane
// select (no dynamic subscript — the ANGLE/D3D freeze foot-gun).
uint laneN(uvec4 v, int c) { return c == 1 ? v.y : (c == 2 ? v.z : v.w); }
uint tileSlot(uvec4 t, int i) {
  if (i == 0) return t.x & 0xffffu;
  uint w = laneN(t, (i + 1) >> 1);
  return (i & 1) == 1 ? (w >> 16) : (w & 0xffffu);
}
vec2 decodePos(uint p) {          // position_anchor_reference → world UNITS
  uint region = (p >> 24) & 255u, zone = (p >> 16) & 255u, tile = (p >> 8) & 255u, anchor = p & 255u;
  uint wtx = (((region >> 4u) * RD + (zone >> 4u)) * ZD + (tile >> 4u));
  uint wty = (((region & 15u) * RD + (zone & 15u)) * ZD + (tile & 15u));
  return vec2(float(wtx * 16u + (anchor >> 4u)), float(wty * 16u + (anchor & 15u)));
}
// #3: a caster prim's anchor TILE ROW (its base-centre row) — the depth key compared against a receiving
// thing's row so a sprite standing in front of the caster isn't shadowed. UPT units per tile.
float casterRowOf(highp usampler2D data, uint primIdx) {
  return floor(decodePos(fetchLin(data, PRIM_BASE + int(primIdx)).y).y / UPT);
}
// The WORLD ground tilt (radians), read from the data map's constants A reserve (centidegrees). Lives in the
// DATA MAP (not a dedicated uniform) so every lighting shader gets the angle for free; __tilt(deg) sets it
// live and the whole geometry (shadow projection, elevation, falloff, caster lean) re-tilts together.
float worldTiltRad(highp usampler2D data) {
  return float(fetchLin(data, CONST_BASE).w & 0xffffu) * (0.01 * 3.14159265 / 180.0);
}
// PURE QUAD placement (no texture sample, no u/v inversion). The caster is a flat 3D card: base on the
// ground (y = Yb, z = 0), top tilted north + elevated (y = Yt, z = Zt). Project its 4 corners from the
// light onto the ground → a convex ground quad; P is shadowed iff it lies inside that quad. The only
// division is the single top-corner projection factor k — no per-pixel /denom or /s to blow up.
//   card corners: base (A.x±W/2, Yb, 0)  ·  top (A.x±W/2, Yt, Zt)
//   ground(C) = L.xy + (L.z/(L.z - C.z)) * (C.xy - L.xy)   → base stays put, top scales by k
float cross2(vec2 a, vec2 b) { return a.x * b.y - a.y * b.x; }
float shadowCover(vec2 P, vec2 A, vec3 L, float W, float H, highp usampler2D data) {
  float th = worldTiltRad(data), ct = cos(th), st = sin(th); // WORLD_TILT — from the data map (F4)
  // KNOWN DEVIATION (kept deliberately — user, 2026-07-23): the north offset uses 0.5·H·cos65, HALF the
  // strict parallel-to-view billboard (H·cos65). Likely the shadow-design "wedge" ±depth half; the
  // elevation (Zt = H·sin65) already matches the canonical model. Left as-is because shadows read right.
  // If the shadow GEOMETRY ever looks wrong, this 0.5 is a prime suspect — see
  // docs/work/2026-07-23-world-geometry forks.md#f2 + issues.md#i1.
  float Yt = A.y - 0.5 * H * ct, Zt = H * st;    // card top: tilted north (0.5·H·cos65) + elevated (H·sin65)
  float Yb = A.y + ${SHADOW_BASE_PUSHF};          // card base — pushed SOUTH into the caster footprint (seam close)
  float k = L.z / (L.z - Zt);                     // ground-projection factor for the top corners
  if (k <= 0.0) return 0.0;                        // top at/above the light — no forward shadow
  float hw = 0.5 * W;
  vec2 bl = vec2(A.x - hw, Yb);                    // base corners project to themselves (z = 0)
  vec2 br = vec2(A.x + hw, Yb);
  vec2 tl = L.xy + k * (vec2(A.x - hw, Yt) - L.xy);   // top corners projected from the light
  vec2 tr = L.xy + k * (vec2(A.x + hw, Yt) - L.xy);
  // point in convex quad (bl → br → tr → tl): inside iff all four edge cross products share a sign.
  float d0 = cross2(br - bl, P - bl);
  float d1 = cross2(tr - br, P - br);
  float d2 = cross2(tl - tr, P - tr);
  float d3 = cross2(bl - tl, P - tl);
  bool pos = d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0;
  bool neg = d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0;
  return (pos || neg) ? 1.0 : 0.0;               // solid quad: 1 occluded, 0 lit
}
// Coverage of one caster (prim index into prim_data) at P from light L: read the prim record + its
// definition (position + geo W/H), then the pure-quad test; if occluded, apply the sprite's SHAPE
// (P4) — invert P back to the card's (s,t) (in-range BY CONSTRUCTION: P is inside the projected
// quad, so no u/v-out-of-range class of reject exists) and sample the surface silhouette (coverage,
// B channel) at the def's opaque frame. frame_w = 0 (not resolved / off-page) → solid quad.
// A disk sample position (unit radius) for area-light emitter sampling — 16 taps: centre + a 6-ring +
// a 9-ring. Deterministic (a pure function of the index) so the corridor and brute paths stay
// bit-identical. Scaled by the emitter radius (world units) at the call site.
vec2 emitterOffset(int i) {
  if (i == 0) return vec2(0.0);
  if (i <= 6) { float a = (float(i) - 1.0) / 6.0 * 6.2831853; return 0.55 * vec2(cos(a), sin(a)); }
  float a = (float(i) - 7.0) / 9.0 * 6.2831853 + 0.4; return vec2(cos(a), sin(a));
}
// Coverage of one caster (prim index into prim_data) at P from light L. Reads the prim record + its
// definition (position + geo W/H + frame), places the tilted card, then computes a GROUND-PROJECTED
// penumbra by AREA-LIGHT sampling: the light is a disk of radius emitter (units) at height Lz; for
// each sub-light we re-invert the ground projection P → card (s,t) and sample the HARD silhouette
// (surface B) at that point; the average over the disk is the soft coverage. Why this grounds the
// shadow (vs blurring the projected card image): a silhouette edge at height z casts to a ground point
// that shifts with the sub-light by ∝ z/(Lz−z) — so a z≈0 CONTACT edge does not move (hard, attached
// base), a high edge moves a lot (soft tip, detail dissolving), and points just outside the true edge
// are covered by only SOME sub-lights (soft outer perimeter, no hard card boundary). lod<4 (no
// silhouette resolved) or a point light (emitter≈0) falls back to the solid/hard quad.
float casterCover(uint primIdx, vec2 P, vec3 L, float emitter, highp usampler2D data, sampler2D surf) {
  if (primIdx == 0u) return 0.0;
  uvec4 Pd = fetchLin(data, PRIM_BASE + int(primIdx));      // v2.1: R = id|reserved, G = position, B = orient
  uint pos = Pd.y;
  uint orient = Pd.z;
  vec2 A = decodePos(pos);                                  // the prim's stored anchor (full-box base-centre)
  int defIdx = int((orient >> 6) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  // v2.1: R = u16 id | u10 offset_x | u6 reserved; G = u9 W | u9 H | u4 span (tiles−1) | u10 offset_y.
  // B: frame origin (16-px grid) + page + lod exponent + 3x3 anchors. A: nudges.
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);    // frame world span (units)
  float ox = float((D.x >> 6) & 1023u), oy = float(D.y & 1023u);
  uint lod = (D.z >> 4) & 15u;
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u); // anchors: 0 none | 1 half | 2 full
  // Anchor shift (units): the bbox's anchored point minus the FULL footprint box's same-anchored
  // point — prim_data stores the full box's base-centre (= anchor 1,2), so shadows land the bbox's
  // base-centre on it; general anchors go live when prim_data carries reported x/y (F3/P5).
  vec2 sh = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU,
                 oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  uint rot = (orient >> 22) & 3u;                           // 1 = E, 3 = W (mirrored E)
  if (rot == 3u) sh.x = -sh.x;                              // flipped sprite → mirrored bbox placement
  vec2 Ac = A + sh;
  float th = worldTiltRad(data), ct = cos(th), st = sin(th); // WORLD_TILT — from the data map (F4)
  // HARD-QUAD gate (NOT dilated) — a caster occludes P only where P is inside its projected quad, which
  // is exactly the occluder set the corridor walk is proven to visit (P6 identity). The emitter penumbra
  // lives INSIDE this quad: the silhouette edge softens as sub-lights partially cover it, the base stays
  // hard (z≈0 edges don't move with the sub-light), and detail dissolves at the tip (high edges do). The
  // outward feather beyond the silhouette extremes would need the corridor pad widened + re-proven — a
  // follow-up, not worth breaking bit-identity for. Outside the quad → lit; skip the sub-light loop.
  if (shadowCover(P, Ac, L, W, H, data) <= 0.0) return 0.0;
  if (lod < 4u || emitter < 0.5) return shadowCover(P, Ac, L, W, H, data); // no silhouette / point light → hard quad
  // Frame sampling constants (whole-px-per-unit; ppu = 2^lod / spanU is a pow2 ≥ 1 by construction).
  // Window top-left = frame origin + offset·ppu − nudge. Atlas rows are image-top-down; card t=0 is the
  // sprite's BOTTOM row → v = 1−t.
  float ppu = float(1u << lod) / spanU;
  float fx = float((D.z >> 22) & 1023u) * 16.0, fy = float((D.z >> 12) & 1023u) * 16.0;
  float nx = float(int((D.w >> 20) & 4095u) - 2048);   // u12, +2048 bias — full either-direction range
  float ny = float(int((D.w >> 8) & 4095u) - 2048);
  float side = float(1u << lod);
  vec2 fmin = vec2(fx, fy), fmax = vec2(fx + side, fy + side);
  // Area-light emitter sampling — 16 sub-lights over the disk, HARD silhouette per sub-light, averaged.
  //   invert P → card (s,t):  y(t) = Ac.y − 0.5·t·H·cosθ, z(t) = t·H·sinθ
  //   ⇒ t = Lp.z·(P.y − Ac.y) / (H·(sinθ·(P.y − Lp.y) − 0.5·Lp.z·cosθ));  s from the row's k factor.
  // A sub-light contributes only if its (s,t) lands inside the card AND the silhouette is opaque there;
  // out-of-range sub-lights are "lit" (0) → the average is the projection-correct soft coverage.
  float cov = 0.0;
  for (int i = 0; i < 16; i++) {
    vec3 Lp = vec3(L.xy + emitterOffset(i) * emitter, L.z); // a sub-light on the emitter disk
    float denom = H * (st * (P.y - Lp.y) - 0.5 * Lp.z * ct);
    if (abs(denom) < 1e-4) continue;
    float t = Lp.z * (P.y - Ac.y) / denom;                  // card height where P's shadow ray grazes
    if (t < 0.0 || t > 1.0) continue;                       // P not under this sub-light's card span → lit
    float k = Lp.z / (Lp.z - t * H * st);
    if (k <= 0.0) continue;
    float s = ((P.x - Lp.x) / k + Lp.x - Ac.x) / W + 0.5;
    if (s < 0.0 || s > 1.0) continue;
    if (rot == 3u) s = 1.0 - s;                             // W-facing = mirrored E frame
    vec2 uv = vec2(fx, fy) + vec2(ox, oy) * ppu - vec2(nx, ny) + vec2(s * W, (1.0 - t) * H) * ppu;
    if (uv.x < fmin.x || uv.x >= fmax.x || uv.y < fmin.y || uv.y >= fmax.y) continue; // outside frame → lit
    cov += texelFetch(surf, ivec2(uv), 0).b;                // hard silhouette coverage (B carries its own edge AA)
  }
  return cov / 16.0;
}
// shadows-onto-prims (attempt #3, IN-FAMILY): is world point P inside prim's UPRIGHT drawn billboard, and
// opaque there? Returns the prim's base tile ROW if so (drives the receiver elevation), else -1. Mirrors
// casterCover's prim/def decode, but with NO light projection — the sprite is drawn parallel to the view, so
// (s,t) come straight from P's offset in the [Ac.x±W/2] × [Ac.y−H .. Ac.y] rect. Reads ONLY the data texture
// + surface atlas by index — never a textile_slot map by world coord — so it is zoom-stable by construction.
float receiverCover(uint primIdx, vec2 P, highp usampler2D data, sampler2D surf, vec2 align, out float baseYOut) {
  baseYOut = 0.0;
  uvec4 Pd = fetchLin(data, PRIM_BASE + int(primIdx));
  vec2 A = decodePos(Pd.y);
  uint orient = Pd.z;
  int defIdx = int((orient >> 6) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  if (W <= 0.0 || H <= 0.0) return -1.0;
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);
  float ox = float((D.x >> 6) & 1023u), oy = float(D.y & 1023u);
  uint lod = (D.z >> 4) & 15u;
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u);
  vec2 sh = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU, oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  uint rot = (orient >> 22) & 3u;
  if (rot == 3u) sh.x = -sh.x;
  vec2 Ac = A + sh;                                         // tight-bbox base-centre (same as casterCover)
  Ac -= align;                                             // seat on the drawn albedo — shadow + lighting pass their OWN offset
  float s = (P.x - (Ac.x - 0.5 * W)) / W;                   // 0 left → 1 right of the drawn rect
  float t = (Ac.y - P.y) / H;                              // 0 at the base → 1 at the top (north)
  if (s < 0.0 || s > 1.0 || t < 0.0 || t > 1.0) return -1.0; // outside the drawn billboard → not this prim
  baseYOut = Ac.y;                                          // EXACT receiver base y (units) — not row-quantised
  // Fade the mask to GROUND over the bottom band so the ground contact-shadow shows at the trunk base
  // (else the mask claims it as a thing texel and the thing-path self-excludes → a lit seam).
  float bf = clamp((Ac.y - P.y) / ${MASK_BASE_FADEF}, 0.0, 1.0); // 0 at the base → 1 above the band
  if (lod < 4u) return bf;                                  // no silhouette resolved → solid rect × base fade
  float ppu = float(1u << lod) / spanU;
  float fx = float((D.z >> 22) & 1023u) * 16.0, fy = float((D.z >> 12) & 1023u) * 16.0;
  float nx = float(int((D.w >> 20) & 4095u) - 2048), ny = float(int((D.w >> 8) & 4095u) - 2048);
  float side = float(1u << lod);
  if (rot == 3u) s = 1.0 - s;                              // W-facing = mirrored E frame (matches casterCover)
  vec2 uv = vec2(fx, fy) + vec2(ox, oy) * ppu - vec2(nx, ny) + vec2(s * W, (1.0 - t) * H) * ppu;
  if (uv.x < fx || uv.x >= fx + side || uv.y < fy || uv.y >= fy + side) return 0.0; // outside frame → gap (0 cover)
  // HARD presence at the composite's 0.5 contour (the same threshold the MRT bake discards at) — sampling the
  // sub-0.5 anti-aliased ramp let a prim's normal reach ~2px PAST its crisp silhouette onto the neighbour.
  return (texelFetch(surf, ivec2(uv), 0).b >= 0.5 ? 1.0 : 0.0) * bf; // presence (0/1) × base fade
}
// Which standing prim is DRAWN at texel P, and its base row? Scan the caster buckets a few rows SOUTH (a
// billboard draws NORTH of its base, so the covering prim's base sits at/south of the drawn texel), test each
// prim's upright silhouette, and take the FRONTMOST (southmost = max row) cover. -1 = ground (no prim drawn).
// The buckets + surface are contiguous/index-addressed (zoom-safe); this is the in-family replacement for the
// reverted attempt's zdepth-composite read.
float receiverAt(vec2 P, highp usampler2D data, sampler2D surf, vec2 align, out uint rprim, out float rcov) {
  int wc = int(floor(P.x / UPT));
  int r0 = int(floor(P.y / UPT));
  float best = -1.0;
  rprim = 0u;                                                // the winning (frontmost) receiver prim — for self-exclusion
  rcov = 0.0;                                                // its soft silhouette coverage at P (the edge blend)
  for (int dy = 0; dy <= 5; dy++) {                         // constant bound; covers billboards up to ~5 tiles
    uvec4 cb = fetchLin(data, CASTER_BASE + foldTile(wc, r0 + dy));
    for (int c = 0; c < 7; c++) {
      uint primIdx = tileSlot(cb, c);
      if (primIdx == 0u) continue;
      float brOut; float cov = receiverCover(primIdx, P, data, surf, align, brOut);
      if (cov > 0.0 && brOut > best) { best = brOut; rprim = primIdx; rcov = cov; } // frontmost cover wins row+prim+cov
    }
  }
  return best;
}
// lightmap (CO-PACK): the world-frame NORMAL at texel P for the prim DRAWN there. The four maps are quadrants
// of ONE co-packed atlas frame, so the NORMAL quadrant = the SURFACE frame origin + (side, -side) on the SAME
// page (uSurface) — identical frameRel to the silhouette in receiverCover, only the quadrant origin shifted.
// Frame-indexed (NOT a world-coord composite read) → zoom-stable; present at EVERY lod the surface is (same
// frame). Returns vec3(0) when unavailable (no silhouette lod / outside frame) → caller uses a flat up.
vec3 primNormal(uint primIdx, vec2 P, highp usampler2D data, sampler2D surf, vec2 align, out float sOut) {
  sOut = -1.0;
  uvec4 Pd = fetchLin(data, PRIM_BASE + int(primIdx));
  vec2 A = decodePos(Pd.y);
  uint orient = Pd.z;
  int defIdx = int((orient >> 6) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  if (W <= 0.0 || H <= 0.0) return vec3(0.0);
  uint lod = (D.z >> 4) & 15u;
  if (lod < 4u) return vec3(0.0);                            // no silhouette lod → no usable frame
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);
  float ox = float((D.x >> 6) & 1023u), oy = float(D.y & 1023u);
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u);
  vec2 shf = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU, oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  uint rot = (orient >> 22) & 3u;
  if (rot == 3u) shf.x = -shf.x;
  vec2 Ac = A + shf;
  Ac -= align;                                             // lighting passes its OWN offset (decoupled from the shadow mask)
  float s = (P.x - (Ac.x - 0.5 * W)) / W;
  float t = (Ac.y - P.y) / H;
  if (s < 0.0 || s > 1.0 || t < 0.0 || t > 1.0) return vec3(0.0);
  sOut = s;                                                 // sprite-relative horizontal (0 left → 1 right, pre-mirror) for debug
  float ppu = float(1u << lod) / spanU;
  float nx = float(int((D.w >> 20) & 4095u) - 2048), ny = float(int((D.w >> 8) & 4095u) - 2048);
  float side = float(1u << lod);                            // quadrant size (= surface frame side)
  // NORMAL quadrant origin: surface frame (D.z fx|fy) shifted +side E, -side N (TR quadrant vs BL).
  float sfx = float((D.z >> 22) & 1023u) * 16.0, sfy = float((D.z >> 12) & 1023u) * 16.0;
  float nfx = sfx + side, nfy = sfy - side;
  if (rot == 3u) s = 1.0 - s;                               // W-facing = mirrored E frame
  vec2 uv = vec2(nfx, nfy) + vec2(ox, oy) * ppu - vec2(nx, ny) + vec2(s * W, (1.0 - t) * H) * ppu;
  if (uv.x < nfx || uv.x >= nfx + side || uv.y < nfy || uv.y >= nfy + side) return vec3(0.0);
  vec3 n = texelFetch(surf, ivec2(uv), 0).xyz * 2.0 - 1.0;   // RGB [0,1] → XYZ [-1,1] (OpenGL +Y-up)
  if (rot == 3u) n.x = -n.x;                                // mirror the normal's X with the frame
  return normalize(n);
}
// shadows-onto-prims: ONE per-caster test shared by the corridor + brute walks (so both stay bit-identical —
// the P6 identity invariant). Q = the point whose shadow we sample (ground texel = the lifted texel; thing
// texel = the elevated pixel's ground projection G). Two THING-only culls (the user's cone method):
//   (0) SELF — the caster must not be the receiver's OWN prim. The row cull (1) can't catch this on its own:
//       the receiver row derives from the tight-bbox base (Ac) while a caster's row derives from the raw
//       anchor (prim.y+height), so "same prim" is NOT "same row". Exact prim-id match kills self-casting
//       (the user: prim == prim → cull).
//   (1) SEEN-FACE + NEAR BAND — the caster's base must be more than SELF_BAND units SOUTH of the receiver
//       base (Cb.y > Rbase.y + SELF_BAND), measured in UNITS. Billboards are seen from the south, so a
//       shadow only lands on the VISIBLE face when the caster is in front (south) of the receiver; a caster
//       north of / on / within SELF_BAND units of the receiver base is on nearly the same y (self / same
//       object) and would darken the unseen BACK — excluded (user: wider same-y band, in units).
//   (2) LIGHT-SIDE — the caster must sit between the light and the receiver: dot(Cb−Rbase, Rbase−L) < 0.
//       (A north light + a caster south of the receiver would otherwise false-positive.)
// Returns coverage + the caster row.
float casterOne(uint primIdx, vec2 Q, vec3 L, float emitter, bool isThing, uint rprim, vec2 Rbase,
                highp usampler2D data, sampler2D surf, out float row) {
  row = 0.0;
  if (primIdx == 0u) return 0.0;
  vec2 Cb = decodePos(fetchLin(data, PRIM_BASE + int(primIdx)).y); // caster base (anchor = base-centre)
  if (isThing) {
    if (primIdx == rprim) return 0.0;                       // (0) self — exact same prim → no self-cast
    if (Cb.y <= Rbase.y + ${SELF_BANDF}) return 0.0;        // (1) seen-face + near-band (units): caster must be >SELF_BAND south
    if (dot(Cb - Rbase, Rbase - L.xy) >= 0.0) return 0.0;   // (2) caster must block the light reaching it
  }
  float cc = casterCover(primIdx, Q, L, emitter, data, surf);
  if (cc > 0.0) row = floor(Cb.y / UPT);
  return cc;
}
// One light's shadow at sample point Q — the corridor (corr=1) or brute (corr=0) walk of the caster buckets,
// MAX-accumulating casterOne. Factored so a texel can evaluate BOTH the ground point AND the elevated thing
// point and blend them at the silhouette edge. corr selects the path; the two must stay bit-identical (P6).
// reachT bounds the brute box (unused by the corridor). In GATHER_COMMON so the lightmap bake can re-run it at
// fine res to sharpen the shadow edge (shadow-edge-refine).
float walkShadow(vec3 L, float emitter, vec2 Q, bool isThing, uint rprim, vec2 Rbase,
                 int corr, int reachT, highp usampler2D data, sampler2D surf, out float cdepth) {
  cdepth = 0.0;
  float cov = 0.0;
  if (corr == 1) {
    vec2 a = L.xy / UPT;                                     // light in tile coords
    vec2 b = Q / UPT;                                        // sample point in tile coords (ground = P, thing = G)
    vec2 d = b - a;
    int nsteps = int(ceil(abs(d.x)) + ceil(abs(d.y))) + 1;   // ≥ max-axis span → ≤1 tile between samples
    for (int m = 0; m <= 48; m++) {                          // constant bound
      if (m > nsteps) break;
      vec2 q = a + d * (float(m) / float(max(nsteps, 1)));
      ivec2 qt = ivec2(floor(q));
      for (int n = 0; n < 5; n++) {                          // cross pad: centre, ±x, ±y
        ivec2 o = qt + ivec2(n == 1 ? 1 : (n == 2 ? -1 : 0), n == 3 ? 1 : (n == 4 ? -1 : 0));
        uvec4 cb = fetchLin(data, CASTER_BASE + foldTile(o.x, o.y));
        for (int c = 0; c < 7; c++) {
          uint primIdx = tileSlot(cb, c);
          float r; float cc = casterOne(primIdx, Q, L, emitter, isThing, rprim, Rbase, data, surf, r);
          if (cc > 0.0) { cov = max(cov, cc); cdepth = max(cdepth, r); }
        }
      }
    }
  } else {
    ivec2 lc = ivec2(floor(L.xy / UPT));                     // light's tile
    for (int dy = -16; dy <= 16; dy++) {
      if (dy < -reachT || dy > reachT) continue;
      for (int dx = -16; dx <= 16; dx++) {
        if (dx < -reachT || dx > reachT) continue;
        uvec4 cb = fetchLin(data, CASTER_BASE + foldTile(lc.x + dx, lc.y + dy));
        for (int c = 0; c < 7; c++) {
          uint primIdx = tileSlot(cb, c);
          float r; float cc = casterOne(primIdx, Q, L, emitter, isThing, rprim, Rbase, data, surf, r);
          if (cc > 0.0) { cov = max(cov, cc); cdepth = max(cdepth, r); }
        }
      }
    }
  }
  return cov;
}
`;

const FULLSCREEN_VERT = /* glsl */ `#version 300 es
in vec2 aPos;                    // NDC -1..1 (2 tris)
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

/** Ambient floor — the small base light so unlit areas aren't pure black (F4/P4 tunes; not a global sun,
 *  just a floor per [[lighting-direction]]). GLSL literal. */
export const AMBIENT_LEVEL = 0.12;  // #4: added once in the blit (uAmbient), not baked into either lightmap

// ── LIGHTING (`2026-07-23-lighting`) ──────────────────────────────────────────────────────────────
// The baked LIGHTMAP (F1): a world-space toroidal RT aligned TEXEL-FOR-TEXEL with shadow-cold (same
// cols·TEXTILE_UNIT × rows·TEXTILE_UNIT, same fc→world→slot mapping, same uDirty gating → clean tiles
// persist). Each texel: ambient + Σ over its presence lights of colour·intensity·falloff. The display
// blit multiplies albedo × lightmap. P1 = emission only (no normal, no shadow); P2 folds Lambert
// N·L; P3 masks each light by its shadow-cold u9 coverage. Reuses GATHER_COMMON (fetchLin / foldTile /
// decodePos / tileSlot / SQ·UNIT·UPT + the data-texture BASE consts).
const LIGHT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;    // presence (sets 3/5) + light records (set 2)
uniform highp usampler2D uDirty;   // shadow_dirty — same gate as the shadow gather (clean → persist)
uniform highp usampler2D uShadow;  // this class's shadow RT (COARSE — upsampled per fine texel) — per-slot u9 coverage
uniform int uLightClass;           // #4: accumulate only this class of light — 0 = COLD, 1 = HOT
uniform int uWorldLight;           // world-space-lighting: 1 = TRUE-3D falloff distance (oval + light height), 0 = screen circle
uniform float uNsInv;              // world-space-lighting: N–S un-foreshorten factor (1/cos65 ≈ 2.366; live-tunable)
uniform sampler2D uSurface;        // lightmap (CO-PACK): the shared sprite atlas — silhouette (surface quadrant) AND normal (normal quadrant)
uniform int uShowNormal;           // lightmap P0 debug (__shownormal): 1 = paint the sampled prim normal into oLight
uniform float uNormalPitch;        // lightmap (__pitchnormal): standing-billboard normal pitch (rad, 90°−tilt; live, F7-reconsidered)
uniform vec2 uLightAlign;          // lightmap (__lightalign): the LIGHTING receiver-mask seat (units, NW), decoupled from the shadow's RECV_ALIGN
uniform int uEdgeRefine;           // shadow-edge-refine (__edgerefine): 1 = re-test the caster silhouette at fine res on (0,1) edge texels
uniform int uHideRight;            // DEBUG (__hideright): 1 = blank the right half of each prim's normal in __shownormal
// lightmap P1: sprite-tangent normal to the WORLD frame. Rotate the flat/ground basis (image-up = north, out
// = up) up by phi about the EAST axis: phi 0 = ground (lies in the plane), phi = 90deg minus tilt = a standing
// billboard treated perpendicular to the ground. Image +Y is sprite-north = world -y, so it flips in.
vec3 worldNormal(vec3 n, float phi) {
  float c = cos(phi), s = sin(phi);
  return vec3(n.x, -(n.y * c + n.z * s), -n.y * s + n.z * c);
}
layout(location = 0) out vec4 oLight;  // lightmap P1: SINGLE attachment — Σ colour·intensity·falloff·(1−shadow)·N·L (no ambient)
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
uint lane4(uvec4 v, int c) { return c == 0 ? v.x : (c == 1 ? v.y : (c == 2 ? v.z : v.w)); }
const int FINE = ${FINE_RATIO};        // fine light texels per coarse shadow texel (TEXTILE_SQUARE/TEXTILE_UNIT)
void main() {
  // Same window mapping as the shadow gather (constants row): fc → toroidal slot → world tile → P.
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0xFFFFu);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  int uSlotF = uSlot * FINE;                                   // this map is FINE (TEXTILE_SQUARE/tile); shadow stays coarse
  ivec2 fc = ivec2(gl_FragCoord.xy);                           // FINE light texel
  int sx = fc.x / uSlotF, sy = fc.y / uSlotF;                  // owning tile (fine slot)
  if (texelFetch(uDirty, ivec2(sx, sy), 0).r == 0u) discard;   // clean tile → keep the persistent texel
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols);
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlotF)) / float(uSlotF);
  float ly = (float(fc.y) - float(sy * uSlotF)) / float(uSlotF);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS
  ivec2 fcC = fc / FINE;                                       // the coarse SHADOW texel this fine texel upsamples

  // lightmap P0 (__shownormal): verify the atlas-frame normal read — paint the prim's sampled normal (enc
  // 0.5+0.5) where a prim is drawn, black on ground. Frame-indexed → must be rock-stable across zoom.
  if (uShowNormal == 1) {
    uint rp; float rc;
    receiverAt(P, uData, uSurface, uLightAlign, rp, rc);
    float sD;
    vec3 n = rp != 0u ? primNormal(rp, P, uData, uSurface, uLightAlign, sD) : vec3(0.0);
    // DEBUG (__hideright): blank the RIGHT half of each prim's normal (sprite-relative s > 0.5) so the LEFT
    // half can be compared edge-to-edge against the albedo.
    if (uHideRight == 1 && rp != 0u && sD > 0.5) { oLight = vec4(0.0, 0.0, 0.0, 1.0); return; }
    oLight = vec4(n * 0.5 + 0.5, 1.0);
    return;
  }

  // lightmap P1: the WORLD-frame normal for per-light N·L. Only THINGS get N·L for now (ground keeps falloff,
  // ndl = 1 — the old behaviour; ground-as-prims N·L is a later step). The corpus normal is RAW (camera-facing);
  // worldNormal pitches it to the world frame IN-SHADER (uNormalPitch = 90°−tilt, kept live — F7 reconsidered).
  uint rprimN; float rcovN;
  receiverAt(P, uData, uSurface, uLightAlign, rprimN, rcovN);
  float sDbg;
  vec3 pn = rprimN != 0u ? primNormal(rprimN, P, uData, uSurface, uLightAlign, sDbg) : vec3(0.0);
  bool applyNL = rprimN != 0u && dot(pn, pn) > 0.0;            // thing with a loaded normal → real N·L
  vec3 N = applyNL ? worldNormal(pn, uNormalPitch) : vec3(0.0, 0.0, 1.0);

  int fold = foldTile(wc, wr);
  uvec4 presLo = fetchLin(uData, PRESENCE_BASE + fold);     // lights 0–6
  uvec4 presHi = fetchLin(uData, PRESENCE_HI_BASE + fold);  // lights 7–13
  uvec4 sh = texelFetch(uShadow, fcC, 0);                   // P3: per-slot u9 shadow coverage — COARSE, upsampled (fcC)
  vec3 acc = vec3(0.0);                                     // #4: NO ambient here — the blit adds it once over cold+hot
  for (int slot = 0; slot < 14; slot++) {
    uint li = slot < 7 ? tileSlot(presLo, slot) : tileSlot(presHi, slot - 7);
    if (li == 0xffffu) continue;                            // empty slot
    uvec4 Ld = fetchLin(uData, LIGHT_BASE + int(li));       // G = position, B = colour|intensity, A = z|reach|hot|…
    if (int((Ld.w >> 1) & 1u) != uLightClass) continue;     // #4: this pass accumulates only its class
    vec2 Lxy = decodePos(Ld.y);                             // light ground pos (world units)
    vec3 col = vec3(float((Ld.z >> 24) & 255u), float((Ld.z >> 16) & 255u), float((Ld.z >> 8) & 255u)) / 255.0;
    float intensity = float(Ld.z & 255u) / 255.0;
    float reach = float((Ld.w >> 12) & 0xfffu);             // reach (units)
    vec2 toL = Lxy - P;
    // world-space-lighting P1: TRUE 3D distance instead of the screen radius — the world is a 65° plane, so
    // the N–S screen axis is foreshortened (un-shorten by uNsInv = 1/cos65) and the light sits Lz above the
    // ground. ⟹ the reach is an OVAL (flattened N–S) + a height term (a point under the light isn't at 0).
    // Ground assumption (Pz=0); billboard elevation folds in at P2. Toggle off = the old screen circle.
    float Lz = float((Ld.w >> 24) & 255u);                  // light height (units)
    float dist; vec3 d3;                                     // world-space delta P→light (for dist + N·L dir)
    if (uWorldLight == 1) {
      d3 = vec3(toL.x, toL.y * uNsInv, Lz);                 // un-foreshorten N–S + light height
      dist = length(d3);                                    // true 3D distance (ground point)
    } else {
      d3 = vec3(toL, 0.0);
      dist = length(toL);                                   // in-plane screen distance (units)
    }
    float fall = smoothstep(reach, 0.0, dist);              // 1 at the light → 0 at reach (F2 smoothstep)
    // lightmap P1: per-light Lambert on THINGS — fold max(0, N·L̂) into the contribution so the lightmap stores
    // Σ colour·falloff·(1−shadow)·N·L per light (no lossy aggregate-direction relief). CLIP-TO-PRESENCE: the
    // fine lightmap (64/tile) is still coarser than the sprite edge, so a hard thing/ground classification spills
    // one texel past the silhouette. Blend N·L → ground (ndl 1) by the sprite's soft coverage (rcovN) so the lit
    // region fades exactly to presence — no coarse fringe, and the edge reveals ground not black. ndl = 1 on ground.
    float ndl = applyNL ? max(dot(N, d3 / max(dist, 1e-3)), 0.0) : 1.0;
    // P3 SHADOW: mask by slot i's u9 coverage (low8 in channel i>>2, high bit in A[16+i]) — a shadowed
    // pixel stops receiving this light. Applied to the contribution so it removes both the irradiance AND
    // the relief drive (a fully-shadowed light must not light the sprite's lit side either). Penumbra and
    // umbra (the gather's soft coverage) modulate it smoothly.
    uint v9 = ((lane4(sh, slot >> 2) >> uint((slot & 3) * 8)) & 0xFFu) | (((sh.w >> uint(16 + slot)) & 1u) << 8);
    float shadow = float(v9) / 511.0;
    // shadow-edge-refine: the coarse shadow (16/tile) is nearest-upsampled → blocky edges. Where it's a PARTIAL
    // (0,1) edge value, re-run the caster-silhouette walk at THIS fine texel to SELECT the sharp coverage (only
    // edge texels pay; interior 0/1 is kept free). Ground cast shadow only for now (rprimN 0) — thing texels
    // keep the coarse blended value. Same GROUND-path args as the gather (Pground = P + SHADOW_LIFT, corridor).
    if (uEdgeRefine == 1 && rprimN == 0u && shadow > 0.0 && shadow < 1.0) {
      vec3 L3 = vec3(Lxy, Lz);
      float emitter = float((Ld.w >> 4) & 255u);
      int reachT = int(reach) / int(UPT) + 1;
      vec2 Pg = P; Pg.y += ${SHADOW_LIFTF};
      float cd;
      shadow = walkShadow(L3, emitter, Pg, false, 0u, P, 1, reachT, uData, uSurface, cd);
    }
    float contrib = intensity * fall * ndl * (1.0 - shadow); // shadowed contribution (× Lambert on things)
    acc += col * contrib;
  }
  oLight = vec4(clamp(acc, 0.0, 1.0), 1.0);                 // LDR clamp (F4: tonemap deferred to P4)
}
`;

const GATHER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;       // THE unified data texture (defs|prims|lights|presence|buckets)
uniform highp usampler2D uDirty;      // shadow_dirty — textile_tile map (R8UI): .r nonzero = recompute
uniform sampler2D uSurface;           // the shared surface atlas page (F2) — silhouette coverage in B
uniform int uCorridor;                // P6: 1 = segment-DDA corridor walk, 0 = brute-force reach box
uniform int uLightClass;              // #4: process only this class of light — 0 = COLD (static), 1 = HOT (dynamic)
uniform float uElevK;                 // shadows-onto-prims: receiver-elevation gain (sin65 default; 0 = flat, __elevk)
layout(location = 0) out uvec4 fragColor; // per-light u9 shadow coverage
layout(location = 1) out uvec4 oCasterD;  // #3: frontmost caster row (R, 7-bit) shadowing this texel
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
void main() {
  // P3/v2.1: window mapping from the CONSTANTS px (R = id|cols, G = rows|slot,
  // B = i16 winCol | i16 winRow — sign-extended halves). Updated through the command path.
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0xFFFFu);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlot, sy = fc.y / uSlot;                 // toroidal slot
  if (texelFetch(uDirty, ivec2(sx, sy), 0).r == 0u) discard; // clean tile → keep the persistent texel
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols); // → world tile
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlot)) / float(uSlot); // 0..1 within the tile
  float ly = (float(fc.y) - float(sy * uSlot)) / float(uSlot);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS (true texel position)
  // shadows-onto-prims: is a standing prim DRAWN at this texel, and its base row? IN-FAMILY (caster buckets
  // + surface atlas by index) — zoom-safe by construction, NOT the reverted zdepth-composite world read.
  uint rprim; float rcov;
  float baseY = receiverAt(P, uData, uSurface, vec2(${RECV_ALIGN_XF}, ${RECV_ALIGN_YF}), rprim, rcov); // shadow mask offset; rprim 0 = ground
  bool isThing = rprim != 0u;
  float maskCov = clamp(rcov, 0.0, 1.0);                     // SOFT mask coverage → blends ground↔thing at the silhouette edge
  // Fictional height of this billboard pixel above its OWN base (units); 0 for ground → the per-light ground
  // projection below makes the shadow CLIMB the billboard. Rbase = the receiver base (the front/behind axis).
  float zElev = isThing ? uElevK * max(0.0, baseY - P.y) : 0.0;
  vec2 Rbase = vec2(P.x, baseY);
  vec2 Pground = P; Pground.y += ${SHADOW_LIFTF}; // #2 lift — GROUND path only (thing path projects instead)

  int fold = foldTile(wc, wr);
  uvec4 presLo = fetchLin(uData, PRESENCE_BASE + fold);     // lights 0–6
  uvec4 presHi = fetchLin(uData, PRESENCE_HI_BASE + fold);  // lights 7–13
  uint o0 = 0u, o1 = 0u, o2 = 0u, o3 = 0u;                   // per-SLOT u8 coverage: slot i at ch i>>2, bit (i&3)*8
  float casterDepth = 0.0;                                   // #3: frontmost (max-row) covering caster (all lights)
  for (int slot = 0; slot < 14; slot++) {
    uint li = slot < 7 ? tileSlot(presLo, slot) : tileSlot(presHi, slot - 7);
    if (li == 0xffffu) continue;                            // empty slot
    uvec4 Ld = fetchLin(uData, LIGHT_BASE + int(li));       // v2.1: G = position, A = z|reach|emitter|hot|cast
    if ((Ld.w & 1u) == 0u) continue;                        // cast_shadows
    if (int((Ld.w >> 1) & 1u) != uLightClass) continue;     // #4: this pass handles only its class (cold/hot)
    vec3 L = vec3(decodePos(Ld.y), float((Ld.w >> 24) & 255u));
    float emitter = float((Ld.w >> 4) & 255u);              // emitter_radius (units) → penumbra width
    int reachT = int((Ld.w >> 12) & 0xfffu) / int(UPT) + 1; // reach (units) → tiles, +1 margin (brute box)
    // Sample the shadow at TWO points and blend by the mask coverage: the GROUND point (the lifted texel)
    // and the elevated THING point (the pixel projected to the ground along the light ray through it — the
    // climbing-billboard shadow). At the silhouette EDGE (0<cov<1) the pixel is partly sprite, partly the
    // ground behind it, so the blend is a real 50/50 there — no hard cut. cov==0 → ground only (identity
    // held); cov==1 → thing only (identity held). Each walk is corridor↔brute bit-identical.
    float cdG = 0.0, cdT = 0.0;
    float shG = (maskCov < 1.0)
      ? walkShadow(L, emitter, Pground, false, rprim, Rbase, uCorridor, reachT, uData, uSurface, cdG)
      : 0.0;
    float shT = 0.0;
    if (maskCov > 0.0) {
      float ze = min(zElev, 0.9 * L.z);                     // keep the projection s bounded
      float sProj = L.z / (L.z - ze);
      vec2 Qt = (sProj > 0.0) ? L.xy + sProj * (P - L.xy) : P;
      shT = walkShadow(L, emitter, Qt, true, rprim, Rbase, uCorridor, reachT, uData, uSurface, cdT);
    }
    float cov = mix(shG, shT, maskCov);
    casterDepth = max(casterDepth, max(maskCov < 1.0 ? cdG : 0.0, maskCov > 0.0 ? cdT : 0.0)); // #3 (vestigial att1)
    // Pack this SLOT's coverage as u9 (0..511): low8 at channel slot>>2 bits (slot&3)*8; the 9th
    // (high) bit into A at bit 16+slot (14 high bits in A[16:30]). No channel straddle.
    uint v = uint(clamp(cov, 0.0, 1.0) * 511.0 + 0.5);
    uint low8 = (v & 0xFFu) << uint((slot & 3) * 8);
    int ch = slot >> 2;                                     // static branch (no dynamic write-subscript)
    if (ch == 0) o0 |= low8; else if (ch == 1) o1 |= low8; else if (ch == 2) o2 |= low8; else o3 |= low8;
    o3 |= ((v >> 8) & 1u) << uint(16 + slot);               // high bit → A[16+slot]
  }
  fragColor = uvec4(o0, o1, o2, o3);
  oCasterD = uvec4(uint(casterDepth) & 0x7Fu, 0u, 0u, 0u);  // #3: frontmost caster row (7-bit local key)
}
`;

/** Overlay: sample shadow-cold at the fragment's world position, decode set bits → per-light colours. */
const OVERLAY_VERT = /* glsl */ `#version 300 es
in vec2 aWorld;                  // world px (window rect)
uniform mat3 uProjection;
out vec2 vWorld;
void main() {
  vWorld = aWorld;
  vec3 p = uProjection * vec3(aWorld, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const OVERLAY_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
in vec2 vWorld;
uniform highp usampler2D uShadow;    // COLD shadow RT — per-slot u9 coverage (cold lights' slots)
uniform highp usampler2D uShadowHot; // HOT shadow RT — hot lights' slots (#4: a slot lives in exactly one class)
uniform highp usampler2D uData;     // presence rides the unified data texture (region-torus fold)
uniform int uCols, uRows, uWinCol, uWinRow, uSlot; // window mapping — the overlay is a per-draw
                                                   // DISPLAY consumer (F2: uniforms are its lane)
out vec4 fragColor;
int pmod(int a, int m) { return ((a % m) + m) % m; }
const int PRESENCE_BASE = 196608, PRESENCE_HI_BASE = 327680;
uvec4 fetchLin(highp usampler2D t, int i) { return texelFetch(t, ivec2(i & 1023, i >> 10), 0); }
int fmod16(int v) { return ((v % 16) + 16) % 16; }
int foldTile(int wc, int wr) {
  int zx = fmod16(wc >> 4), zy = fmod16(wr >> 4), tx = fmod16(wc), ty = fmod16(wr);
  return ((zx >> 2) + zy * 4) * 1024 + (zx & 3) * 256 + ty * 16 + tx;
}
uint laneN(uvec4 v, int c) { return c == 1 ? v.y : (c == 2 ? v.z : v.w); }
uint lane4(uvec4 v, int c) { return c == 0 ? v.x : (c == 1 ? v.y : (c == 2 ? v.z : v.w)); }
uint tileSlot(uvec4 t, int i) {
  if (i == 0) return t.x & 0xffffu;
  uint w = laneN(t, (i + 1) >> 1);
  return (i & 1) == 1 ? (w >> 16) : (w & 0xffffu);
}
// Per-light colour — MUST match LIGHT_COLORS (the gizmo rings) so a light's shadow reads as the same
// colour as its ring while debugging. (Debug: only the first 6 lights; overlap sums.)
vec3 lightColour(int k) {
  if (k == 0) return vec3(1.0, 0.25, 0.25);   // red
  if (k == 1) return vec3(0.25, 1.0, 0.3);    // green
  if (k == 2) return vec3(0.3, 0.55, 1.0);    // blue
  if (k == 3) return vec3(1.0, 0.95, 0.25);   // yellow
  if (k == 4) return vec3(1.0, 0.35, 1.0);    // magenta
  return vec3(0.3, 1.0, 1.0);                 // cyan
}
void main() {
  int tx = int(floor(vWorld.x / ${SQF})), ty = int(floor(vWorld.y / ${SQF}));
  if (tx < uWinCol || tx >= uWinCol + uCols || ty < uWinRow || ty >= uWinRow + uRows) { fragColor = vec4(0.0); return; }
  int sx = pmod(tx, uCols), sy = pmod(ty, uRows);
  float lx = fract(vWorld.x / ${SQF}), ly = fract(vWorld.y / ${SQF});
  ivec2 texel = ivec2(sx * uSlot + int(lx * float(uSlot)), sy * uSlot + int(ly * float(uSlot)));
  int fold = foldTile(tx, ty);
  uvec4 presLo = fetchLin(uData, PRESENCE_BASE + fold);     // lights 0–6
  uvec4 presHi = fetchLin(uData, PRESENCE_HI_BASE + fold);  // lights 7–13
  uvec4 shC = texelFetch(uShadow, texel, 0);                // cold class slots
  uvec4 shH = texelFetch(uShadowHot, texel, 0);             // hot class slots (a slot is nonzero in one only)
  vec3 acc = vec3(0.0);
  float any = 0.0;
  for (int slot = 0; slot < 14; slot++) {
    uint li = slot < 7 ? tileSlot(presLo, slot) : tileSlot(presHi, slot - 7);
    if (li == 0xffffu) continue;                            // empty slot
    uint vC = ((lane4(shC, slot >> 2) >> uint((slot & 3) * 8)) & 0xFFu) | (((shC.w >> uint(16 + slot)) & 1u) << 8);
    uint vH = ((lane4(shH, slot >> 2) >> uint((slot & 3) * 8)) & 0xFFu) | (((shH.w >> uint(16 + slot)) & 1u) << 8);
    uint v = max(vC, vH);                                    // u9 — combine the two classes' RTs
    if (v > 0u) { float cvg = float(v) / 511.0; acc += lightColour(int(li)) * cvg; any = max(any, cvg); }
  }
  if (any <= 0.0) { fragColor = vec4(0.0); return; }        // lit → transparent
  fragColor = vec4(clamp(acc, 0.0, 1.0), any);              // shadow tint × coverage
}
`;

/** Light gizmo: a filled dot at the light + a ring at its radius (screen-constant thickness). */
const GIZMO_VERT = /* glsl */ `#version 300 es
in vec2 aUnit;                   // 0..1 quad
uniform vec2 uCenter;            // light world px
uniform float uHalf;             // quad half-extent (world px) — covers the radius
uniform mat3 uProjection;
out vec2 vWorld;
void main() {
  vWorld = uCenter + (aUnit * 2.0 - 1.0) * uHalf;
  vec3 p = uProjection * vec3(vWorld, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const GIZMO_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vWorld;
uniform vec2 uCenter;
uniform float uRadius;           // world px
uniform float uPxWorld;          // 1 screen px in world px (= 1/zoom) → constant on-screen thickness
uniform vec3 uColor;
out vec4 fragColor;
void main() {
  float dist = length(vWorld - uCenter);
  if (dist < 6.0 * uPxWorld) { fragColor = vec4(uColor, 1.0); return; }              // light dot
  if (abs(dist - uRadius) < 1.5 * uPxWorld) { fragColor = vec4(uColor, 0.9); return; } // radius ring
  discard;
}
`;

const LIGHT_COLORS: ReadonlyArray<[number, number, number]> = [
  [1.0, 0.25, 0.25], [0.25, 1.0, 0.3], [0.3, 0.55, 1.0], [1.0, 0.95, 0.25], [1.0, 0.35, 1.0], [0.3, 1.0, 1.0],
];

/**
 * Owns the light list, the cold data textures, the world-space toroidal `shadow-cold` bitfield, and the
 * gather pass that writes it. {@link tick} recomputes the bitfield each frame (P4-core); {@link drawOverlay}
 * decodes it over the world for `/overlayRT shadow-cold`.
 */
export class ShadowGather {
  private readonly gather: Program;
  /** The LIGHTING pass — bakes the world-space toroidal lightmap (P1+, `2026-07-23-lighting`). */
  private readonly lighting: Program;
  private readonly overlay: Program;
  private readonly gizmo: Program;
  private readonly fsQuad: Geometry;
  private readonly gizmoQuad: Geometry;
  private overlayGeo: Geometry | null = null;
  private readonly overlayPos = new Float32Array(8);
  private readonly lights: Light[] = [];
  /** DEBUG: orbit the lights every frame so the shadow recompute runs each frame — turns the FPS
   *  panel into a live gather-cost readout (the "hot lights" perf case). Toggle with {@link setOrbit}. */
  private orbit = false;            // orbit disabled — lights are static
  private orbitPhase = 0;
  private seedX = 0;
  private seedY = 0;
  private readonly empty: Texture;
  private enabled = true;
  /** shadows-onto-prims: receiver-elevation gain (sin65 by the world-geometry model). `__elevk` tunes the
   *  climb rate by eye; 0 collapses prim shadows to the flat ground shadow. */
  private elevK = ELEV_K_DEFAULT;
  /** world-space-lighting (P1): TRUE-3D elliptical falloff (`__worldlight`) + the live-tunable N–S
   *  un-foreshorten factor (`__nsfactor`, default 1/cos65). Default ON so the corrected look shows; toggle
   *  off to A/B against the screen circle. */
  private worldLight = true;
  private nsInv = INV_COS_TILT_DEFAULT;
  /** lightmap P0 debug (`__shownormal`): paint the atlas-frame prim normal into the lightmap to verify the
   *  read (per-prim, zoom-stable). Off in normal operation. */
  private showNormal = false;
  /** lightmap: the standing-billboard normal pitch (deg, `__pitchnormal`; default 90°−tilt, re-derived by
   *  `__tilt`). The corpus normals are RAW (camera-facing) and the world pitch is applied IN-SHADER — kept
   *  live rather than bake-committed (F7 reconsidered): it's ~free in the dirty-gated bake and keeps the
   *  world angle adjustable. Ground (rprim 0) is unpitched (flat-up); things pitch to perpendicular. */
  private normalPitchDeg = 90 - WORLD_TILT_DEG;
  /** lightmap: the LIGHTING receiver-mask seat (units, NW shift), DECOUPLED from the shadow's `RECV_ALIGN`.
   *  The shadow's (0.5, 1.5) offset clipped the lit sprite's SE edge (bottom + right) — the lighting coverage
   *  aligns to the RAW def anchor (the albedo draw), so this is 0. `__lightalign(x, y)` re-tunes by eye. */
  private lightAlignX = 0;
  private lightAlignY = 0;
  /** shadow-edge-refine (`__edgerefine`): re-test the caster silhouette at fine res on `(0,1)` edge texels so
   *  the cast shadow's edge is near-pixel-perfect instead of the coarse 16/tile blocks. Default ON; A/B off. */
  private edgeRefine = true;
  /** DEBUG (`__hideright`): blank the right half of each prim's normal in `__shownormal` (alignment probe). */
  private hideRight = false;
  /** The LIVE world ground tilt (degrees) — written into the data map constants each frame so the shadow
   *  projection (and any shader) reads the angle without a uniform. `__tilt(deg)` re-tilts the whole model:
   *  it updates this + the derived elevation gain (`sin`) + the falloff N–S factor (`1/cos`). */
  private worldTiltDeg = WORLD_TILT_DEG;
  private readonly coldData: ColdShadowData;
  private lastCasterCount = -1;
  private coldDirty = true;

  /** shadow-cold RT — the **textile_unit map** (`cols·TEXTILE_UNIT × rows·TEXTILE_UNIT` RGBA32UI,
   *  world-space toroidal), resized when the tile window changes. */
  private rtCols = 0;
  private rtRows = 0;
  /** #4 COLD/HOT SPLIT — static (cold) and dynamic (hot) lights bake into SEPARATE shadow + lightmap RTs,
   *  gated by independent dirty textures, so a moving hot light never re-bakes the static lights. All are
   *  world-space toroidal + TEXEL-aligned. shadow = RGBA32UI (per-light u9 coverage); lightmap = MRT
   *  [irradiance rgba8, aggregate-dir rgba8]. The display blit sums cold+hot (+ ambient once). */
  private coldShadowRT: RenderTarget | null = null;
  private hotShadowRT: RenderTarget | null = null;
  private coldLightRT: RenderTarget | null = null;
  private hotLightRT: RenderTarget | null = null;

  /** light presence — now folded into the data texture (set 3, region-torus). CPU scratch only: per
   *  in-window tile the **nearest 7** `u16` light indices (`0xFFFF` = empty); `slotDist` tracks each
   *  slot's distance² for nearest-N eviction. `buildPresence` writes each tile via `coldData.writePresence`
   *  (compare-write diffs → scatter). */
  private presSlots = new Uint16Array(0);
  private slotDist = new Float32Array(0);
  private presSig = "";
  private lightsVer = 0;

  /** caster buckets — folded into the data texture (set 4, region-torus). CPU scratch: per in-window
   *  tile ≤7 `u16` prim indices (`0` = empty); `castCount` the per-tile fill. `buildCasters` writes each
   *  tile via `coldData.writeCasters`. */
  private castSlots = new Uint16Array(0);
  private castCount = new Uint8Array(0);

  /** #4 shadow_dirty — TWO **textile_tile maps** (`R8UI`, nonzero = recompute), one per class (cold/hot).
   *  Owner tracking is SHARED (pan exposes tiles for both). A COLD tile recomputes on a static-light or
   *  caster change or pan; a HOT tile on a dynamic-light move, caster change, or pan. The common frame
   *  (only the green light moves) dirties HOT only → the cold shadow + lightmap are never re-baked. */
  private coldDirtyTex: Texture;
  private hotDirtyTex: Texture;
  private coldMirror = new Uint8Array(0);
  private hotMirror = new Uint8Array(0);
  private ownerCol = new Int32Array(0);
  private ownerRow = new Int32Array(0);
  private slotValid = new Uint8Array(0);
  private dirtyCols = 0;
  private dirtyRows = 0;
  /** Sticky "recompute every tile next build", per class — set on a rebuild; survives a not-ready frame. */
  private forceColdDirty = true;
  private forceHotDirty = true;
  /** P6: walk the segment corridor (true) or the brute-force reach box (false). Brute is the
   *  validation baseline — `__corridor(false)` + `__shadowDiff()` must report 0 mismatches. */
  private corridor = true;
  /** P5 scoped dirty: world-tile rects `[x0,y0,x1,y1,cls]` (inclusive) queued by a light MOVE or caster
   *  change — the union of old + new reach. `cls`: 0 = cold only, 1 = hot only, 2 = both (a caster change
   *  affects every light that reaches it). Applied (∩ window) per class on top of the owner pass in
   *  {@link buildDirty}, then cleared. A moved light only changes shadow inside its old ∪ new reach. */
  private pendingRects: [number, number, number, number, number][] = [];

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.gather = new Program(gl, FULLSCREEN_VERT, GATHER_FRAG, "shadow-gather");
    this.lighting = new Program(gl, FULLSCREEN_VERT, LIGHT_FRAG, "world-lighting");
    this.overlay = new Program(gl, OVERLAY_VERT, OVERLAY_FRAG, "shadow-overlay");
    this.gizmo = new Program(gl, GIZMO_VERT, GIZMO_FRAG, "shadow-gizmo");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    this.coldDirtyTex = new Texture(gl, { width: 1, height: 1, format: "r8uint" });
    this.hotDirtyTex = new Texture(gl, { width: 1, height: 1, format: "r8uint" });
    this.coldData = new ColdShadowData(renderer);
    (globalThis as unknown as { __cold: unknown }).__cold = this.coldData; // DEBUG
    // DEBUG: console toggle for the orbit (perf measurement) — `__orbit(false)` to freeze the lights.
    (globalThis as unknown as { __orbit: (on?: boolean) => boolean }).__orbit = (on?: boolean) => this.setOrbit(on);
    // DEBUG (P6): corridor↔brute toggle + the gather itself (for `debugReadShadow` diffing).
    (globalThis as unknown as { __corridor: (on?: boolean) => boolean }).__corridor = (on?: boolean) => this.setCorridor(on);
    (globalThis as unknown as { __gather: ShadowGather }).__gather = this;
    // DEBUG: tune the emitter (area-light) RADIUS in world px live — bigger = softer penumbra.
    (globalThis as unknown as { __emit: (px?: number) => number }).__emit = (px?: number) => this.setEmitter(px);
    // DEBUG (shadows-onto-prims): tune the receiver-elevation gain live — bigger = the prim shadow climbs
    // faster/higher up a billboard; 0 = flat ground shadow (the A/B baseline).
    (globalThis as unknown as { __elevk: (k?: number) => number }).__elevk = (k?: number) => this.setElevK(k);
    // DEBUG (world-space-lighting): toggle true-3D elliptical falloff vs the screen circle; tune the N–S
    // un-foreshorten factor (default 1/cos65 ≈ 2.366) by eye — the sim-check on cos65 vs sin65.
    (globalThis as unknown as { __worldlight: (on?: boolean) => boolean }).__worldlight = (on?: boolean) => {
      this.worldLight = on ?? !this.worldLight;
      this.forceColdDirty = this.forceHotDirty = true;
      return this.worldLight;
    };
    (globalThis as unknown as { __nsfactor: (f?: number) => number }).__nsfactor = (f?: number) => {
      if (f !== undefined) { this.nsInv = f; this.forceColdDirty = this.forceHotDirty = true; }
      return this.nsInv;
    };
    // DEBUG (world-space-lighting): the ONE ground-angle dial. Sets the data-map tilt (shadow projection +
    // caster lean) AND re-derives the elevation gain (sin), the falloff N–S factor (1/cos), AND the normal
    // pitch (90°−tilt) so the WHOLE geometry — including the in-shader normal pitch — re-tilts coherently.
    // (lightmap F7 reconsidered: the pitch stays in-shader, kept live, NOT bake-committed — it costs ~nothing
    // in the dirty-gated bake, and this keeps the world angle a live knob.) No rebuild, no burned uniform.
    (globalThis as unknown as { __tilt: (deg?: number) => number }).__tilt = (deg?: number) => {
      if (deg !== undefined) {
        this.worldTiltDeg = deg;
        const rad = deg * Math.PI / 180;
        this.elevK = Math.sin(rad);
        this.nsInv = 1 / Math.cos(rad);
        this.normalPitchDeg = 90 - deg;   // standing billboard: flat card → perpendicular-to-ground normal
        this.forceColdDirty = this.forceHotDirty = true;
      }
      return this.worldTiltDeg;
    };
    // DEBUG (lightmap P0): paint the atlas-frame prim normal into the lightmap — verify the read is per-prim
    // and rock-stable across zoom (frame-indexed, not a world-coord composite read).
    (globalThis as unknown as { __shownormal: (on?: boolean) => boolean }).__shownormal = (on?: boolean) => {
      this.showNormal = on ?? !this.showNormal;
      this.forceColdDirty = this.forceHotDirty = true;
      return this.showNormal;
    };
    // DEBUG (lightmap P1): the DEV in-shader normal pitch (deg) — eyeball the sign/magnitude while the corpus
    // is still raw; default 90−tilt. Bake-at-ingest (F7) retires this.
    (globalThis as unknown as { __pitchnormal: (deg?: number) => number }).__pitchnormal = (deg?: number) => {
      if (deg !== undefined) { this.normalPitchDeg = deg; this.forceColdDirty = this.forceHotDirty = true; }
      return this.normalPitchDeg;
    };
    // DEBUG (lightmap): the LIGHTING receiver-mask seat (units NW), decoupled from the shadow's RECV_ALIGN —
    // dial by eye to stop the lit sprite clipping its bottom/right edge. Returns [x, y].
    (globalThis as unknown as { __lightalign: (x?: number, y?: number) => number[] }).__lightalign = (x?: number, y?: number) => {
      if (x !== undefined) this.lightAlignX = x;
      if (y !== undefined) this.lightAlignY = y;
      this.forceColdDirty = this.forceHotDirty = true;
      return [this.lightAlignX, this.lightAlignY];
    };
    // DEBUG (shadow-edge-refine): A/B the fine caster-silhouette re-test (on) vs the coarse nearest shadow (off).
    (globalThis as unknown as { __edgerefine: (on?: boolean) => boolean }).__edgerefine = (on?: boolean) => {
      this.edgeRefine = on ?? !this.edgeRefine;
      this.forceColdDirty = this.forceHotDirty = true;
      return this.edgeRefine;
    };
    // DEBUG (__hideright): blank the right half of each prim's normal in __shownormal (alignment probe).
    (globalThis as unknown as { __hideright: (on?: boolean) => boolean }).__hideright = (on?: boolean) => {
      this.hideRight = on ?? !this.hideRight;
      this.forceColdDirty = this.forceHotDirty = true;
      return this.hideRight;
    };
    // DEBUG (lightmap P3): scatter n STATIC lights in a grid around the seed to stress the dirty-gated bake —
    // static lights bake ONCE then never re-bake, so fps should hold (the many-lights caching payoff). Returns
    // the new total light count (capped at the N_LIGHTS bitfield ceiling).
    (globalThis as unknown as { __manylights: (n?: number) => number }).__manylights = (n = 24) => {
      const room = N_LIGHTS - this.lights.length;
      const add = Math.max(0, Math.min(n, room));
      const side = Math.max(1, Math.ceil(Math.sqrt(add)));
      const step = 4 * SQUARE;                       // 4 tiles apart
      const ox = this.seedX - (side - 1) * step / 2, oy = this.seedY - (side - 1) * step / 2;
      let done = 0;
      for (let i = 0; i < side && done < add; i++) {
        for (let j = 0; j < side && done < add; j++, done++) {
          const lx = ox + j * step, ly = oy + i * step;
          this.lights.push({ x: lx, y: ly, hx: lx, hy: ly, z: LIGHT_Z, reach: LIGHT_REACH, emitterRadius: LIGHT_EMITTER, dynamic: false });
        }
      }
      this.lightsVer++;
      this.coldDirty = true;
      this.forceColdDirty = this.forceHotDirty = true;
      return this.lights.length;
    };
    this.fsQuad = new Geometry(gl, this.gather, {
      aPos: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.overlayGeo = new Geometry(gl, this.overlay, {
      aWorld: { data: this.overlayPos, size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.gizmoQuad = new Geometry(gl, this.gizmo, {
      aUnit: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.seed(54, 21);
  }

  /** Seed the debug lights: one at the seed tile + the {@link EXTRA_LIGHT_TILES} fixed set (world px). */
  seed(tileX: number, tileY: number): void {
    const cx = (tileX + 0.5) * SQUARE, cy = (tileY + 0.5) * SQUARE;
    this.seedX = cx; this.seedY = cy;
    this.lights.length = 0;
    this.lights.push({ x: cx, y: cy, hx: cx, hy: cy, z: LIGHT_Z, reach: LIGHT_REACH, emitterRadius: LIGHT_EMITTER, dynamic: false });
    for (const [tx, ty, dynamic, emitter] of EXTRA_LIGHT_TILES) {
      const lx = (tx + 0.5) * SQUARE, ly = (ty + 0.5) * SQUARE;
      this.lights.push({ x: lx, y: ly, hx: lx, hy: ly, z: LIGHT_Z, reach: LIGHT_REACH, emitterRadius: emitter, dynamic });
    }
    this.enabled = true;
    this.coldDirty = true;
    this.lightsVer++; // invalidates light_presence_cold
    this.forceColdDirty = this.forceHotDirty = true; // lights changed → recompute every tile
  }

  /** Rebuild the per-tile **presence** (P5) when the lights or window change: each tile gets the
   *  **nearest 8** lights whose (reach−1, F10) circle covers it, as `u16` indices (`0xFFFF` = empty).
   *  Nearest-N eviction via `slotDist`. This bounds the gather to O(8) lights/texel. */
  private buildPresence(win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    const sig = `${winCol},${winRow},${cols},${rows},${this.lightsVer}`;
    if (sig === this.presSig && this.presSlots.length === cols * rows * PRES_SLOTS) return;
    this.presSig = sig;
    if (this.presSlots.length !== cols * rows * PRES_SLOTS) {
      this.presSlots = new Uint16Array(cols * rows * PRES_SLOTS);
      this.slotDist = new Float32Array(cols * rows * PRES_SLOTS);
    }
    this.presSlots.fill(0xffff); // every slot empty
    this.slotDist.fill(Infinity);
    // Nearest-14 per in-window tile (dense window-local scratch; the GPU slot is the region-torus fold).
    const n = Math.min(this.lights.length, MAX_LIGHTS);
    for (let k = 0; k < n; k++) {
      const L = this.lights[k], r = L.reach;
      if (r <= 0) continue;
      const r2 = r * r;
      const t0x = Math.max(winCol, Math.floor((L.x - r) / SQUARE)), t1x = Math.min(winCol + cols - 1, Math.floor((L.x + r) / SQUARE));
      const t0y = Math.max(winRow, Math.floor((L.y - r) / SQUARE)), t1y = Math.min(winRow + rows - 1, Math.floor((L.y + r) / SQUARE));
      for (let wr = t0y; wr <= t1y; wr++) {
        const cy = (wr + 0.5) * SQUARE;
        for (let wc = t0x; wc <= t1x; wc++) {
          const cx = (wc + 0.5) * SQUARE;
          const d2 = (cx - L.x) * (cx - L.x) + (cy - L.y) * (cy - L.y);
          if (d2 > r2) continue; // circular reach
          const ti = (wr - winRow) * cols + (wc - winCol); // dense window-local tile
          let worst = -1, worstD = d2;
          for (let s = 0; s < PRES_SLOTS; s++) { const sd = this.slotDist[ti * PRES_SLOTS + s]; if (sd > worstD) { worstD = sd; worst = s; } }
          if (worst < 0) continue; // all 14 slots already closer → drop this light for this tile
          this.slotDist[ti * PRES_SLOTS + worst] = d2;
          this.presSlots[ti * PRES_SLOTS + worst] = k & 0xffff;
        }
      }
    }
    // Write EVERY in-window tile (compare-write diffs — empty tiles clear, changed tiles command).
    for (let wr = winRow; wr < winRow + rows; wr++)
      for (let wc = winCol; wc < winCol + cols; wc++) {
        const ti = (wr - winRow) * cols + (wc - winCol);
        this.coldData.writePresence(wc, wr, this.presSlots.subarray(ti * PRES_SLOTS, ti * PRES_SLOTS + PRES_SLOTS));
      }
  }

  /** Rebuild the per-tile **caster buckets** (P1): allocate each standing caster's def+prim, then
   *  drop its `prim_data` index into every tile of its **base line** (anchor row × width cols) that
   *  the window covers, up to 8/tile. The corridor sweep reads these; casters are bucketed by where
   *  they *stand* (ground base), not the billboard's aerial bbox. Cheap enough to rebuild each frame
   *  for now; the O(1) re-bucket on move lands with the hot tier. */
  private buildCasters(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    if (this.castSlots.length !== cols * rows * 7) {
      this.castSlots = new Uint16Array(cols * rows * 7);
      this.castCount = new Uint8Array(cols * rows);
    }
    this.castSlots.fill(0); // 0 = empty (prim sentinel)
    this.castCount.fill(0);
    const count = this.castCount;
    // The card is TILTED back 65° (see the shader), so its ground footprint spans from the base
    // (anchor y) UP to the top (anchor y − 0.5·H·cos65). The corridor crosses the caster anywhere in
    // that y-range (far shadow ↔ top, near ↔ base), so we must bucket every row it spans — bucketing
    // only the base row missed casters whose top sits in the row above (I-7).
    const TILT = 0.5 * Math.cos(this.worldTiltDeg * Math.PI / 180); // 0.5·cos(tilt) of the height, leaned back (live tilt)
    this.litSeen.clear();
    const seen = this.primSeen;
    seen.clear();
    for (const p of standing) {
      const def = this.coldData.definitionFor(p, resolver);
      if (def < 0) continue; // no textureName → not a caster
      const inst = this.coldData.primDataFor(p, def);
      seen.add(p.id); // resident this frame — everything else gets freed (P2)
      // Bucket by the TIGHT opaque bbox (P3), not the full prim box — matches the quad we actually cast.
      // A flipped (W-facing) prim mirrors the box within the prim rect, same as the gather mirrors `off.x`.
      const t = this.coldData.tightBoxOf(def) ?? { dx: 0, dy: 0, w: p.width, h: p.height };
      const tdx = p.flipX ? p.width - (t.dx + t.w) : t.dx;
      const tx = p.x + tdx, ty = p.y + t.dy;
      // A changed prim (new immutable def — lod landed/zoom — or first sight) cascades its region.
      if (inst.changed) this.markPrimChange(tx, ty, t.w, t.h);
      const baseY = ty + t.h, topY = baseY - TILT * t.h; // card ground y-extent (px)
      const r0 = Math.floor(topY / SQUARE), r1 = Math.floor(baseY / SQUARE);
      const c0 = Math.floor(tx / SQUARE), c1 = Math.floor((tx + t.w) / SQUARE);
      for (let wr = r0; wr <= r1; wr++) {
        if (wr < winRow || wr >= winRow + rows) continue;
        for (let wc = c0; wc <= c1; wc++) {
          if (wc < winCol || wc >= winCol + cols) continue;
          const ti = (wr - winRow) * cols + (wc - winCol); // dense window-local tile
          const n = count[ti];
          if (n >= 7) continue; // tile full — drop the rest (rare)
          this.castSlots[ti * 7 + n] = inst.idx & 0xffff;
          count[ti] = n + 1;
        }
      }
    }
    // Prims that left `standing` (zone evicted / destroyed) free their slots (P2 free-list); their
    // buckets already clear via the rebuild below, and caster removal force-alls the recompute.
    this.coldData.freePrimsExcept(seen);
    // Write EVERY in-window tile (compare-write diffs). The region-torus fold is the GPU slot.
    for (let wr = winRow; wr < winRow + rows; wr++)
      for (let wc = winCol; wc < winCol + cols; wc++) {
        const ti = (wr - winRow) * cols + (wc - winCol);
        this.coldData.writeCasters(wc, wr, this.castSlots.subarray(ti * 7, ti * 7 + 7));
      }
    // Flush all queued writes (def/prim/presence/caster) as ONE scatter batch. NO force-all: def
    // swaps + new prims queued their scoped rects via the prim dirty cascade (markPrimChange).
    this.coldData.flush();
  }

  /** Queue the scoped dirty rect for a light move (P5): the union box of the old + new reach,
   *  +1 tile margin, in world tiles. `cls` = which class it dirties (0 cold / 1 hot / 2 both — #4).
   *  Applied by {@link buildDirty} this frame. */
  private markLightMove(ox: number, oy: number, nx: number, ny: number, reach: number, cls: number): void {
    const x0 = Math.floor((Math.min(ox, nx) - reach) / SQUARE) - 1;
    const y0 = Math.floor((Math.min(oy, ny) - reach) / SQUARE) - 1;
    const x1 = Math.floor((Math.max(ox, nx) + reach) / SQUARE) + 1;
    const y1 = Math.floor((Math.max(oy, ny) + reach) / SQUARE) + 1;
    this.pendingRects.push([x0, y0, x1, y1, cls]);
  }

  /** The PRIM DIRTY CASCADE applied to texture/def changes: a prim changed (new immutable def —
   *  lod landed/zoomed — or first sight) → dirty the prim's own tiles, then every light whose reach
   *  touches them, then those lights' full cast regions (their reach boxes — a shadow texel is only
   *  written inside its light's presence, so the reach box bounds the cast). `x/y/w/h` = the prim's
   *  tight box in world px. `litSeen` dedupes light boxes within a frame (streaming floods). */
  private readonly litSeen = new Set<number>();
  /** Prim ids resident this frame (P2) — anything allocated but absent gets freed via the free-list. */
  private readonly primSeen = new Set<number>();
  /** DEBUG: shadow tiles marked dirty (recomputed) on the last frame. */
  debugDirtyTiles = 0;
  private markPrimChange(x: number, y: number, w: number, h: number): void {
    const x0 = Math.floor(x / SQUARE) - 1, y0 = Math.floor(y / SQUARE) - 1;
    const x1 = Math.floor((x + w) / SQUARE) + 1, y1 = Math.floor((y + h) / SQUARE) + 1;
    this.pendingRects.push([x0, y0, x1, y1, 2]); // a caster affects BOTH classes at its own tiles
    for (let k = 0; k < this.lights.length; k++) {
      if (this.litSeen.has(k)) continue;
      const L = this.lights[k];
      const cx = Math.min(Math.max(L.x, x), x + w), cy = Math.min(Math.max(L.y, y), y + h);
      if (Math.hypot(cx - L.x, cy - L.y) > L.reach + SQUARE) continue; // light can't see the prim
      this.litSeen.add(k);
      this.markLightMove(L.x, L.y, L.x, L.y, L.reach, L.dynamic ? 1 : 0); // that light's class + cast region
    }
  }

  /** Rebuild `shadow_dirty` for this frame: a slot is dirty when its world-tile **owner changed** (pan /
   *  resize), `forceAll` (the cold data rebuilt — lights/casters changed), or it falls in a queued
   *  light-move rect (P5 scoped dirty). Clean slots `discard` in the gather, so `shadow-cold` persists
   *  (F6). Mirrors `SquareCache.markStale`'s per-slot owner tracking. */
  private buildDirty(win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    let forceCold = this.forceColdDirty, forceHot = this.forceHotDirty;
    this.forceColdDirty = false; this.forceHotDirty = false;
    if (this.dirtyCols !== cols || this.dirtyRows !== rows) {
      this.coldDirtyTex.destroy(); this.hotDirtyTex.destroy();
      this.coldDirtyTex = new Texture(this.renderer.gl, { width: cols, height: rows, format: "r8uint" });
      this.hotDirtyTex = new Texture(this.renderer.gl, { width: cols, height: rows, format: "r8uint" });
      this.coldMirror = new Uint8Array(cols * rows);
      this.hotMirror = new Uint8Array(cols * rows);
      this.ownerCol = new Int32Array(cols * rows);
      this.ownerRow = new Int32Array(cols * rows);
      this.slotValid = new Uint8Array(cols * rows);
      this.dirtyCols = cols;
      this.dirtyRows = rows;
      forceCold = forceHot = true;
    }
    const pm = (a: number, m: number): number => ((a % m) + m) % m;
    const baseC = pm(winCol, cols), baseR = pm(winRow, rows);
    this.coldMirror.fill(0); this.hotMirror.fill(0);
    for (let sy = 0; sy < rows; sy++) {
      const wr = winRow + pm(sy - baseR, rows);
      for (let sx = 0; sx < cols; sx++) {
        const wc = winCol + pm(sx - baseC, cols);
        const si = sy * cols + sx;
        // Owner change (pan/resize) exposes a NEW world tile → dirty BOTH classes there + adopt the owner.
        const ownerChanged = this.slotValid[si] === 0 || this.ownerCol[si] !== wc || this.ownerRow[si] !== wr;
        if (ownerChanged) { this.ownerCol[si] = wc; this.ownerRow[si] = wr; this.slotValid[si] = 1; }
        if (forceCold || ownerChanged) this.coldMirror[si] = 1;
        if (forceHot || ownerChanged) this.hotMirror[si] = 1;
      }
    }
    // P5 scoped dirty: mark every window tile inside a queued rect, into its class(es) (cls 0/1/2).
    for (const [x0, y0, x1, y1, cls] of this.pendingRects) {
      const c0 = Math.max(x0, winCol), c1 = Math.min(x1, winCol + cols - 1);
      const r0 = Math.max(y0, winRow), r1 = Math.min(y1, winRow + rows - 1);
      for (let wr = r0; wr <= r1; wr++)
        for (let wc = c0; wc <= c1; wc++) {
          const idx = pm(wr, rows) * cols + pm(wc, cols);
          if (cls !== 1) this.coldMirror[idx] = 1; // cold (0) or both (2)
          if (cls !== 0) this.hotMirror[idx] = 1;  // hot (1) or both (2)
        }
    }
    this.pendingRects.length = 0;
    let dc = 0; for (let i = 0; i < this.coldMirror.length; i++) if (this.coldMirror[i] || this.hotMirror[i]) dc++;
    this.debugDirtyTiles = dc; // DEBUG: tiles recomputed this frame (cold ∪ hot)
    this.coldDirtyTex.upload(this.coldMirror);
    this.hotDirtyTex.upload(this.hotMirror);
  }

  get on(): boolean {
    return this.enabled;
  }
  /** DEBUG: turn the light orbit on/off (no arg = toggle). Off freezes the lights so the scene is
   *  static again (gather goes idle via dirty-gating); on drives a full recompute every frame. */
  setOrbit(on?: boolean): boolean {
    this.orbit = on ?? !this.orbit;
    if (!this.orbit) this.forceColdDirty = this.forceHotDirty = true; // one last clean recompute at the frozen positions
    return this.orbit;
  }
  /** P6: switch corridor ↔ brute walk (no arg = toggle) + recompute everything under the new path. */
  setCorridor(on?: boolean): boolean {
    this.corridor = on ?? !this.corridor;
    this.forceColdDirty = this.forceHotDirty = true;
    return this.corridor;
  }
  /** DEBUG: set the emitter (area-light) radius on every light, world px (no arg = read). Rebuilds the
   *  light records + recomputes every tile so the softer/harder penumbra takes immediately. */
  setEmitter(px?: number): number {
    if (px === undefined) return this.lights[0]?.emitterRadius ?? 0;
    for (const L of this.lights) L.emitterRadius = px;
    this.coldDirty = true;   // light records changed (emitter is in the record)
    this.forceColdDirty = this.forceHotDirty = true;  // recompute every shadow/light tile
    return px;
  }
  /** DEBUG (shadows-onto-prims): the receiver-elevation gain (no arg = read). Recomputes every tile. */
  setElevK(k?: number): number {
    if (k === undefined) return this.elevK;
    this.elevK = k;
    this.forceColdDirty = this.forceHotDirty = true;
    return this.elevK;
  }
  /** DEBUG (P6): read a class's shadow RT back (RGBA32UI) — the corridor↔brute identity diff. `cls` 1 =
   *  hot (default, the moving light), 0 = cold. Each class's gather is independently corridor-identical. */
  debugReadShadow(cls = 1): Uint32Array | null {
    const rt = cls === 0 ? this.coldShadowRT : this.hotShadowRT;
    if (!rt) return null;
    const gl = this.renderer.gl;
    const out = new Uint32Array(rt.width * rt.height * 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, rt.fbo);
    gl.readPixels(0, 0, rt.width, rt.height, gl.RGBA_INTEGER, gl.UNSIGNED_INT, out);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return out;
  }
  toggle(): boolean {
    this.enabled = !this.enabled;
    return this.enabled;
  }

  private ensureRT(cols: number, rows: number): void {
    if (this.coldShadowRT && cols === this.rtCols && rows === this.rtRows) return;
    this.coldShadowRT?.destroy(); this.hotShadowRT?.destroy();
    this.coldLightRT?.destroy(); this.hotLightRT?.destroy();
    const gl = this.renderer.gl;
    const w = Math.max(1, cols * TEXTILE_UNIT), h = Math.max(1, rows * TEXTILE_UNIT);
    // Shadow — MRT [0] per-light u9 coverage, [1] frontmost caster row (#3). One RT per class; clear both
    // attachments to 0 (known-zero persistence baseline, P3).
    this.coldShadowRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint", "rgba32uint"] });
    this.hotShadowRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint", "rgba32uint"] });
    for (const rt of [this.coldShadowRT, this.hotShadowRT]) {
      rt.bind();
      gl.clearBufferuiv(gl.COLOR, 0, new Uint32Array([0, 0, 0, 0]));
      gl.clearBufferuiv(gl.COLOR, 1, new Uint32Array([0, 0, 0, 0]));
    }
    // Lightmap — lightmap P1 Step B: a FINE (TEXTILE_SQUARE/tile) single-attachment map holding the fully
    // accumulated per-light irradiance (Σ colour·falloff·(1−shadow)·N·L). 4× per axis of the coarse shadow;
    // att1 (aggregate dir) + att2 (unshadowed) are gone — subsumed by baking N·L (F5). NO ambient baked (the
    // blit adds it once over cold+hot). fw/fh = cols/rows · TEXTILE_SQUARE.
    const fw = Math.max(1, cols * TEXTILE_SQUARE), fh = Math.max(1, rows * TEXTILE_SQUARE);
    this.coldLightRT = new RenderTarget(gl, { width: fw, height: fh, formats: ["rgba8unorm"] });
    this.hotLightRT = new RenderTarget(gl, { width: fw, height: fh, formats: ["rgba8unorm"] });
    for (const rt of [this.coldLightRT, this.hotLightRT]) {
      rt.bind();
      gl.clearBufferfv(gl.COLOR, 0, new Float32Array([0, 0, 0, 1]));       // irradiance = 0
    }
    this.rtCols = cols;
    this.rtRows = rows;
  }

  /** The baked COLD/HOT lightmap irradiance textures (FINE, world-space toroidal). Single attachment each —
   *  the per-light N·L is baked in (F5), so there's no separate dir/unshadowed. The blit samples both by world
   *  position and composites `ambient + cold + hot`. Null until the first bake has laid out the RTs. */
  get coldLightmap(): Texture | null { return this.coldLightRT?.textures[0] ?? null; }
  get hotLightmap(): Texture | null { return this.hotLightRT?.textures[0] ?? null; }

  /** Rebuild the cold data (on change) + recompute the DIRTY tiles of the shadow-cold bitfield. */
  tick(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    if (!this.enabled || this.lights.length === 0) return;

    // Motion. Default: only `dynamic` lights move (each orbits its own home) — the moving-light case
    // in a scene with static reference lights. `__orbit` (DEBUG) instead rings EVERY light around the
    // seed so the whole field recomputes each frame, reading the max per-frame gather cost.
    let moved = false;
    if (this.orbit) {
      this.orbitPhase += 0.01;
      const n = this.lights.length;
      for (let k = 0; k < n; k++) {
        const a = (k / n) * Math.PI * 2 + this.orbitPhase;
        const L = this.lights[k], ox = L.x, oy = L.y;
        L.x = this.seedX + Math.cos(a) * RING_RADIUS;
        L.y = this.seedY + Math.sin(a) * RING_RADIUS;
        this.markLightMove(ox, oy, L.x, L.y, L.reach, L.dynamic ? 1 : 0); // debug: each light dirties its class
      }
      moved = true;
    } else {
      this.orbitPhase += 0.02;
      for (const L of this.lights) {
        if (!L.dynamic) continue;
        const ox = L.x, oy = L.y;
        L.x = L.hx + Math.cos(this.orbitPhase) * MOTION_RADIUS;
        L.y = L.hy + Math.sin(this.orbitPhase) * MOTION_RADIUS;
        this.markLightMove(ox, oy, L.x, L.y, L.reach, 1); // dynamic → HOT dirty only (cold lights untouched)
        moved = true;
      }
    }
    if (moved) {
      this.coldDirty = true;   // light records changed
      this.lightsVer++;        // presence changed
    }
    if (this.coldDirty || standing.length !== this.lastCasterCount) {
      const coldLights = this.lights.slice(0, MAX_LIGHTS).map((L, k) => ({
        x: L.x, y: L.y, z: L.z, reach: L.reach, emitterRadius: L.emitterRadius,
        color: LIGHT_COLORS[k % LIGHT_COLORS.length], intensity: 1, castShadows: true,
        hot: L.dynamic, // #4: dynamic lights are HOT (per-frame), static are COLD (baked once)
      }));
      this.coldData.buildLights(coldLights);
      const removed = standing.length < this.lastCasterCount;
      const lightsChanged = this.coldDirty;
      this.lastCasterCount = standing.length;
      this.coldDirty = false;
      // Scoped dirty: a light MOVE queued its own rects; NEW casters cascade per prim
      // (markPrimChange on first sight). Force-all only for caster REMOVAL (stale shadows with no
      // owner to cascade from) or a light change with no scoped rects (e.g. a re-seed).
      if (removed || (lightsChanged && this.pendingRects.length === 0)) this.forceColdDirty = this.forceHotDirty = true;
    }
    if (this.coldData.lights === 0 || win.cols === 0) return;

    this.ensureRT(win.cols, win.rows);
    // P3: the window mapping rides the data texture's constants row (compare-written — an
    // unchanged window costs nothing), through the same scatter path as every other write.
    this.coldData.setConstants(win.cols, win.rows, win.winCol, win.winRow, TEXTILE_UNIT, this.coldData.lights, Math.round(this.worldTiltDeg * 100));
    this.buildPresence(win);                    // F5 per-tile light cull (rebuilt on light/window change)
    this.buildCasters(standing, resolver, win); // P1 per-tile caster buckets (the corridor reads these)
    this.buildDirty(win);    // #4: owner-change (pan) + per-class rebuild → cold/hot dirty; clean tiles persist
    // #4 COLD then HOT pass. Each is: gather (shadow-cold for that class) → lighting (its lightmap), gated
    // by that class's dirty texture (clean tiles `discard` → persist). A frame where only the green (hot)
    // light moves leaves cold-dirty EMPTY → both cold draws no-op (every fragment discards), so the static
    // lights + their shadows are never recomputed. Each gather is corridor↔brute identical within its class.
    this.classPass(0, this.coldDirtyTex, this.coldShadowRT!, this.coldLightRT!);
    this.classPass(1, this.hotDirtyTex, this.hotShadowRT!, this.hotLightRT!);
  }

  /** One class's shadow + lighting bake (#4). `cls` 0 = cold / 1 = hot; `dirty` gates it (clean → persist);
   *  the gather writes `shadowRT` (this class's per-light u9), the lighting reads it into `lightRT`. */
  private classPass(cls: number, dirty: Texture, shadowRT: RenderTarget, lightRT: RenderTarget): void {
    this.renderer.draw({
      program: this.gather,
      geometry: this.fsQuad,
      target: shadowRT,
      blend: "none",
      textures: {
        uData: this.coldData.dataTexture, // defs | prims | lights | presence | buckets
        uDirty: dirty,
        uSurface: this.coldData.surfacePage ?? this.empty, // no page yet → defs have no frame → solid quads
      },
      uniforms: (p) => {
        p.uInt("uCorridor", this.corridor ? 1 : 0);
        p.uInt("uLightClass", cls);
        p.uFloat("uElevK", this.elevK);
      },
    });
    this.renderer.draw({
      program: this.lighting,
      geometry: this.fsQuad,
      target: lightRT,
      blend: "none",
      textures: {
        uData: this.coldData.dataTexture, // presence (sets 3/5) + light records (set 2)
        uDirty: dirty,
        uShadow: shadowRT.textures[0], // this class's shadow-cold (COARSE — the fine bake upsamples it)
        uSurface: this.coldData.surfacePage ?? this.empty, // lightmap (CO-PACK): silhouette + normal quadrants share this page
      },
      uniforms: (p) => {
        p.uInt("uLightClass", cls);
        p.uInt("uWorldLight", this.worldLight ? 1 : 0);
        p.uFloat("uNsInv", this.nsInv);
        p.uInt("uShowNormal", this.showNormal ? 1 : 0);
        p.uFloat("uNormalPitch", this.normalPitchDeg * Math.PI / 180);
        p.uVec2("uLightAlign", this.lightAlignX, this.lightAlignY);
        p.uInt("uEdgeRefine", this.edgeRefine ? 1 : 0);
        p.uInt("uHideRight", this.hideRight ? 1 : 0);
      },
    });
  }

  /** Decode shadow-cold over the world (debug `/overlayRT shadow-cold`). */
  drawOverlay(camera: Camera, win: TileWindow): void {
    if (!this.coldShadowRT || !this.hotShadowRT || win.cols === 0) return;
    const w = camera.width, h = camera.height, z = camera.zoom, ax = camera.anchorX, ay = camera.anchorY;
    // Cover the window's world rect.
    const x0 = win.winCol * SQUARE, y0 = win.winRow * SQUARE;
    const x1 = (win.winCol + win.cols) * SQUARE, y1 = (win.winRow + win.rows) * SQUARE;
    this.overlayPos.set([x0, y0, x1, y0, x1, y1, x0, y1]);
    this.overlayGeo!.update("aWorld", this.overlayPos);
    const proj = new Float32Array([(2 * z) / w, 0, 0, 0, -(2 * z) / h, 0, (-ax * 2 * z) / w, (ay * 2 * z) / h, 1]);
    this.renderer.draw({
      program: this.overlay,
      geometry: this.overlayGeo!,
      blend: "normal",
      textures: { uShadow: this.coldShadowRT.textures[0], uShadowHot: this.hotShadowRT.textures[0], uData: this.coldData.dataTexture },
      uniforms: (p) => {
        p.uMat3("uProjection", proj);
        p.uInt("uCols", win.cols);
        p.uInt("uRows", win.rows);
        p.uInt("uWinCol", win.winCol);
        p.uInt("uWinRow", win.winRow);
        p.uInt("uSlot", TEXTILE_UNIT);
      },
    });

    // Gizmos: a dot at each light + a ring at its radius (screen-constant thickness).
    const pxWorld = 1 / z;
    for (let k = 0; k < this.lights.length; k++) {
      const L = this.lights[k], col = LIGHT_COLORS[k % LIGHT_COLORS.length];
      this.renderer.draw({
        program: this.gizmo,
        geometry: this.gizmoQuad,
        blend: "normal",
        uniforms: (p) => {
          p.uMat3("uProjection", proj);
          p.uVec2("uCenter", L.x, L.y);
          p.uFloat("uHalf", L.reach * 1.06);
          p.uFloat("uRadius", L.reach);
          p.uFloat("uPxWorld", pxWorld);
          p.uVec3("uColor", col[0], col[1], col[2]);
        },
      });
    }
  }

  destroy(): void {
    this.fsQuad.destroy();
    this.overlayGeo?.destroy();
    this.gizmoQuad.destroy();
    this.gather.destroy();
    this.lighting.destroy();
    this.overlay.destroy();
    this.gizmo.destroy();
    this.empty.destroy();
    this.coldDirtyTex.destroy(); this.hotDirtyTex.destroy();
    this.coldShadowRT?.destroy(); this.hotShadowRT?.destroy();
    this.coldLightRT?.destroy(); this.hotLightRT?.destroy();
    this.coldData.destroy();
  }
}
