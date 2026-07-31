//! shadowGather (webgl) — the per-light **shadow bitfield** via a fragment **gather**
//! (`2026-07-21-shadow-bitfield`). Each cold light is one bit in a world-space toroidal `RGBA32UI`
//! `shadow-cold` buffer; a fragment computes its texel's full 128-bit mask by looping the reaching
//! lights, testing point-in-projected-silhouette against each light's casters, and OR-ing bits in a
//! register — one write, no ping-pong (F1). Data comes from `ColdShadowData` (`light_data` / `billboard_data`
//! / `billboard_definition_data`) via `texelFetch`. Layouts authoritative in `docs/VARIABLES.md`.
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
import { ColdShadowData, N_LIGHTS, SET_BILLBOARD_DATA, SET_DEFINITION_DATA, type TileDefLanes } from "./coldShadowData";
import { SQUARE, UNIT, TEXTILE_UNIT, TEXTILE_LIGHT, SLOTS_X, SLOTS_Y } from "./squareMath";

/** Texels per TILE edge in the SHADOW map — **`TEXTILE_UNIT` (16), i.e. one texel per world unit.**
 *
 *  Kept as a named dial because it was A/B'd against `TEXTILE_SQUARE` on 2026-07-27 and the answer is worth
 *  not re-deriving (`moving-lights` I13). Matching the lightmap's 64/tile removes the fine→coarse upsample —
 *  `FINE_RATIO` becomes 1 and the blocky shadow edge disappears by construction — but the **gather runs one
 *  fragment per shadow texel**, so it rasterises across the whole shadow map:
 *
 *    shadow RT 512×256 → 2048×1024 · VRAM 16 → 256 MiB · gather 2.64 → 9.44 ms · GPU total 3.09 → 9.84 ms
 *
 *  Reverted: a sharp edge is not worth 3.2× GPU and 4× VRAM when the coarse edge is acceptable.
 *  (Cost is sublinear in texels — 16× the fragments cost 3.6× the time, cache locality — but still 3.2×.) */
const SHADOW_TEXELS = TEXTILE_UNIT;

/** Fine light texels per shadow texel — `TEXTILE_LIGHT / SHADOW_TEXELS` = 64/16 = **4**. Both operands are
 *  now pinned constants that `SQUARE` cannot move, so this ratio is fixed by construction: the fine-refine
 *  loop in `LIGHT_FRAG` is immune to the art resolution. If this stops reading 4, the split is broken. */
const FINE_RATIO = TEXTILE_LIGHT / SHADOW_TEXELS;

/** Quantisation step for one light's contribution to the additive lightmap (F11b).
 *
 *  Each light deposits `round(contribution · LIGHT_QUANT)` — an INTEGER — and is removed by emitting
 *  that same integer negated. FP32 holds every integer below 2^24 exactly, so the pair cancels bit-exactly
 *  regardless of order; unquantised floats do not (measured drift 1.8e-7 over 64 pairs). The quantisation
 *  IS the correctness mechanism, not a compression choice.
 *
 *  **255 is load-bearing, not a default — do not raise it.** It is the largest step for which exactness is
 *  UNCONDITIONAL. Worst case is every light that can exist landing on one texel:
 *      65,535 (max u16 ids) x 255 = 16,711,425  <  2^24 = 16,777,216
 *  so overflow is impossible by construction — no presence cap, no bookkeeping discipline, no distribution
 *  assumption required. At 512 that breaks (65,535 x 512 = 33.5M) and correctness would depend on a policy
 *  we cannot currently guarantee. The failure mode is silent: light that will not fully turn off.
 *
 *  This bound is ENFORCED by the per-light clamp in LIGHT_FRAG, not merely hoped for. That clamp is also why
 *  a light may exceed 255 before write with no consequence: a raw 512 clamps to 255 and the light still
 *  reads brighter, because clamping caps the PEAK and not the falloff profile.
 *
 *  And raising it buys nothing observable, because QUANT is PRECISION, not RANGE:
 *   - HDR does not need it. A 4x brazier just deposits 1020; the accumulator is float and holds that at
 *     any step size. Brightness comes from the value, not the granularity.
 *   - Smoothness cannot use it. Output is `albedo x light` and the display quantises to 1/255, so with
 *     `albedo <= 1` one lightmap step maps to AT MOST one display step. Finer steps land below what the
 *     screen can show.
 *
 *  Revisit only if the output pipeline goes deeper than 8-bit (a 10-bit swapchain, or a tonemap working in
 *  float before quantising). Banding at 255 indicates a different bug, not too coarse a step. */
export const LIGHT_QUANT = 255;

/** Lights this iteration — a ring of debug lights around the seed tile. `number`-typed so the
 *  isolate-one-light debug path (`MAX_LIGHTS = 1`) below isn't flagged as a constant comparison. */
const MAX_LIGHTS: number = 3;
/** Physical source RADIUS (world px); `/UNIT` → units in the light record. The AREA-LIGHT disk radius the
 *  penumbra samples over (bigger = softer). Per-light (see the config below); `__emit(px)` tunes live. */
/** Light height: 40 units = 2.5 tiles — above the tree billboard (2 tiles / 32 units). Lower = longer
 *  shadows, but ONLY down to the caster's card top: `solveCentre` returns its semi-infinite status when the light sits
 *  below it (`k = Lz/(Lz−Zt) <= 0`), so a light under ~26 units casts NOTHING, silently. Lights are
 *  content-authored per kind now (`&thing.light.height`, in TILES) — this figure is the constraint that
 *  authored value must clear, not a default anything reads. Authoring 0.6 zeroed every shadow in the
 *  world; see I37 / I38 in work/2026-07-25-primitive-graph/issues.md. */
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
/** shadows-onto-billboards: a billboard pixel drawn Δ units above its own base has fictional height
 *  `sin(WORLD_TILT)·Δ` (the ratified world-geometry model) — the DEFAULT receiver-elevation gain that makes
 *  a cast shadow climb the sprite. `__elevk(k)` tunes the climb by eye; 0 collapses it to the flat ground
 *  shadow. Empirical because the caster card runs its own internal ratio (the documented 0.5 north offset). */
const ELEV_K_DEFAULT = Math.sin(WORLD_TILT_DEG * Math.PI / 180);
/** Exponent applied to the smoothstep falloff. **1 = the raw S-curve; below 1 flattens the drop-off** —
 *  the mid-range lifts (0.25 → 0.5 at 0.5) so a pool reads bright most of the way out instead of dying
 *  just outside the source. It cannot leak light past a light's reach: 0 and 1 are fixed points of
 *  `pow`, so the endpoints are untouched and the corridor/brute reach box stays exactly valid.
 *  This is a LOOK dial, not a physical one — inverse-square it is not, and deliberately so. */
/** A/B DIAL (2026-07-26, `moving-lights` I6): tiles fetched per corridor step. **3** = the current
 *  perpendicular-only dilation (I30); **5** = the pre-I30 cross this replaced, kept switchable so
 *  "is today's walk actually better than this morning's" is a measurement and not a commit message. */
const DILATE = 3;
/** DEFAULT falloff exponent. Lowered 0.75 → 0.5 (2026-07-27) alongside torch reach 8 → 20: at reach 20
 *  the 0.75 curve left the outer half of the pool too dim to judge a shadow edge in, which defeats the
 *  point of the longer reach. Now a LIVE uniform (`__falloff(e)`) rather than a baked constant, because
 *  it is a look dial being tuned by eye and a shader rebuild per step makes that miserable. */
const FALLOFF_EXP = 0.5;
/** world-space-lighting (P1): the N–S un-foreshorten factor = 1/cos(WORLD_TILT). Screen N–S is compressed
 *  by cos(tilt), so true world N–S = screen·(1/cos(tilt)). Derived in
 *  work/2026-07-24-world-space-lighting/model.md; sin-vs-cos is the flagged risk, so it's live-tunable. */
const INV_COS_TILT_DEFAULT = 1 / Math.cos(WORLD_TILT_DEG * Math.PI / 180);
/** Lift the rendered shadow up (toward smaller world-y) by this many world units — a fragment shows
 *  shadow if the point this far BELOW it is shadowed, so the whole silhouette slides up. Closes the ~1
 *  unit gap between the shadow base and the sprite's drawn base (a fixed anchor discrepancy: the sprite
 *  bottom sits ~1 unit south of the billboard base-centre the shadow projects from). `__lift(u)` tunes live. */
const SHADOW_LIFT = 3.0;             // units the shadow slides up to seat on the sprite base (sprite bottom
                                    // sits ~3 units south of the billboard base-centre the shadow projects from)
/** shadows-onto-billboards: the receiver MASK (receiverCover) cuts the sprite-shaped hole slightly OFF from the
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
/** shadows-onto-billboards: the seen-face cull excludes casters that aren't strictly SOUTH of the receiver base
 *  — widened into a UNIT band so a caster within this many units of the receiver's base (nearly the same y,
 *  i.e. self / same-object) is also excluded, killing near-self shadow. Measured in world units (user). */
const SELF_BAND = 4.0;
const SELF_BANDF = SELF_BAND.toFixed(1);
/** shadows-onto-billboards: extend the shadow quad's BASE (contact) edge this many units SOUTH — tucking the
 *  bottom of the shadow into the caster's own footprint/trunk (which reads dark / is under the sprite, so
 *  it's not visibly coloured) to close the seam at the shadow base (user nit). */
const SHADOW_BASE_PUSH = 0.75;
const SHADOW_BASE_PUSHF = SHADOW_BASE_PUSH.toFixed(2);
/** shadows-onto-billboards: fade the receiver MASK to GROUND over this many units at the sprite BASE. Without it
 *  the mask claims the trunk base as a thing texel, whose thing-path self-excludes the caster → the ground
 *  contact shadow (incl. the pushed base) gets CULLED there, leaving a lit seam. Fading the bottom band to
 *  ground lets that contact shadow show (thing ≈ ground at the base anyway). Units (user nit). */
const MASK_BASE_FADE = 1.5;
const MASK_BASE_FADEF = MASK_BASE_FADE.toFixed(1);
const SHADOW_LIFTF = SHADOW_LIFT.toFixed(1);
/** Slots in ONE tile-keyed px — v3 retired the u16 self-address, so all four lanes hold pairs. */
const TILE_SLOTS = 8;
/** Lights per tile — **8** (F7, 2026-07-31). Was 16 across a lo + hi presence pair.
 *
 *  8 is what makes the shadow texel hold a caster ID instead of a value: `8 × u16 = 128 bits`, exactly
 *  one `uvec4`, with a sentinel meaning "not occluded" so occlusion is implied rather than stored. At 16
 *  the ids needed 256 bits and had to live somewhere else — and every somewhere else failed (a second
 *  MRT attachment hung the renderer twice; an id short enough to pack in spare bits cannot index a u16
 *  `billboardIdx`).
 *
 *  This caps how many lights may OVERLAP ONE TILE, not how many exist: presence is per tile and the sets
 *  are independent, so neighbouring tiles hold different eights. Raise back to 16 only if a real scene
 *  is measured hitting it. Halving it also halves the gather's inner loop. */
const PRES_SLOTS = TILE_SLOTS;   // F7: 8. The hi presence set is retired -- see the doc above.
/** Casting billboards bucketed per tile — ONE px, so {@link TILE_SLOTS}. I17: this is the stride the
 *  fill, the allocation AND the write must all agree on; when they were separate literals they
 *  desynced (7 vs 8) and the buckets were built from misaligned memory. Derive, never re-type it. */
const BILLBOARD_SLOTS = TILE_SLOTS;
/** texture-generalization P1 (D7): `prim_presence` slots per tile — 4 full u32s. */
const PRIM_SLOTS = 4;
/** Slot flag bits (top nibble): cast_shadows + the receives_shadows mode (D8). */
const SLOT_CAST = 0x8000_0000;
const SLOT_RECV_SHIFT = 29;


/** The cold cache's toroidal tile window (from `SquareCache.window`) — shadow-cold aligns to it. */
export interface TileWindow {
  winCol: number;
  winRow: number;
  /** Tiles across/down = `SLOTS << lod`. Grows with zoom-out; the TEXTURES do not. */
  cols: number;
  rows: number;
  /** Current lod 0–3. Siblings derive their own per-tile texel size as `texelsPerSlot >> lod`. */
  lod: number;
}

/** Shared GLSL: packed-position decode + ground projection + the fan region predicate (roles → tris). */
const GATHER_COMMON = /* glsl */ `
// PROFILING STAIRCASE inside casterCover (2026-07-27, moving-lights I15) — casterOne is 85 % of the walk,
// so this is where the frame actually goes. 0 = full; 1..3 cut short:
//   1 the two record fetches (billboard + definition) + decode   2 + the geometry, no silhouette
//   3 + hard-quad return (skips the 16-tap emitter loop)         0 + the 16-tap silhouette loop
uniform int uCProfile;
// CARD LEAN — the caster card's north offset as a fraction of H·cos(tilt). 1.0 = a RIGID parallel-to-view
// billboard (top at north H·cos0, up H·sin0, card length exactly H). 0.5 was the shipped value and is
// NOT a rigid rotation: it leans half as far north while keeping full elevation, so the card measures
// ~0.87·H at 55 degrees and shadows come out short in the north-south direction. Nobody derived the 0.5;
// it is world-geometry F2, left open as a look call. Live via __lean(x) so it can be A/B'd in one frame.
// MUST match the CPU-side TILT in buildCasters, or casters get bucketed for a card they do not cast.
uniform float uCardLean;
const float UNIT = ${UNITF};      // SQUARE/16 (compile-time; px per unit)
const float SQ = ${SQF};          // SQUARE world px per tile
const float UPT = SQ / UNIT;      // world UNITS per tile (= TEXTILE_UNIT = 16)
const uint  ZD = 16u, RD = 16u;  // ZONE_DIM, REGION_DIM
// THE unified data texture (1024×1024): linear index → texel; 64-row bands (VARIABLES.md).
const int DEF_BASE = 0, PRIM_BASE = 65536, LIGHT_BASE = 131072, CONST_BASE = 1047552; // row 1023
const int BILLBOARD_BASE = 393216;  // v3 set 6 — the BILLBOARD LEAF (set 1 = prim_data, the node)
const int PRESENCE_BASE = 196608, BILLBOARD_PRESENCE_BASE = 262144, PRESENCE_HI_BASE = 327680; // tile-keyed sets 3, 4, 5
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
// v5 SHADOW TEXEL (F7, 2026-07-31) -- THE encoding, and the only place it is defined.
//
//   8 slots x u16 = 128 bits = one uvec4, laid out exactly like tileSlot: slot i in lane i>>1,
//   high half when (i&1)==0.
//
//   bit 15     on-billboard flag (a per-TEXEL fact; every occupied slot carries the same value)
//   bits 0-14  the occluding caster's billboardIdx, or SHADOW_NONE when nothing occluded
//
// Occlusion is no longer STORED, it is IMPLIED: a slot is shadowed iff its id is not SHADOW_NONE.
// That is what makes 8 x u16 fit where 16 x u16 did not, and it is why there is no coverage bit --
// the value the map used to hold was measured perfectly bimodal (P0), so the id subsumes it.
const uint SHADOW_NONE = 0x7fffu;
uint shadowSlot(uvec4 t, int i) {              // the raw u16 for slot i
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}
float shadowOcc(uvec4 t, int i) { return (shadowSlot(t, i) & 0x7fffu) != SHADOW_NONE ? 1.0 : 0.0; }
uint shadowCaster(uvec4 t, int i) { return shadowSlot(t, i) & 0x7fffu; }   // SHADOW_NONE = none
uint tileSlot(uvec4 t, int i) {   // v3: 8 slots/px — the self-address is gone (R=s0|s1 .. A=s6|s7)
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}
// texture-generalization P1 (D7): a prim_presence slot — ONE FULL u32 per channel:
// flags 31-28 (bit 31 cast_shadows | bits 30-29 receives_shadows) | set 27-24 | index 15-0.
// 0 = empty. The flags let a walk reject a non-caster at this one read, no record fetch.
uint primSlot(uvec4 t, int i) { return i == 0 ? t.x : (i == 1 ? t.y : (i == 2 ? t.z : t.w)); }
// texture-generalization P3 (D1): is the tile at (wc, wr) a CONNECTING autotile — its slot-0
// def in rotation MODE 1? The variant/kind gate is FUTURE content work (README R4); today
// mode-1 match IS the adjacency rule, and walls are the only mode-1 kind.
bool tileConnects(int wc, int wr, highp usampler2D data) {
  uint s0 = fetchLin(data, BILLBOARD_PRESENCE_BASE + foldTile(wc, wr)).x;
  if (s0 == 0u || ((s0 >> 24) & 15u) != 0u) return false;    // empty, or not a definition_data slot
  uvec4 D = fetchLin(data, DEF_BASE + int(s0 & 0xffffu));
  return ((D.x >> 26) & 3u) == 1u;                           // rotation lane = the per-type tile MODE
}
// The 4×4 autotile CELL for the rotation-1 tile at (wc, wr), from its cardinal neighbors —
// THE D1 formula (x = N + 2E, y = 3 − (S + 2W), cell = y·4 + x). ONE formula, two consumers:
// this must match linkedCell.ts exactly (the CPU draws the art with it; the boot assertion
// pins the 16-row table). North is −y.
int tileAutoCell(int wc, int wr, highp usampler2D data) {
  int n = tileConnects(wc, wr - 1, data) ? 1 : 0;
  int e = tileConnects(wc + 1, wr, data) ? 1 : 0;
  int s = tileConnects(wc, wr + 1, data) ? 1 : 0;
  int w = tileConnects(wc - 1, wr, data) ? 1 : 0;
  return (3 - (s + 2 * w)) * 4 + (n + 2 * e);
}
// texture-generalization P4: the TILE's normal at world point P, from its slot-0 def — the
// atlas CELL (the D1 in-shader autotile for rotation-mode-1 defs; the frame as-is otherwise),
// the pad-inset window stretched across the tile square, sampled from the NORMAL quadrant of
// the co-pack (+side E, −side N of the surface quadrant — same layout billboardNormal uses).
// vec3(0) when unresolved (loose def / empty tile / outside the frame) → caller keeps flat.
vec3 tileNormal(int wc, int wr, vec2 P, highp usampler2D data, sampler2D surf) {
  uint s0 = fetchLin(data, BILLBOARD_PRESENCE_BASE + foldTile(wc, wr)).x;
  if (s0 == 0u || ((s0 >> 24) & 15u) != 0u) return vec3(0.0);
  uvec4 D = fetchLin(data, DEF_BASE + int(s0 & 0xffffu));
  uint lod = (D.z >> 4) & 15u;
  if (lod < 4u) return vec3(0.0);                            // loose def — no frame resolved
  uint mode = (D.x >> 26) & 3u;
  int cols = int(((D.y >> 10) & 15u) + 1u);                  // grid = frame span (1 tile per cell)
  float spanU = float(cols) * 16.0;
  float ppu = float(1u << lod) / spanU;
  float pad = float((D.x >> 14) & 0x3ffu);                   // the between-cell inset (units)
  float W = float(((D.y >> 23) & 511u) * 2u);                // one cell's window (units)
  int cell = (mode == 1u && cols > 1) ? tileAutoCell(wc, wr, data) : 0;
  vec2 co = vec2(float(cell % cols), float(cell / cols)) * 16.0 * ppu; // cell origin (row-major, top)
  vec2 l = fract(P / 16.0);                                  // position across the tile square [0,1)
  float side = float(1u << lod);
  float sfx = float((D.z >> 22) & 1023u) * 16.0, sfy = float((D.z >> 12) & 1023u) * 16.0;
  float nfx = sfx + side, nfy = sfy - side;                  // NORMAL quadrant origin
  vec2 uv = vec2(nfx, nfy) + co + (vec2(pad) + l * W) * ppu;
  if (uv.x < nfx || uv.x >= nfx + side || uv.y < nfy || uv.y >= nfy + side) return vec3(0.0);
  vec3 n = texelFetch(surf, ivec2(uv), 0).xyz * 2.0 - 1.0;
  return normalize(n);
}
// D9: how many rows SOUTH of a visited tile the walks also check — under BASE-LINE occupancy
// registration those are the only tiles whose occupants' cards can lean over the visited one.
// CPU-derived from content (ceil(TILT x maxCardTiles)).
uniform int uDilateS;
vec2 decodePos(uint p) {          // position_anchor_reference → world UNITS
  uint region = (p >> 24) & 255u, zone = (p >> 16) & 255u, tile = (p >> 8) & 255u, anchor = p & 255u;
  uint wtx = (((region >> 4u) * RD + (zone >> 4u)) * ZD + (tile >> 4u));
  uint wty = (((region & 15u) * RD + (zone & 15u)) * ZD + (tile & 15u));
  return vec2(float(wtx * 16u + (anchor >> 4u)), float(wty * 16u + (anchor & 15u)));
}
// v3: a leaf stores its RESOLVED position as zone|tile|unit (period = 256 tiles), not an absolute
// address — the region is recovered by taking the congruent representative NEAREST the reading point.
// Exact while the true separation is under half a period (128 tiles), which any light reach satisfies.
vec2 resolvedPos(uint zone, uint tile, uint unit, vec2 ref) {
  vec2 tl = vec2(float(((zone >> 4) & 15u) * 16u + ((tile >> 4) & 15u)),
                 float((zone & 15u) * 16u + (tile & 15u)));
  vec2 un = vec2(float((unit >> 4) & 15u), float(unit & 15u));
  vec2 lm = tl * UPT + un;                    // position within the 256-tile period, in units
  float period = 256.0 * UPT;
  vec2 d = lm - ref;
  d -= period * floor(d / period + 0.5);      // wrap to the nearest congruent representative
  return ref + d;
}
// v3 sibling of resolvedPos for a BILLBOARD leaf, which stores only tile|unit (no zone — its presence
// is a CONTAINMENT relation). Period is therefore 16 tiles, so ref MUST be near the billboard: pass
// the VISITED BUCKET TILE, never the sample point (which can be many tiles away during the walk).
vec2 resolvedTilePos(uint tile, uint unit, vec2 ref) {
  vec2 lm = vec2(float((tile >> 4) & 15u), float(tile & 15u)) * UPT
          + vec2(float((unit >> 4) & 15u), float(unit & 15u));
  float period = 16.0 * UPT;
  vec2 d = lm - ref;
  d -= period * floor(d / period + 0.5);
  return ref + d;
}
// F8 (2026-07-31): THE billboard anchor decode — every consumer of a billboard record goes through
// here, so the ROOT/CHILD split lives in exactly one place.
//
//   child = 0 (A bit 13 clear)  RED is the full region|zone|tile|unit address -> decodePos is
//                               ABSOLUTE. No reference, no period, exact at any separation.
//   child = 1                   RED's top half is parent_id, so only tile|unit survive (16-tile
//                               period) and ref must be within +-8 tiles. Safe by construction:
//                               a child is a piece carried by a composite, and its reference is
//                               its carrier, a tile or two away.
//
// The absolute path is what makes a bare caster id positionable (I7). Recovering a caster from the
// id map hands you no reference to pass, and substituting the receiver's own tile was NOT safe at
// reach 16 -- the caster can sit beyond the +-8 the wrap tolerates, and the failure is a false
// POSITIVE: a mis-decoded caster lands somewhere plausible, occludes, and paints a phantom shadow.
vec2 billboardPos(uvec4 Pd, vec2 ref) {
  vec2 b = ((Pd.w >> 13) & 1u) == 1u
         ? resolvedTilePos((Pd.x >> 8) & 255u, Pd.x & 255u, ref)
         : decodePos(Pd.x);
  // hot-sync P3: + the sub-unit lanes (eighths of a unit) so lighting tracks the DRAWN anchor
  // rather than the unit grid. Carried on B for both forms.
  return b + vec2(float((Pd.z >> 11) & 7u), float((Pd.z >> 8) & 7u)) * 0.125;
}
// The WORLD ground tilt as (sin, cos) -- NOT the angle. Nothing downstream ever wants the angle itself:
// it appears only as st and ct in the card maths (dn, lean, k), so carrying radians just means every
// consumer pays a sin and a cos to recover what we could have stored directly.
//
// It used to be fetched-and-trigged INSIDE casterCover, i.e. once per caster PER TEXEL -- a dependent
// texel fetch plus two transcendentals, redone dozens of times per texel for a value that is constant
// for the whole pass. Now a uniform, computed CPU-side from the same worldTiltDeg that writes the data
// map's centidegrees, so __tilt(deg) still re-tilts everything together and there is one source of
// truth. The data map keeps carrying the angle for any other shader that reads it.
uniform vec2 uTilt;               // x = sin(tilt), y = cos(tilt)
// ONE backward solve at the emitter CENTRE. Returns false if the ray never meets the card's plane at
// all; otherwise hands back u (across the card), t (up it) and invk = 1/k, which the caller needs to
// build the analytic interval.
//
// invk instead of k is not a micro-tweak: u needs 1/k, so computing k and then dividing by it spends
// TWO divides where one multiply will do. The k <= 0 test survives unchanged as invk <= 0, since
// invk = (Lz - t*H*sin0)/Lz and Lz > 0, so the two share a sign.
// Returns 0 = the ray never meets the card (lit), 1 = solved, u/t/invk valid,
//         2 = SEMI-INFINITE: the light sits at or below this card height so the shadow runs to infinity.
// That last case is the one I38 fixed -- returning "lit" there silently deleted every shadow whenever a
// light was authored low. The caller caps it at reach, which is the only place the reach bound is
// load-bearing (forks F2).
int solveCentre(vec2 Lxy, float Lz, float invLz, vec2 P, vec2 Ac, float invW,
                float Hst, float leanC, float tMin,
                out float uOut, out float tOut, out float invkOut) {
  uOut = 0.0; tOut = 0.0; invkOut = 0.0;
  float dn = Hst * (P.y - Lxy.y) - leanC;             // leanC = H*uCardLean*Lz*cos0, per-caster constant
  if (abs(dn) < 1e-4) return 0;                       // ray parallel to the card plane
  float t = Lz * (P.y - Ac.y) / dn;                   // the ONE unavoidable divide
  // SHADOW_BASE_PUSH, translated. The forward path put the card BASE at Ac.y + push (SOUTH, into the
  // caster footprint) to close a sprite/shadow seam. y(t) moves NORTH as t rises, so south of the base is
  // t slightly NEGATIVE -- the same fudge expressed in the card parameterisation instead of in a corner.
  if (t < tMin || t > 1.0) return 0;                  // outside the card's height -> no shadow from it
  float invk = (Lz - t * Hst) * invLz;
  if (invk <= 0.0) return 2;                          // semi-infinite -- see above
  uOut = ((P.x - Lxy.x) * invk + Lxy.x - Ac.x) * invW + 0.5;
  tOut = t; invkOut = invk;
  return 1;
}
// Sample the silhouette at card coords (u,t). Split from the solve because the WEDGE RULE needs BOTH
// rays' u before either is fetched -- see casterCover.
float sampleCard(float u, float t, float W, float H, uint rot, float ppu, float ox, float oy,
                 float nx, float ny, vec2 fo, vec2 fmin, vec2 fmax, sampler2D surf) {
  float sm = rot == 3u ? 1.0 - u : u;                 // W-facing = mirrored E frame
  vec2 uv = fo + vec2(ox, oy) * ppu - vec2(nx, ny) + vec2(sm * W, (1.0 - t) * H) * ppu;
  if (uv.x < fmin.x || uv.x >= fmax.x || uv.y < fmin.y || uv.y >= fmax.y) return 0.0;
  return texelFetch(surf, ivec2(uv), 0).b;            // silhouette coverage (B carries its own edge AA)
}
// ns-shadows P1: coverage of a PERPENDICULAR (n/s-rotated) caster. The card is a VERTICAL plane
// on the sprite's centerline -- it contains the north-south axis + height, no lean (user-ratified
// geometry, mockup 2026-07-28). Its silhouette is the SIDE (east) frame carried in the record's
// A lane (caster_definition_id), spanning the side frame's tight width CENTERED on the anchor row
// (D2). The centre ray L->Q intersects x = A.x once; card coords are s = n-s offset across the
// span (caster_flip mirrors, reusing sampleCard's rot==3 arm) and t = height at the intersection.
// Penumbra: the same interval arithmetic as the main card, with du/dlambda LINEARIZED at the
// centre ray (u(lambda) is rational here, not linear -- same approximation class as holding t
// fixed across the span, which the main card also does). East-vs-west needs no special casing:
// a light east of the plane projects west and vice versa, straight from the ray-plane solve.
float casterCoverNS(uvec4 Pd, vec2 A, vec2 Q, vec3 L, float emitter, highp usampler2D data, sampler2D surf) {
  int defIdx = int((Pd.w >> 16) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  float W = float(((D.y >> 23) & 511u) * 2u);         // side frame tight width = the card's n-s span
  float H = float(((D.y >> 14) & 511u) * 2u);         // side frame tight height = the card's height
  if (W <= 0.0 || H <= 0.0) return 0.0;
  float b = Q.x - L.x;
  if (abs(b) < 1e-4) return 0.0;                      // ray parallel to the plane -> no intersection
  float tx = (A.x - L.x) / b;
  if (tx <= 0.0 || tx >= 1.0) return 0.0;             // caster not between light and receiver
  float t0 = L.z * (1.0 - tx) / H;                    // intersection height in card coords
  if (t0 < 0.0 || t0 > 1.0) return 0.0;               // ray passes over the card top -> lit
  // shadow-polish P1: the record's anchor is the BASE-CENTRE = the sprite's BOTTOM (south
  // tip) — for a top-down n/s sprite the drawn body IS the ground trace, so the card spans
  // [A.y − W, A.y], NOT A.y ± W/2 (which displaced the whole shadow half a body south —
  // the "flipped n/s" report).
  float u0 = (L.y + tx * (Q.y - L.y) - (A.y - W)) / W;
  // caster_flip: head end follows facing (D3). shadow-polish P1 (user drill): the selection
  // was INVERTED — with the card's u running north→south onto the side frame (head at the
  // frame's east/right), the unflipped read put the head at the SOUTH end for a north-facing
  // wolf. Swapped so the shadow's head tracks the sprite's head at both facings.
  uint mrot = ((Pd.w >> 15) & 1u) == 1u ? 1u : 3u;
  // Penumbra interval on the rotated axis: sub-light L + lambda*perp, du/dlambda at lambda 0.
  vec2 sdir = A - L.xy;
  float slen = length(sdir);
  vec2 along = slen > 1e-4 ? sdir / slen : vec2(1.0, 0.0);
  vec2 perp = vec2(-along.y, along.x) * emitter;
  // BINARY (2026-07-30) -- the n/s card drops its interval for the same reason as the main card above.
  if (u0 < 0.0 || u0 > 1.0) return 0.0;               // centre ray misses the card -> lit
  float uMid = u0;
  uint lod = (D.z >> 4) & 15u;
  if (lod < 4u) return 1.0;                           // no silhouette resolved -> solid rotated quad
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);
  float ox = float((D.x >> 14) & 1023u), oy = float((D.x >> 4) & 1023u);
  float ppu = float(1u << lod) / spanU;
  float fx = float((D.z >> 22) & 1023u) * 16.0, fy = float((D.z >> 12) & 1023u) * 16.0;
  float nx = float(int((D.w >> 20) & 4095u) - 2048);
  float ny = float(int((D.w >> 8) & 4095u) - 2048);
  float fside = float(1u << lod);
  vec2 fo = vec2(fx, fy), fmin = fo, fmax = fo + vec2(fside);
  return sampleCard(clamp(uMid, 0.0, 1.0), t0, W, H, mrot, ppu, ox, oy, nx, ny, fo, fmin, fmax, surf);
}
// Coverage of one caster (billboard index into billboard_data) at P from light L: read the billboard record + its
// definition (position + geo W/H), then the pure-quad test; if occluded, apply the sprite's SHAPE
// (P4) — invert P back to the card's (s,t) (in-range BY CONSTRUCTION: P is inside the projected
// quad, so no u/v-out-of-range class of reject exists) and sample the surface silhouette (coverage,
// B channel) at the def's opaque frame. frame_w = 0 (not resolved / off-page) → solid quad.
// Coverage of one caster (billboard index into billboard_data) at P from light L. Reads the billboard record + its
// definition (position + geo W/H + frame), places the tilted card, then computes a GROUND-PROJECTED
// penumbra by AREA-LIGHT sampling: the light is a disk of radius emitter (units) at height Lz; for
// each sub-light we re-invert the ground projection P → card (s,t) and sample the HARD silhouette
// (surface B) at that point; the average over the disk is the soft coverage. Why this grounds the
// shadow (vs blurring the projected card image): a silhouette edge at height z casts to a ground point
// that shifts with the sub-light by ∝ z/(Lz−z) — so a z≈0 CONTACT edge does not move (hard, attached
// base), a high edge moves a lot (soft tip, detail dissolving), and points just outside the true edge
// are covered by only SOME sub-lights (soft outer perimeter, no hard card boundary). lod<4 (no
// silhouette resolved) or a point light (emitter≈0) falls back to the solid/hard quad.
float casterCover(uint billboardIdx, vec2 P, vec3 L, float emitter, float reachU, vec2 ref, highp usampler2D data, sampler2D surf) {
  if (billboardIdx == 0u) return 0.0;
  uvec4 Pd = fetchLin(data, BILLBOARD_BASE + int(billboardIdx));      // R = position (root) or parent|tile|unit (child), G = orient, B = def
  vec2 A = billboardPos(Pd, ref);                                     // F8: root decodes absolutely
  // ns-shadows P1: an n/s-rotated caster with a resolved side def casts from the perpendicular card.
  uint rotc = (Pd.y >> 26) & 3u;
  if ((rotc == 0u || rotc == 2u) && ((Pd.w >> 14) & 1u) == 1u) {
    return casterCoverNS(Pd, A, P, L, emitter, data, surf);
  }
  int defIdx = int((Pd.z >> 16) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  // v2.1: R = u16 id | u10 offset_x | u6 reserved; G = u9 W | u9 H | u4 span (tiles−1) | u10 offset_y.
  // B: frame origin (16-px grid) + page + lod exponent + 3x3 anchors. A: nudges.
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);    // frame world span (units)
  float ox = float((D.x >> 14) & 1023u), oy = float((D.x >> 4) & 1023u); // v3: both offsets in R
  uint lod = (D.z >> 4) & 15u;
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u); // anchors: 0 none | 1 half | 2 full
  // Anchor shift (units): the bbox's anchored point minus the FULL footprint box's same-anchored
  // point — billboard_data stores the full box's base-centre (= anchor 1,2), so shadows land the bbox's
  // base-centre on it; general anchors go live when billboard_data carries reported x/y (F3/P5).
  vec2 sh = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU,
                 oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  if (uCProfile == 1) return float(defIdx & 1) * 1e-6;      // C1: the two record fetches + decode only
  uint rot = (Pd.y >> 26) & 3u;                           // 1 = E, 3 = W (mirrored E)
  if (rot == 3u) sh.x = -sh.x;                              // flipped sprite → mirrored bbox placement
  vec2 Ac = A + sh;
  float st = uTilt.x, ct = uTilt.y;                  // WORLD_TILT, precomputed CPU-side (no fetch, no trig)
  // HARD-QUAD gate (NOT dilated) — a caster occludes P only where P is inside its projected quad, which
  // is exactly the occluder set the corridor walk is proven to visit (P6 identity). The emitter penumbra
  // lives INSIDE this quad: the silhouette edge softens as sub-lights partially cover it, the base stays
  // NO GATE. The analytic interval below IS the test -- there is nothing cheaper to reject with, and the
  // centre-ray gate P1 used was narrower than the caster's extremes, so it clipped the very penumbra
  // this is meant to produce.
  //
  // Everything from here needs the per-caster reciprocals and the tilt products, so hoist them once.
  float invLz = 1.0 / L.z, invW = 1.0 / W;
  float Hst = H * st, leanC = H * uCardLean * L.z * ct;
  float tMin = -${SHADOW_BASE_PUSHF} / max(1e-4, uCardLean * H * ct);
  float u0, t0, invk;
  int hit = solveCentre(L.xy, L.z, invLz, P, Ac, invW, Hst, leanC, tMin, u0, t0, invk);
  if (hit == 0) return 0.0;                            // ray never meets the card -> lit
  if (hit == 2) {                                      // SEMI-INFINITE (I38): cap it at the light's reach
    vec2 dR = P - L.xy;
    return dot(dR, dR) <= reachU * reachU ? 1.0 : 0.0; // solid; there is no finite u to sample
  }
  if (uCProfile == 2 || uCProfile == 3 || lod < 4u) {
    // No silhouette resolved (or a profile cut): the geometry alone, solid inside the card.
    float solid = (u0 >= 0.0 && u0 <= 1.0) ? 1.0 : 0.0;
    return uCProfile == 2 ? solid * 1e-6 : solid;
  }
  // Frame sampling constants (whole-px-per-unit; ppu = 2^lod / spanU is a pow2 >= 1 by construction).
  // Window top-left = frame origin + offset*ppu - nudge. Atlas rows are image-top-down; card t=0 is the
  // sprite's BOTTOM row, so v = 1-t.
  float ppu = float(1u << lod) / spanU;
  float fx = float((D.z >> 22) & 1023u) * 16.0, fy = float((D.z >> 12) & 1023u) * 16.0;
  float nx = float(int((D.w >> 20) & 4095u) - 2048);   // u12, +2048 bias -- full either-direction range
  float ny = float(int((D.w >> 8) & 4095u) - 2048);
  float fside = float(1u << lod);
  vec2 fo = vec2(fx, fy), fmin = fo, fmax = fo + vec2(fside);
  // THE CROSS-AXIS -- the direction the emitter is spread across, perpendicular to light->caster. The
  // penumbra lives across the shadow's WIDTH, not along its length, so this is the axis that matters.
  vec2 sdir = Ac - L.xy;
  float slen = length(sdir);
  vec2 along = slen > 1e-4 ? sdir / slen : vec2(1.0, 0.0);
  vec2 perp = vec2(-along.y, along.x) * emitter;
  // ANALYTIC INTERVAL (user, 2026-07-27). The sampling schemes before this -- 16 taps, then 2, then a
  // 4-tap binary search -- were all hunting for the same thing: WHERE on the emitter the shadow boundary
  // falls. That boundary can just be solved for.
  //
  // Parameterise a sub-light as L + lambda*perp, lambda in [-1,1]. Then u is LINEAR in lambda:
  //     u(lambda) = u0 + lambda*D,      D = perp.x * (1 - 1/k) * (1/W)
  // so inverting it gives the two lambdas whose rays graze the card's edges -- u = 0 and u = 1. That is
  // the same plane intersection we already do, solved for the OTHER unknown (user: "Are we going to
  // intersect the light from the point using the edges of the billboard instead?" -- yes, exactly).
  // The card's own coordinate already encodes the edges as 0 and 1, so no edge geometry is needed.
  //
  // Clamping that interval to the emitter's extent IS the penumbra: fully inside -> 1.0, half hanging
  // off the end -> 0.5, entirely outside -> 0. A continuous gradient, from arithmetic, with no taps.
  // BINARY (2026-07-30). The emitter-interval penumbra that lived here is DELETED. It solved
  // u(lambda) = u0 + lambda*Dslope at the card's two edges and clamped to [-1,1], which is a correct
  // soft-shadow integral -- and which, at the shipped emitter size, produced a value with exactly TWO
  // states. Measured on the live cold map before deleting it: 31 155 shadowed slot-samples, of which
  // **0** were partial. Not "few": zero. See P0 in the stream, and I4 for why (the clamp saturates
  // once the emitter is small against the card width).
  //
  // So this is not a quality trade. Binary is what the map already stored; this stops paying to
  // compute a gradient that never survived to the texture. What it BUYS is the reason for the change:
  // max() over casters becomes any(), which is order-independent -- so the winning caster's id can be
  // recorded and trusted, which is what P2-P4 spend on a 4x finer edge.
  if (u0 < 0.0 || u0 > 1.0) return 0.0;                // centre ray misses the card -> lit
  float uMid = u0;
  // ONE silhouette fetch, at the middle of the blocked span. The interval is the CARD's geometry; the
  // caster's actual outline still comes from the atlas. Approximating the silhouette as constant across
  // the span is exact in the umbra and on clean edges, and softens fine structure (a branch comb inside
  // one penumbra width gets one sample) -- the same limit the distance-field option in F5 addresses.
  return sampleCard(clamp(uMid, 0.0, 1.0), t0, W, H, rot, ppu, ox, oy, nx, ny, fo, fmin, fmax, surf);
}
// shadows-onto-billboards (attempt #3, IN-FAMILY): is world point P inside billboard's UPRIGHT drawn billboard, and
// opaque there? Returns the billboard's base tile ROW if so (drives the receiver elevation), else -1. Mirrors
// casterCover's billboard/def decode, but with NO light projection — the sprite is drawn parallel to the view, so
// (s,t) come straight from P's offset in the [Ac.x±W/2] × [Ac.y−H .. Ac.y] rect. Reads ONLY the data texture
// + surface atlas by index — never a textile_slot map by world coord — so it is zoom-stable by construction.
float receiverCover(uint billboardIdx, vec2 P, vec2 ref, highp usampler2D data, sampler2D surf, vec2 align, out float baseYOut) {
  baseYOut = 0.0;
  uvec4 Pd = fetchLin(data, BILLBOARD_BASE + int(billboardIdx));
  vec2 A = billboardPos(Pd, ref);                                     // F8: root decodes absolutely
  int defIdx = int((Pd.z >> 16) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  if (W <= 0.0 || H <= 0.0) return -1.0;
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);
  float ox = float((D.x >> 14) & 1023u), oy = float((D.x >> 4) & 1023u); // v3: both offsets in R
  uint lod = (D.z >> 4) & 15u;
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u);
  vec2 sh = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU, oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  uint rot = (Pd.y >> 26) & 3u;
  if (rot == 3u) sh.x = -sh.x;
  vec2 Ac = A + sh;                                         // tight-bbox base-centre (same as casterCover)
  Ac -= align;                                             // seat on the drawn albedo — shadow + lighting pass their OWN offset
  float s = (P.x - (Ac.x - 0.5 * W)) / W;                   // 0 left → 1 right of the drawn rect
  float t = (Ac.y - P.y) / H;                              // 0 at the base → 1 at the top (north)
  if (s < 0.0 || s > 1.0 || t < 0.0 || t > 1.0) return -1.0; // outside the drawn billboard → not this billboard
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
  // sub-0.5 anti-aliased ramp let a billboard's normal reach ~2px PAST its crisp silhouette onto the neighbour.
  return (texelFetch(surf, ivec2(uv), 0).b >= 0.5 ? 1.0 : 0.0) * bf; // presence (0/1) × base fade
}
// Which standing billboard is DRAWN at texel P, and its base row? Scan the caster buckets a few rows SOUTH (a
// billboard draws NORTH of its base, so the covering billboard's base sits at/south of the drawn texel), test each
// billboard's upright silhouette, and take the FRONTMOST (southmost = max row) cover. -1 = ground (no billboard drawn).
// The buckets + surface are contiguous/index-addressed (zoom-safe); this is the in-family replacement for the
// reverted attempt's zdepth-composite read.
float receiverAt(vec2 P, highp usampler2D data, sampler2D surf, vec2 align, out uint rbillboard, out float rcov) {
  int wc = int(floor(P.x / UPT));
  int r0 = int(floor(P.y / UPT));
  float best = -1.0;
  rbillboard = 0u;                                                // the winning (frontmost) receiver billboard — for self-exclusion
  rcov = 0.0;                                                // its soft silhouette coverage at P (the edge blend)
  for (int dy = 0; dy <= 5; dy++) {                         // constant bound; covers billboards up to ~5 tiles
    uvec4 cb = fetchLin(data, BILLBOARD_PRESENCE_BASE + foldTile(wc, r0 + dy));
    for (int c = 0; c < 4; c++) {
      uint slot = primSlot(cb, c);                          // D7 slots — the receiver scan reads the
      // P2 contract: slot 0 is the TILE's (may be empty); billboards fill DENSELY from slot 1.
      if (slot == 0u) { if (c == 0) continue; break; }
      if (((slot >> 24) & 15u) != ${SET_BILLBOARD_DATA}u) continue; // tile slot — not a standing billboard
      uint billboardIdx = slot & 0xffffu;
      float brOut; float cov = receiverCover(billboardIdx, P, (vec2(float(wc), float(r0 + dy)) + 0.5) * UPT, data, surf, align, brOut);
      if (cov > 0.0 && brOut > best) { best = brOut; rbillboard = billboardIdx; rcov = cov; } // frontmost cover wins row+billboard+cov
    }
  }
  return best;
}
// lightmap (CO-PACK): the world-frame NORMAL at texel P for the billboard DRAWN there. The four maps are quadrants
// of ONE co-packed atlas frame, so the NORMAL quadrant = the SURFACE frame origin + (side, -side) on the SAME
// page (uSurface) — identical frameRel to the silhouette in receiverCover, only the quadrant origin shifted.
// Frame-indexed (NOT a world-coord composite read) → zoom-stable; present at EVERY lod the surface is (same
// frame). Returns vec3(0) when unavailable (no silhouette lod / outside frame) → caller uses a flat up.
vec3 billboardNormal(uint billboardIdx, vec2 P, highp usampler2D data, sampler2D surf, vec2 align, out float sOut) {
  vec2 ref = P;   // the billboard is DRAWN at P, so P is within its own footprint (16-tile period is ample)
  sOut = -1.0;
  uvec4 Pd = fetchLin(data, BILLBOARD_BASE + int(billboardIdx));
  vec2 A = billboardPos(Pd, ref);                                     // F8: root decodes absolutely
  int defIdx = int((Pd.z >> 16) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  float W = float(((D.y >> 23) & 511u) * 2u);
  float H = float(((D.y >> 14) & 511u) * 2u);
  if (W <= 0.0 || H <= 0.0) return vec3(0.0);
  uint lod = (D.z >> 4) & 15u;
  if (lod < 4u) return vec3(0.0);                            // no silhouette lod → no usable frame
  float spanU = float((((D.y >> 10) & 15u) + 1u) * 16u);
  float ox = float((D.x >> 14) & 1023u), oy = float((D.x >> 4) & 1023u); // v3: both offsets in R
  float axf = float((D.z >> 2) & 3u), ayf = float(D.z & 3u);
  vec2 shf = vec2(ox + 0.5 * axf * W - 0.5 * axf * spanU, oy + 0.5 * ayf * H - 0.5 * ayf * spanU);
  uint rot = (Pd.y >> 26) & 3u;
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
// shadows-onto-billboards: ONE per-caster test shared by the corridor + brute walks (so both stay bit-identical —
// the P6 identity invariant). Q = the point whose shadow we sample (ground texel = the lifted texel; thing
// texel = the elevated pixel's ground projection G). Two THING-only culls (the user's cone method):
//   (0) SELF — the caster must not be the receiver's OWN billboard. The row cull (1) can't catch this on its own:
//       the receiver row derives from the tight-bbox base (Ac) while a caster's row derives from the raw
//       anchor (billboard.y+height), so "same billboard" is NOT "same row". Exact billboard-id match kills self-casting
//       (the user: billboard == billboard → cull).
//   (1) SEEN-FACE + NEAR BAND — the caster's base must be more than SELF_BAND units SOUTH of the receiver
//       base (Cb.y > Rbase.y + SELF_BAND), measured in UNITS. Billboards are seen from the south, so a
//       shadow only lands on the VISIBLE face when the caster is in front (south) of the receiver; a caster
//       north of / on / within SELF_BAND units of the receiver base is on nearly the same y (self / same
//       object) and would darken the unseen BACK — excluded (user: wider same-y band, in units).
//   (2) LIGHT-SIDE — the caster must sit between the light and the receiver: dot(Cb−Rbase, Rbase−L) < 0.
//       (A north light + a caster south of the receiver would otherwise false-positive.)
// Returns coverage + the caster row.
// pawn-render P2: a billboard record's HOT class bit (leaf G lane, bit 25 — the same bit hot
// lights carry). Hot prims participate in the HOT pass only.
bool billboardHot(uint billboardIdx, highp usampler2D data) {
  return ((fetchLin(data, BILLBOARD_BASE + int(billboardIdx)).y >> 25) & 1u) == 1u;
}
float casterOne(uint billboardIdx, vec2 Q, vec3 L, float emitter, float reachU, bool isThing, uint rbillboard, vec2 Rbase,
                int casterMode, vec2 ref, highp usampler2D data, sampler2D surf, out float row) {
  row = 0.0;
  if (billboardIdx == 0u) return 0.0;
  uvec4 cPd = fetchLin(data, BILLBOARD_BASE + int(billboardIdx));   // ONE fetch — this is the corridor's
  // pawn-render P2/P3 caster filter: mode 1 (the COLD pass) skips HOT casters — a mover's
  // shadow never bakes cold; mode 2 (the hot pass's cold-light DELTA) walks HOT casters ONLY —
  // the correction subtracts exactly the mover's added occlusion. Mode 0 walks all.
  uint cHot = (cPd.y >> 25) & 1u;
  if (casterMode == 1 && cHot == 1u) return 0.0;
  if (casterMode == 2 && cHot == 0u) return 0.0;
  vec2 Cb = billboardPos(cPd, ref);        // F8: hot loop, per caster per light — root is exact
  if (isThing) {
    if (billboardIdx == rbillboard) return 0.0;                       // (0) self — exact same billboard → no self-cast
    // ns-shadows P1 (fork F1): gate (1) is the E/W card's seen-face asymmetry (a caster NORTH of a
    // receiver sits behind its seen face). A perpendicular (n/s) caster throws E/W — there is no
    // north/south asymmetry to protect, and the band would kill its climb onto e/w neighbours whose
    // bases share its row. Gates (0) and (2) still apply.
    uint cRot = (cPd.y >> 26) & 3u;
    bool nsCaster = (cRot == 0u || cRot == 2u) && ((cPd.w >> 14) & 1u) == 1u;
    if (!nsCaster && Cb.y <= Rbase.y + ${SELF_BANDF}) return 0.0;   // (1) seen-face + near-band (units): caster must be >SELF_BAND south
    if (dot(Cb - Rbase, Rbase - L.xy) >= 0.0) return 0.0;   // (2) caster must block the light reaching it
  }
  float cc = casterCover(billboardIdx, Q, L, emitter, reachU, ref, data, surf);
  if (cc > 0.0) row = floor(Cb.y / UPT);
  return cc;
}
// One light's shadow at sample point Q — the corridor (corr=1) or brute (corr=0) walk of the caster buckets,
// MAX-accumulating casterOne. Factored so a texel can evaluate BOTH the ground point AND the elevated thing
// point and blend them at the silhouette edge. corr selects the path; the two must stay bit-identical (P6).
// reachT bounds the brute box (unused by the corridor). In GATHER_COMMON so the lightmap bake can re-run it at
// fine res to sharpen the shadow edge (shadow-edge-refine).
// PROFILING STAIRCASE for the WALK itself (2026-07-27, moving-lights I11). 0 = full; 1..4 cut it short so a
// GPU timer prices each substep by difference. Each level includes every level below it:
//   1 DDA setup only   2 + tile traversal (no bucket fetch)   3 + bucket fetches (no slot unpack)
//   4 + slot unpack (no casterOne)   0 + casterOne
// Levels 3 and 4 fold their fetched value into cov with a tiny weight ONLY to defeat dead-code elimination —
// without that the compiler removes the very fetch we are trying to price. Output is meaningless below 0.
uniform int uWProfile;
// The winner out-param reports WHICH caster occluded -- 0xffff when none. Well defined under binary
// any-semantics: the walk stops at the first occluder, so the winner IS that caster. It is what the
// texel stores (v5) and what a later refine re-tests instead of searching the corridor again.
float walkShadow(vec3 L, float emitter, vec2 Q, bool isThing, uint rbillboard, vec2 Rbase,
                 int casterMode, int corr, int reachT, highp usampler2D data, sampler2D surf,
                 out float cdepth, out uint winner) {
  cdepth = 0.0;
  winner = 0xffffu;
  float cov = 0.0;
  // Reach in UNITS, the radial cap for a shadow whose light sits at/below the caster top (I38). Derived
  // from the walk's own tile reach so the cap can never exceed the region the walk visits.
  float reachU = float(reachT) * UPT;
  if (corr == 1) {
    // SUPERCOVER DDA (I30). The old walk SAMPLED the light→Q segment at ≤1-tile spacing and padded every
    // sample with a 5-tile cross, because a sample POINT can miss a tile the segment actually crosses near a
    // corner. That pad was ~59% of the walk's fetches, and it covered SAMPLING misses only — NOT caster
    // extent: buildCasters buckets each caster into EVERY tile its ground footprint spans (rows
    // topY..baseY, tight-bbox cols — see I-7), so any caster able to shadow Q is registered in some tile the
    // segment crosses. Walking the EXACT crossed set therefore needs no pad at all.
    // Amanatides-Woo: advance whichever axis boundary comes first in the segment parameter t ∈ [0,1].
    vec2 a = L.xy / UPT;                                     // light in tile coords
    vec2 b = Q / UPT;                                        // sample point in tile coords (ground = P, thing = G)
    vec2 d = b - a;
    ivec2 ct = ivec2(floor(a));                              // current tile — starts at the light's
    ivec2 te = ivec2(floor(b));                              // end tile — the receiver's
    ivec2 stp = ivec2(d.x > 0.0 ? 1 : (d.x < 0.0 ? -1 : 0),
                      d.y > 0.0 ? 1 : (d.y < 0.0 ? -1 : 0));
    // t advanced per whole tile on each axis, and the t of the FIRST boundary crossing. BIG stands in for
    // "this axis never crosses", so an axis with no motion always loses the comparison below.
    const float BIG = 1e30;
    vec2 adv = vec2(d.x != 0.0 ? abs(1.0 / d.x) : BIG, d.y != 0.0 ? abs(1.0 / d.y) : BIG);
    float fx = d.x > 0.0 ? (float(ct.x + 1) - a.x) : (a.x - float(ct.x)); // tiles to the first x boundary
    float fy = d.y > 0.0 ? (float(ct.y + 1) - a.y) : (a.y - float(ct.y));
    vec2 tnext = vec2(d.x != 0.0 ? fx * adv.x : BIG, d.y != 0.0 ? fy * adv.y : BIG);
    int nvisit = abs(te.x - ct.x) + abs(te.y - ct.y) + 1;    // exact size of the 4-connected cover
    // PERPENDICULAR-ONLY dilation. The old 5-tile cross was measured to be load-bearing, NOT redundant: an
    // exact DDA under-covers (1792 texels of shadow found by brute and missed), because a caster is bucketed
    // by its TIGHT-BBOX ground cover while casterCover tests a wider projected extent, so a caster in tile T
    // can occlude a ray through T±1. But the cross is only needed ACROSS the ray: consecutive walk tiles
    // already supply each other's ±1 along the direction of travel. Dilating perpendicular to the dominant
    // axis keeps the conservative cover at 3 fetches/tile instead of 5.
    ivec2 perp = abs(d.x) >= abs(d.y) ? ivec2(0, 1) : ivec2(1, 0);
    if (uWProfile == 1) return 0.0;                          // W1: DDA setup only
    for (int m = 0; m < 64; m++) {                           // CONSTANT bound — a body-modified var in a
      if (m >= nvisit) break;                                // loop CONDITION can miscompile (known trap).
      if (uWProfile != 2)                                    // W2: traverse the tiles, fetch nothing
      for (int n = 0; n < ${DILATE}; n++) {                  // centre, +perp, -perp (DILATE 5 = the pre-I30
        ivec2 o = ct + (${DILATE} == 5                       // 5-tile CROSS, kept switchable for A/B)
          ? ivec2(n == 1 ? 1 : (n == 2 ? -1 : 0), n == 3 ? 1 : (n == 4 ? -1 : 0))
          : (n == 1 ? perp : (n == 2 ? -perp : ivec2(0))));
        // D9: the visited tile + uDilateS rows SOUTH — occupancy registers a caster on its BASE
        // LINE only, so the tall-card overhang the old extent bucketing pre-registered is found
        // by looking at the southern tiles whose occupants can lean over this one.
        for (int sr = 0; sr < 8; sr++) {                     // constant bound (loop-condition trap)
          if (sr > uDilateS) break;
          ivec2 os = ivec2(o.x, o.y + sr);
          uvec4 cb = fetchLin(data, BILLBOARD_PRESENCE_BASE + foldTile(os.x, os.y));
          vec2 bref = (vec2(os) + 0.5) * UPT;                // resolve casters against THEIR tile
          if (uWProfile == 3) { cov = max(cov, float(cb.x & 1u) * 1e-6); continue; } // W3: fetched, no unpack
          for (int c = 0; c < 4; c++) {
            uint slot = primSlot(cb, c);
            // DENSE EARLY OUT (P2 contract: slot 0 = the tile's — may be empty; billboards
            // fill densely from slot 1).
            if (slot == 0u) { if (c == 0) continue; break; }
            if ((slot & 0x80000000u) == 0u) continue;        // cast_shadows flag — zero-fetch cull
            if (((slot >> 24) & 15u) != ${SET_BILLBOARD_DATA}u) continue; // only billboard casters walk (tiles-cast is future wall-shadow work)
            uint billboardIdx = slot & 0xffffu;
            if (uWProfile == 4) { cov = max(cov, float(billboardIdx & 1u) * 1e-6); continue; } // W4: no casterOne
            float r; float cc = casterOne(billboardIdx, Q, L, emitter, reachU, isThing, rbillboard, Rbase, casterMode, bref, data, surf, r);
            // ANY, not MAX (2026-07-30). With binary coverage these are the same operator, so this is
            // not a behaviour change -- it is the licence to STOP. One occluder is a complete answer,
            // so the remaining casters cannot change the result and need not be tested.
            if (cc > 0.0) { cov = 1.0; cdepth = max(cdepth, r); winner = billboardIdx; break; }
          }
          if (cov > 0.0) break;
        }
        if (cov > 0.0) break;
      }
      if (cov > 0.0) break;   // NOTE: a body statement, never a loop CONDITION -- a body-modified var
                              // in a for-condition is the documented miscompile trap in this file.
      if (tnext.x < tnext.y) { ct.x += stp.x; tnext.x += adv.x; }
      else                   { ct.y += stp.y; tnext.y += adv.y; }
    }
  } else {
    ivec2 lc = ivec2(floor(L.xy / UPT));                     // light's tile
    for (int dy = -16; dy <= 20; dy++) {
      // D9: the brute box extends uDilateS rows FURTHER SOUTH — a base just past reach can
      // still lean its card north INTO reach (the extent rows the old bucketing pre-spanned).
      if (dy < -reachT || dy > reachT + uDilateS) continue;
      for (int dx = -16; dx <= 16; dx++) {
        if (dx < -reachT || dx > reachT) continue;
        uvec4 cb = fetchLin(data, BILLBOARD_PRESENCE_BASE + foldTile(lc.x + dx, lc.y + dy));
        vec2 bref = (vec2(float(lc.x + dx), float(lc.y + dy)) + 0.5) * UPT;
        for (int c = 0; c < 4; c++) {
          uint slot = primSlot(cb, c);
          if (slot == 0u) { if (c == 0) continue; break; }   // slot 0 = tile; billboards dense from 1
          if ((slot & 0x80000000u) == 0u) continue;          // cast_shadows flag
          if (((slot >> 24) & 15u) != ${SET_BILLBOARD_DATA}u) continue; // only billboard casters walk
          uint billboardIdx = slot & 0xffffu;
          float r; float cc = casterOne(billboardIdx, Q, L, emitter, reachU, isThing, rbillboard, Rbase, casterMode, bref, data, surf, r);
          if (cc > 0.0) { cov = 1.0; cdepth = max(cdepth, r); winner = billboardIdx; break; }   // ANY -- see the corridor arm
        }
        if (cov > 0.0) break;
      }
      if (cov > 0.0) break;
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
// cols·TEXTILE_UNIT × rows·TEXTILE_UNIT, same fc→world→slot mapping, same dirty-rect draw list → clean
// tiles persist by never being rasterized). Each texel: ambient + Σ over its presence lights of colour·intensity·falloff. The display
// blit multiplies albedo × lightmap. P1 = emission only (no normal, no shadow); P2 folds Lambert
// N·L; P3 masks each light by its shadow-cold u9 coverage. Reuses GATHER_COMMON (fetchLin / foldTile /
// decodePos / tileSlot / SQ·UNIT·UPT + the data-texture BASE consts).
const LIGHT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;    // presence (sets 3/5) + light records (set 2)
uniform highp usampler2D uRecvFine; // baked FINE receiver map (P3): id(16)|presence(1) — replaces the scan
uniform highp usampler2D uShadow;  // this class's shadow RT (COARSE — upsampled per fine texel) — per-slot u9 coverage
uniform highp usampler2D uShadowCold; // the COLD class's shadow RT — the P3 delta's reference (cold-caster coverage)
uniform int uLightClass;           // #4: accumulate only this class of light — 0 = COLD, 1 = HOT
uniform int uWorldLight;           // world-space-lighting: 1 = TRUE-3D falloff distance (oval + light height), 0 = screen circle
uniform float uNsInv;              // world-space-lighting: N–S un-foreshorten factor (1/cos65 ≈ 2.366; live-tunable)
uniform float uFalloff;            // falloff exponent (<1 lifts the mid-range). Live via __falloff(e); 0/1 stay fixed
                                   // points of pow(), so no exponent can leak light past a light's reach.
uniform float uWarmShift;          // lighting-feel P1 (__lighttemp): core warm shift (R up / B down). 0 = identity.
uniform float uRimSat;             // lighting-feel P1 (__lighttemp): rim saturation (1 = identity, <1 desaturates).
uniform float uGlintStr;           // lighting-feel P1 (__glint): specular strength on things. 0 = off.
uniform float uGlintPow;           // lighting-feel P1 (__glint): Blinn exponent.
uniform sampler2D uSurface;        // lightmap (CO-PACK): the shared sprite atlas — silhouette (surface quadrant) AND normal (normal quadrant)
uniform int uShowNormal;           // lightmap P0 debug (__shownormal): 1 = paint the sampled billboard normal into oLight
uniform float uNormalPitch;        // lightmap (__pitchnormal): standing-billboard normal pitch (rad, 90°−tilt; live, F7-reconsidered)
uniform vec2 uLightAlign;          // lightmap (__lightalign): the LIGHTING receiver-mask seat (units, NW), decoupled from the shadow's RECV_ALIGN
// PROFILING STAIRCASE (2026-07-27, moving-lights I9). 0 = full shader. 1..4 cut the fragment short at a named
// boundary so a GPU timer can price each substep by difference — this pass is ONE draw, so it cannot be
// timed any other way. Each level includes every level below it:
//   1 dirty gate + window mapping + P   2 + receiverAt   3 + billboard/world normal
//   4 + accumulateLights with NO shadow fetch   0 + coarse shadow lookup
// (level 5 was "no edge refine"; the refine is deleted, so 5 and 0 are the same shader.)
uniform int uProfile;
uniform int uHideRight;            // DEBUG (__hideright): 1 = blank the right half of each billboard's normal in __shownormal
uniform int uShadowFilter;         // shadow-polish P3 (__shadowfilter): 1 = bilinear coarse-shadow upsample, 0 = the old NEAREST
// lightmap P1: sprite-tangent normal to the WORLD frame. Rotate the flat/ground basis (image-up = north, out
// = up) up by phi about the EAST axis: phi 0 = ground (lies in the plane), phi = 90deg minus tilt = a standing
// billboard treated perpendicular to the ground. Image +Y is sprite-north = world -y, so it flips in.
vec3 worldNormal(vec3 n, float phi) {
  float c = cos(phi), s = sin(phi);
  // The card's OUT axis (n.z, toward the viewer) IS world SOUTH (+y) — a screen-facing billboard's front faces
  // the viewer / a south light. Rotate the (out, up-card) pair by the pitch φ: SOUTH = n.z·c − n.y·s, UP (+z) =
  // n.z·s + n.y·c. (The earlier −(n.y·c+n.z·s) form gave an inverted N·L — trees toward the light read dark.)
  return vec3(n.x, (n.z * c - n.y * s), (n.z * s + n.y * c));
}
layout(location = 0) out vec4 oLight;  // lightmap P1: SINGLE attachment — Σ colour·intensity·falloff·(1−shadow)·N·L (no ambient)
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
uint lane4(uvec4 v, int c) { return c == 0 ? v.x : (c == 1 ? v.y : (c == 2 ? v.z : v.w)); }
const int FINE = ${FINE_RATIO};        // fine light texels per coarse shadow texel (TEXTILE_LIGHT/TEXTILE_UNIT)
const float QUANT = ${LIGHT_QUANT}.0;  // additive-lightmap quantisation step (F11b) — deposits are INTEGERS

// The per-texel light accumulation, with its DATA and SHADOW sources as PARAMETERS (F11b.1 / [I36]).
//
// Parameterised so ONE definition can be evaluated against the CURRENT state and against last frame's
// snapshot (uDataPrev / uShadowPrev) in the same pass — which is what lets the differential emit
// new-minus-old and have the old term be exactly what was originally deposited. Note walkShadow also
// takes the data parameter: the old term must walk the OLD casters, or a moved caster desyncs the subtract.
//
// While the differential is not yet wired this is called once with the current textures, so it is a
// pure refactor — identical output, no behaviour change.
vec3 accumulateLights(highp usampler2D data, highp usampler2D shadowTex, highp usampler2D shadowColdTex,
                      ivec2 sb, vec2 sf,
                      vec2 P, int fold, vec3 N, bool applyNL, uint rbillboardN, bool recvHot) {
    uvec4 presLo = fetchLin(data, PRESENCE_BASE + fold);     // lights 0–6
    // shadow-polish P3 (user): BILINEAR shadow upsample. The shadow textile is UNIT
    // resolution while this map is fine — the old single NEAREST fetch (fc/FINE) gave every
    // fine texel its coarse parent's word with zero fractional consideration, so shadows
    // moved in unit steps and silhouette-straddling texels wore their neighbour's value
    // (the pinned squares). Four corner words, per-slot u7 blend below; sb/sf arrive
    // from the caller with the torus-wrap and RT-edge guards already applied (a clamped
    // weight degrades to nearest exactly at seams where texel adjacency is not world
    // adjacency). The integer RT cannot hardware-filter — this IS the filter.
    bool p4 = uProfile == 4;                                  // L4: price the loop WITHOUT shadow fetches
    uvec4 sh   = p4 ? uvec4(0u) : texelFetch(shadowTex, sb, 0);
    uvec4 sh10 = p4 || sf.x <= 0.0 ? sh : texelFetch(shadowTex, sb + ivec2(1, 0), 0);
    uvec4 sh01 = p4 || sf.y <= 0.0 ? sh : texelFetch(shadowTex, sb + ivec2(0, 1), 0);
    uvec4 sh11 = p4 || (sf.x <= 0.0 && sf.y <= 0.0) ? sh : texelFetch(shadowTex, sb + ivec2(sf.x > 0.0 ? 1 : 0, sf.y > 0.0 ? 1 : 0), 0);
    // pawn-render P3 (the delta): the COLD class's shadow map, for cold-light corrections in
    // the hot pass — correction = -light * max(0, shadowHotOnly - shadowCold).
    uvec4 shC   = p4 ? uvec4(0u) : texelFetch(shadowColdTex, sb, 0);
    uvec4 shC10 = p4 || sf.x <= 0.0 ? shC : texelFetch(shadowColdTex, sb + ivec2(1, 0), 0);
    uvec4 shC01 = p4 || sf.y <= 0.0 ? shC : texelFetch(shadowColdTex, sb + ivec2(0, 1), 0);
    uvec4 shC11 = p4 || (sf.x <= 0.0 && sf.y <= 0.0) ? shC : texelFetch(shadowColdTex, sb + ivec2(sf.x > 0.0 ? 1 : 0, sf.y > 0.0 ? 1 : 0), 0);
    vec3 acc = vec3(0.0);                                     // #4: NO ambient here — the blit adds it once over cold+hot
    // lighting-feel P1: the world-frame VIEW direction for the glint — per-fragment constant, hoisted
    // out of the light loop (cos/sin once, not per light).
    vec3 V = vec3(0.0, cos(uNormalPitch), sin(uNormalPitch));
    for (int slot = 0; slot < 8; slot++) {
      uint li = tileSlot(presLo, slot);   // F7: one presence word, 8 slots
      if (li == 0xffffu) continue;                            // empty slot
      uvec4 Ld = fetchLin(data, LIGHT_BASE + int(li));       // G = position, B = colour|intensity, A = z|reach|hot|…
      // pawn-render P2 (the tier matrix): cold pass = cold lights only; hot pass = hot lights
      // everywhere + cold lights on HOT-RECEIVER texels (a cold torch LIGHTS the wolf via the
      // hot map — the blit zeroes the cold map on mover pixels, so no double-count).
      uint lcls = (Ld.y >> 25) & 1u;
      if (uLightClass == 0 && lcls != 0u) continue;   // cold pass: cold lights only (as ever)
      // hot pass: hot lights everywhere; a cold light contributes FULLY on hot-receiver texels
      // and as a NEGATIVE delta elsewhere (P3 — the mover's added occlusion of a cold-baked
      // light; the blit's cold+hot sum then carves the mover's shadow out of the cold pool).
      bool deltaMode = uLightClass == 1 && lcls == 0u && !recvHot;
      vec2 Lxy = resolvedPos((Ld.w >> 4) & 255u, (Ld.x >> 8) & 255u, Ld.x & 255u, P); // v3 resolved pos
      vec3 col = vec3(float((Ld.z >> 24) & 255u), float((Ld.z >> 16) & 255u), float((Ld.z >> 8) & 255u)) / 255.0;
      float intensity = float(Ld.z & 255u) / 255.0;
      float reach = float((Ld.w >> 20) & 0xfffu);             // reach (units)
      vec2 toL = Lxy - P;
      // world-space-lighting P1: TRUE 3D distance instead of the screen radius — the world is a 65° plane, so
      // the N–S screen axis is foreshortened (un-shorten by uNsInv = 1/cos65) and the light sits Lz above the
      // ground. ⟹ the reach is an OVAL (flattened N–S) + a height term (a point under the light isn't at 0).
      // Ground assumption (Pz=0); billboard elevation folds in at P2. Toggle off = the old screen circle.
      float Lz = float((Ld.y >> 16) & 255u);                  // v3: z_offset (units) moved to G
      float dist; vec3 d3;                                     // world-space delta P→light (for dist + N·L dir)
      if (uWorldLight == 1) {
        d3 = vec3(toL.x, toL.y * uNsInv, Lz);                 // un-foreshorten N–S + light height
        dist = length(d3);                                    // true 3D distance (ground point)
      } else {
        d3 = vec3(toL, 0.0);
        dist = length(toL);                                   // in-plane screen distance (units)
      }
      // 1 at the light → 0 at reach (F2 smoothstep), then FLATTENED by a <1 exponent so the mid-range
      // lifts (0.25 → 0.5 at exp 0.5) and the pool reads bright most of the way out instead of falling off
      // right outside the source. Endpoints are fixed points of pow(), so this brightens the interior
      // WITHOUT extending the light past its reach — the corridor/brute reach box stays exactly valid.
      float fall = pow(smoothstep(reach, 0.0, dist), uFalloff);
      // lightmap P1: per-light Lambert on THINGS — fold max(0, N·L̂) into the contribution so the lightmap stores
      // Σ colour·falloff·(1−shadow)·N·L per light (no lossy aggregate-direction relief). CLIP-TO-PRESENCE: the
      // fine lightmap (64/tile) is still coarser than the sprite edge, so a hard thing/ground classification spills
      // one texel past the silhouette. Blend N·L → ground (ndl 1) by the sprite's soft coverage (rcovN) so the lit
      // region fades exactly to presence — no coarse fringe, and the edge reveals ground not black. ndl = 1 on ground.
      float ndl = applyNL ? max(dot(N, d3 / max(dist, 1e-3)), 0.0) : 1.0;
      // P3 SHADOW: slot i's u8 coverage (low8 in channel i>>2) + the elevated-path flag in bit 0.
      // The ON-BILLBOARD CUT is RETIRED (texture-generalization D8): a receiver texel's coverage is
      // computed FOR its receive mode (billboard texels bake the CLIMBING shadow, never the ground
      // pool), so cutting ground coverage off billboard pixels no longer has a job — the prim pass's
      // fine overwrite handles the silhouette edge, as it already did for the deleted edge-refine.
      // shadow-polish P3: the per-slot u7 blended across the four corner words (the flag
      // bit 0 is documentation-only here; nothing consumes it in this loop).
      float shadow = mix(
        mix(shadowOcc(sh,   slot), shadowOcc(sh10, slot), sf.x),
        mix(shadowOcc(sh01, slot), shadowOcc(sh11, slot), sf.x),
        sf.y);   // v4: OCCLUDED is bit 1 of the slot byte -- shift straight to it, 0/1, no 127 scale
      // The shadow-edge REFINE lived here and is DELETED (2026-07-27, moving-lights F7). It re-ran the full
      // walkShadow corridor **per fine texel** on every (0,1) edge value to sharpen the upsampled edge —
      // 8.39 M fine texels against the shadow map's 131 k, a 64× resolution multiplier, INSIDE the per-light
      // loop. Measured at **9.29 ms of a 10.88 ms pass — 85 %** (I9), for ONE light.
      //
      // It is not optimised, it is gone, because the boundary it recomputed is one the rasteriser gives away.
      // The only fine edge that matters is the BILLBOARD SILHOUETTE (where bright ground would leak out from
      // under a trunk), and the prim pass draws that at fine resolution masked by the sprite's own coverage —
      // so **the overwrite IS the fine cut**. The blocky edge that remains on open ground is accepted (user).
      // Nothing replaces this: the coarse shadow nearest-upsampled across its 8×8 block already overdraws.
      // lighting-feel P1: COLOUR TEMPERATURE over falloff. The falloff already dims toward the rim;
      // this shifts HUE with it — warm (R up, B down) at the core, desaturated toward grey at the
      // rim — so a fire pool reads as fire instead of a uniform orange disk. Identity at
      // (uWarmShift 0, uRimSat 1), which is the A/B toggle. The per-light clamp below still bounds
      // every channel at 1, so the exactness budget is untouched.
      float lum = dot(col, vec3(0.299, 0.587, 0.114));
      vec3 colT = mix(mix(vec3(lum), col, uRimSat),                      // rim: desaturated
                      col * vec3(1.0 + uWarmShift, 1.0, 1.0 - uWarmShift), // core: warmed
                      fall);
      // lighting-feel P1: SPECULAR GLINT — Blinn N·H on THINGS only (ground has no normal yet).
      // V is the world-frame view direction (the card's out-axis pitched by uNormalPitch); H the
      // half-vector to this light. Rides the same falloff + shadow as the diffuse term and joins
      // it INSIDE the per-light clamp. Off at uGlintStr 0 (the A/B toggle).
      float spec = 0.0;
      if (applyNL && uGlintStr > 0.0) {
        vec3 H = normalize(d3 / max(dist, 1e-3) + V);
        spec = pow(max(dot(N, H), 0.0), uGlintPow) * uGlintStr;
      }
      // hot-sync P4 (ground shadow BEFORE the billboard — user): the HOT map is a pure
      // CORRECTION over the cold-baked ground, at EVERY texel. The blit sums cold+hot
      // unconditionally (no per-pixel cold zeroing), so the ground pool with its carved
      // shadow is the base layer and the mover's body lighting rides on top as a delta —
      // the sprite overlays at pixel precision, wolf over its shadow by construction.
      vec3 dep;
      if (uLightClass == 1 && lcls == 0u) {
        float shadowCold = mix(
          mix(shadowOcc(shC,   slot), shadowOcc(shC10, slot), sf.x),
          mix(shadowOcc(shC01, slot), shadowOcc(shC11, slot), sf.x),
          sf.y);   // v4: OCCLUDED bit, 0/1
        if (recvHot) {
          // BODY-over-ground correction: deposit body − ground so cold+hot == the body value.
          // The ground term mirrors EXACTLY what the cold pass baked at this texel: it ran
          // with the receiver demoted to ground (rbillboard forced 0) — ndl 1, no glint, NO
          // billboard shadow cut — so the subtraction leaves precisely the body lighting.
          float body = intensity * fall * (ndl + spec) * (1.0 - shadow);
          float ground = intensity * fall * (1.0 - shadowCold);
          dep = clamp(colT * body, vec3(0.0), vec3(1.0)) - clamp(colT * ground, vec3(0.0), vec3(1.0));
        } else {
          // The cold map baked X·(1−shC); truth is X·(1−max(shC, shHotOnly)). The correction is
          // −X·max(0, shHotOnly − shC) — exact where the mover adds occlusion, zero where the
          // cold casters already shadowed. The shadow var here IS the hot-only coverage (mode 2).
          // (The ground-cut mirror is gone with the cut itself — D8.)
          float contrib = -intensity * fall * (ndl + spec) * max(0.0, shadow - shadowCold);
          dep = clamp(colT * contrib, vec3(-1.0), vec3(1.0));
        }
      } else {
        float contrib = intensity * fall * (ndl + spec) * (1.0 - shadow); // shadowed contribution (× Lambert on things)
        dep = clamp(colT * contrib, vec3(-1.0), vec3(1.0));
      }
      // CLAMP PER LIGHT, before it joins the sum. This is what makes the accumulator's exactness bound
      // UNCONDITIONAL rather than merely likely: with every light capped at 1.0 (255 after quantisation),
      // the worst case is 65,535 (the whole u16 id space) x 255 = 16,711,425, under the 2^24 = 16,777,216
      // that FP32 represents exactly. No presence cap, no distribution assumption, no bookkeeping
      // discipline needed — overflow becomes impossible by construction. (The body-over-ground form
      // clamps each of its two terms to [0,1] before differencing — same bound, same removal exactness.)
      //
      // It costs no brightness. Clamping caps a light's PEAK, not its profile: at intensity 4 every
      // distance is still 4x, so the core saturates over a WIDER radius and the falloff stays brighter
      // further out — which is what a brighter light looks like. So intensity is free to exceed 1.
      //
      // The clamp belongs HERE and nowhere downstream. Clamping the accumulated SUM would break light
      // removal: two lights at 255 clipped to 255 means subtracting one leaves 0 where 255 is correct.
      acc += dep;
    }
  return acc;
}

void main() {
  // Same window mapping as the shadow gather (constants row): fc → toroidal slot → world tile → P.
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  // G = u16 rows | u2 lod (14–15) | u14 slot (0–13) — mask the lod OFF the slot (textile-slot P2).
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0x3FFFu);
  int uLod = int((C0.y >> 14) & 3u);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  int uSlotF = uSlot * FINE;                                   // this map is FINE (TEXTILE_LIGHT/tile); shadow stays coarse
  ivec2 fc = ivec2(gl_FragCoord.xy);                           // FINE light texel
  int sx = fc.x / uSlotF, sy = fc.y / uSlotF;                  // owning tile (fine slot)
  // Dirty gate RETIRED (standing-costs P2) — the dirty-rect geometry IS the gate (see GATHER_FRAG).
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols);
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlotF)) / float(uSlotF);
  float ly = (float(fc.y) - float(sy * uSlotF)) / float(uSlotF);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS
  // shadow-polish P3: bilinear base + fractional weights into the COARSE shadow textile,
  // guarded where texel adjacency is not world adjacency — the torus wrap slot (the
  // window's origin column/row: its west/north texel neighbour is the window's FAR edge)
  // and the RT borders. A zeroed weight degrades that axis to nearest.
  vec2 sfp = (vec2(fc) + 0.5) / float(FINE) - 0.5;
  ivec2 sb = ivec2(floor(sfp));
  vec2 sf = uShadowFilter == 1 ? sfp - vec2(sb) : vec2(0.0);  // A/B: 0 = the old NEAREST read
  if (sb.x < 0) { sb.x = 0; sf.x = 0.0; }
  if (sb.y < 0) { sb.y = 0; sf.y = 0.0; }
  if (sb.x + 1 >= uCols * uSlot) sf.x = 0.0;
  if (sb.y + 1 >= uRows * uSlot) sf.y = 0.0;
  int wrapX = pmod(uWinCol, uCols), wrapY = pmod(uWinRow, uRows);
  if ((sb.x + 1) / uSlot != sb.x / uSlot && (sb.x + 1) / uSlot == wrapX) sf.x = 0.0;
  if ((sb.y + 1) / uSlot != sb.y / uSlot && (sb.y + 1) / uSlot == wrapY) sf.y = 0.0;
  if (uProfile == 1) { oLight = vec4(0.0); return; }           // L1: dirty gate + window mapping + P only

  // lightmap P0 (__shownormal): verify the atlas-frame normal read — paint the billboard's sampled normal (enc
  // 0.5+0.5) where a billboard is drawn, black on ground. Frame-indexed → must be rock-stable across zoom.
  if (uShowNormal == 1) {
    uint rp = texelFetch(uRecvFine, fc, 0).x & 0xffffu;   // P3: baked receiver id
    float sD;
    vec3 n = rp != 0u ? billboardNormal(rp, P, uData, uSurface, uLightAlign, sD) : vec3(0.0);
    // DEBUG (__hideright): blank the RIGHT half of each billboard's normal (sprite-relative s > 0.5) so the LEFT
    // half can be compared edge-to-edge against the albedo.
    if (uHideRight == 1 && rp != 0u && sD > 0.5) { oLight = vec4(0.0, 0.0, 0.0, 1.0); return; }
    oLight = vec4(n * 0.5 + 0.5, 1.0);
    return;
  }

  // lightmap P1: the WORLD-frame normal for per-light N·L. Only THINGS get N·L for now (ground keeps falloff,
  // ndl = 1 — the old behaviour; ground-as-billboards N·L is a later step). The corpus normal is RAW (camera-facing);
  // worldNormal pitches it to the world frame IN-SHADER (uNormalPitch = 90°−tilt, kept live — F7 reconsidered).
  // P3: one baked-map fetch replaces the receiverAt scan. The normal is still fetched LIVE below —
  // on a KNOWN billboard that's 2 record fetches + 1 atlas fetch, and it keeps uNormalPitch live.
  uint RF = texelFetch(uRecvFine, fc, 0).x;
  uint rbillboardN = RF & 0xffffu;
  // pawn-render P2: the COLD pass sees a HOT receiver (a mover) as GROUND — its texels bake the
  // terrain beneath; the HOT pass owns the mover's own lighting (normal, N·L, shadows).
  bool recvHotN = rbillboardN != 0u && billboardHot(rbillboardN, uData);
  if (uLightClass == 0 && recvHotN) rbillboardN = 0u;
  float rcovN = (RF & 0x10000u) != 0u ? 1.0 : 0.0;                  // presence bit (debug L2 only)
  if (uProfile == 2) { oLight = vec4(rcovN); return; }              // L2: + receiver fetch
  float sDbg;
  // texture-generalization P4: where no billboard is drawn, the TILE's def supplies the
  // normal (walls shade directionally under a torch) — vec3(0) on plain/loose tiles keeps
  // the old flat-ground behaviour (ndl = 1).
  vec3 pn = rbillboardN != 0u ? billboardNormal(rbillboardN, P, uData, uSurface, uLightAlign, sDbg)
                              : tileNormal(wc, wr, P, uData, uSurface);
  bool applyNL = dot(pn, pn) > 0.0;                                 // a loaded normal → real N·L
  vec3 N = applyNL ? worldNormal(pn, uNormalPitch) : vec3(0.0, 0.0, 1.0);
  if (uProfile == 3) { oLight = vec4(N, 1.0); return; }             // L3: + billboard/world normal

  int fold = foldTile(wc, wr);
  vec3 acc = accumulateLights(uData, uShadow, uShadowCold, sb, sf, P, fold, N, applyNL, rbillboardN, recvHotN);
  // QUANTISED into the additive accumulator (F11b). Rounding is what makes each deposit an exact integer,
  // so removing this light later — same value negated — cancels bit-exactly in FP32. Do NOT drop the
  // round for "smoother" values: the exactness is the correctness mechanism, and unquantised deposits
  // leave residue that reads as light which will not turn off.
  //
  // No LDR clamp: the accumulator is the SUM over lights and may legitimately exceed one light's range.
  // The blit divides by QUANT and tonemaps at display time instead.
  // P3: the HOT class may hold NEGATIVE deposits (the cold-light delta) — clamp only the
  // cold class (whose sums are non-negative by construction); the blit floors the total.
  oLight = vec4(round((uLightClass == 1 ? acc : max(acc, vec3(0.0))) * QUANT), 1.0);
}
`;

// ── RECEIVER MAP bakes (standing-costs P3) ────────────────────────────────────────────────────────
// receiverAt + the corner test are LIGHT-INDEPENDENT: they change only when a BILLBOARD changes, yet
// the gather + lighting passes re-derived them per texel on every light-motion re-bake (a 6-row
// bucket scan with full record decodes, per texel, per frame). These two passes bake them ONCE into
// persistent maps under their own receiver-dirty channel ([F2] in the stream's forks): light moves
// can NOT dirty them. Values are the EXACT outputs of the same functions at the same P, stored as
// raw f32 bits — the consumers reconstruct coverage with the identical expression, so the pipeline
// stays bit-identical to the live-scan build (verified by fixture hash).
// COARSE (shadow-RT res) — everything GATHER_FRAG needs: baseY, rcov, id, allBillboard.
const RECEIVER_COARSE_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;
uniform sampler2D uSurface;
layout(location = 0) out uvec4 oRecv; // R = f32 bits baseY | G = id(16)|allB(1) | B = f32 bits rcov
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
void main() {
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0x3FFFu);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlot, sy = fc.y / uSlot;
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols);
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlot)) / float(uSlot);
  float ly = (float(fc.y) - float(sy * uSlot)) / float(uSlot);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // IDENTICAL to GATHER_FRAG's P
  uint rbillboard; float rcov;
  float baseY = receiverAt(P, uData, uSurface, vec2(${RECV_ALIGN_XF}, ${RECV_ALIGN_YF}), rbillboard, rcov);
  bool allB = false;
  if (clamp(rcov, 0.0, 1.0) > 0.0) {
    float b0, b1, b2;
    vec2 al = vec2(${RECV_ALIGN_XF}, ${RECV_ALIGN_YF});
    allB = receiverCover(rbillboard, P + vec2(1.0, 0.0), P, uData, uSurface, al, b0) > 0.0
        && receiverCover(rbillboard, P + vec2(0.0, 1.0), P, uData, uSurface, al, b1) > 0.0
        && receiverCover(rbillboard, P + vec2(1.0, 1.0), P, uData, uSurface, al, b2) > 0.0;
  }
  oRecv = uvec4(floatBitsToUint(baseY), (rbillboard & 0xffffu) | (allB ? 0x10000u : 0u), floatBitsToUint(rcov), 0u);
}
`;
// FINE (lightmap res) — LIGHT_FRAG needs only the winning receiver ID (its normal fetch is cheap on
// a KNOWN billboard; the scan was the cost). R32UI: id(16) | presence(1).
const RECEIVER_FINE_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;
uniform sampler2D uSurface;
uniform vec2 uLightAlign;             // the LIGHTING seat (decoupled from the shadow's RECV_ALIGN)
layout(location = 0) out uvec4 oRecv;
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
const int FINE = ${FINE_RATIO};
void main() {
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0x3FFFu);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  int uSlotF = uSlot * FINE;
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlotF, sy = fc.y / uSlotF;
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols);
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlotF)) / float(uSlotF);
  float ly = (float(fc.y) - float(sy * uSlotF)) / float(uSlotF);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // IDENTICAL to LIGHT_FRAG's P
  uint rp; float rc;
  receiverAt(P, uData, uSurface, uLightAlign, rp, rc);
  // hot-sync P4 (squares of self-shadow on the mover): CONSERVATIVE classification for HOT
  // receivers. The min-corner test leaves fine texels the sprite partially covers classified
  // as GROUND, so the fine cut never fires there and the mover's own carved shadow shows as
  // dark squares ON its sprite. A texel counts as on-a-hot-billboard if ANY corner (or the
  // centre) touches the silhouette -- the billboard treatment then covers the sprite's whole
  // footprint, the ground shadow stays underneath it (drawn first, overwritten -- the tree
  // order). COLD receivers keep the tight single-sample test: their edge look is shipped.
  if (rp == 0u || !billboardHot(rp, uData)) {
    float ts = 16.0 / float(uSlotF);                 // this texel's span in UNITS (tile = 16 units)
    for (int k = 0; k < 4; k++) {
      vec2 o = k == 0 ? vec2(ts, 0.0) : (k == 1 ? vec2(0.0, ts) : (k == 2 ? vec2(ts, ts) : vec2(0.5 * ts, 0.5 * ts)));
      uint rp2; float rc2;
      receiverAt(P + o, uData, uSurface, uLightAlign, rp2, rc2);
      if (rp2 != 0u && billboardHot(rp2, uData)) { rp = rp2; rc = max(rc, rc2); break; }
    }
  }
  oRecv = uvec4((rp & 0xffffu) | (rc > 0.0 ? 0x10000u : 0u), 0u, 0u, 0u);
}
`;

const GATHER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;       // THE unified data texture (defs|billboards|lights|presence|buckets)
uniform highp usampler2D uRecvCoarse; // baked receiver map (P3) — baseY | id|allB | rcov, this shader's res
uniform sampler2D uSurface;           // the shared surface atlas page (F2) — silhouette coverage in B
uniform int uCorridor;                // P6: 1 = segment-DDA corridor walk, 0 = brute-force reach box
uniform int uLightClass;              // #4: process only this class of light — 0 = COLD (static), 1 = HOT (dynamic)
uniform float uElevK;                 // shadows-onto-billboards: receiver-elevation gain (sin65 default; 0 = flat, __elevk)
// PROFILING STAIRCASE for the GATHER (2026-07-27, moving-lights I10) — same device as LIGHT_FRAG's uProfile.
// 0 = full shader; 1..4 cut the fragment short at a named boundary so a GPU timer prices each substep by
// difference. Each level includes every level below it:
//   1 dirty gate + window mapping + P   2 + receiverAt   3 + the allBillboard corner test
//   4 + presence fetch and the light loop WITHOUT walkShadow   0 + walkShadow
uniform int uGProfile;
// Attachment 1 (oCasterD, the frontmost caster row) is DELETED (standing-costs P1): nothing ever read
// textures[1], and the dead MRT doubled the gather's write bandwidth + the RT memory of all four
// shadow RTs. walkShadow still reports cdepth; re-add the attachment when something consumes it.
layout(location = 0) out uvec4 fragColor; // per-light u9 shadow coverage
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
void main() {
  // P3/v2.1: window mapping from the CONSTANTS px (R = id|cols, G = rows|slot,
  // B = i16 winCol | i16 winRow — sign-extended halves). Updated through the command path.
  uvec4 C0 = fetchLin(uData, CONST_BASE);
  // G = u16 rows | u2 lod (14–15) | u14 slot (0–13) — mask the lod OFF the slot (textile-slot P2).
  int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0x3FFFu);
  int uLod = int((C0.y >> 14) & 3u);
  int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlot, sy = fc.y / uSlot;                 // toroidal slot
  // The per-texel dirty gate is RETIRED (standing-costs P2): the draw rasterizes ONLY dirty-rect
  // geometry, whose tile-aligned edges make coverage exactly the dirty set — clean texels are never
  // rasterized, so they persist by never being drawn rather than by discard.
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols); // → world tile
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlot)) / float(uSlot); // 0..1 within the tile
  float ly = (float(fc.y) - float(sy * uSlot)) / float(uSlot);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS (true texel position)
  // shadows-onto-billboards: is a standing billboard DRAWN at this texel, and its base row? IN-FAMILY (caster buckets
  // + surface atlas by index) — zoom-safe by construction, NOT the reverted zdepth-composite world read.
  if (uGProfile == 1) { fragColor = uvec4(0u); return; } // G1: prologue only
  // P3: the receiver scan is BAKED — one fetch replaces receiverAt (6-row bucket scan + record
  // decodes) and the 3-corner test. Values are the same functions' exact outputs at this P.
  uvec4 R0 = texelFetch(uRecvCoarse, fc, 0);
  uint rbillboard = R0.y & 0xffffu;
  float rcov = uintBitsToFloat(R0.z);
  float baseY = uintBitsToFloat(R0.x);
  if (uGProfile == 2) { fragColor = uvec4(uint(rcov)); return; } // G2: + receiver fetch
  // pawn-render P2: a HOT receiver (a mover) exists only for the HOT pass. The COLD pass treats
  // its texels as GROUND — the cold map bakes the terrain under the wolf (correct the moment it
  // walks away, and never re-baked by its motion); the blit picks the hot map on mover pixels.
  bool recvHot = rbillboard != 0u && billboardHot(rbillboard, uData);
  if (uLightClass == 0 && recvHot) { rbillboard = 0u; rcov = 0.0; R0.y = 0u; }
  bool isThing = rbillboard != 0u;
  int fold = foldTile(wc, wr);
  // D8 (texture-generalization P3): the texel's RECEIVE MODE — the drawn billboard's where
  // one is drawn (standing billboards author like-billboard today; their def lanes land with
  // the DSL size rework), else the TILE's authored mode from its slot-0 presence FLAGS.
  // 0 = receives nothing (zero walks — the flag early-out), 1 = like a billboard (the
  // climbing walk), 2 = like ground (the flat walk). The old model — ground receives
  // UNCONDITIONALLY — is this same fork with every tile hardwired to mode 2.
  uint rmode = isThing ? 1u : (fetchLin(uData, BILLBOARD_PRESENCE_BASE + fold).x >> 29) & 3u;
  if (rmode == 0u) { fragColor = uvec4(0u); return; }        // mode 0: nothing to receive
  float maskCov = clamp(rcov, 0.0, 1.0);                     // SOFT mask coverage → blends ground↔thing at the silhouette edge
  // Is this shadow texel ENTIRELY on the billboard? A texel = 1/16 tile = 1 UNIT (= 4px @ 64px/tile); test its 4
  // corners against rbillboard's silhouette (P is already on rbillboard since maskCov>0). If a corner falls OFF the billboard
  // the texel STRADDLES ground → we add the ground shadow below so its ground px darken; if all-on-billboard we cull
  // the ground (no ground visible there). Light-INDEPENDENT (geometry only) → computed ONCE, out of the loop.
  bool allBillboard = (R0.y & 0x10000u) != 0u; // P3: corner test baked with the receiver map
  // A mode-1 TILE (a wall): the tile ITSELF is the standing receiver — base at its south
  // edge, the shadow climbs its own height, and the whole texel is receiver (no silhouette,
  // no ground straddle).
  if (rmode == 1u && !isThing) {
    baseY = float(wr + 1) * 16.0;
    maskCov = 1.0;
    allBillboard = true;
  }
  // Fictional height of this receiver pixel above its OWN base (units); 0 for ground → the per-light ground
  // projection below makes the shadow CLIMB the receiver. Rbase = the receiver base (the front/behind axis).
  float zElev = rmode == 1u ? uElevK * max(0.0, baseY - P.y) : 0.0;
  vec2 Rbase = vec2(P.x, baseY);
  vec2 Pground = P; Pground.y += ${SHADOW_LIFTF}; // #2 lift — GROUND path only (thing path projects instead)

  if (uGProfile == 3) { fragColor = uvec4(allBillboard ? 1u : 0u); return; } // G3: + corner test

  uvec4 presLo = fetchLin(uData, PRESENCE_BASE + fold);     // lights 0–6
  // v5: seed every slot to SHADOW_NONE -- zero is a REAL billboardIdx, so an unwritten slot
  // must not read as 'caster 0 occludes here'.
  uint o0 = 0x7fff7fffu, o1 = 0x7fff7fffu, o2 = 0x7fff7fffu, o3 = 0x7fff7fffu;                   // per-SLOT u8 coverage: slot i at ch i>>2, bit (i&3)*8
  for (int slot = 0; slot < 8; slot++) {
    uint li = tileSlot(presLo, slot);   // F7: one presence word, 8 slots
    if (li == 0xffffu) continue;                            // empty slot
    uvec4 Ld = fetchLin(uData, LIGHT_BASE + int(li));       // v2.1: G = position, A = z|reach|emitter|hot|cast
    if (((Ld.y >> 24) & 1u) == 0u) continue;                // cast_shadows
    // pawn-render P2 (the tier matrix): the COLD pass handles cold lights only (as before); the
    // HOT pass handles hot lights everywhere PLUS cold lights on HOT-RECEIVER texels — a cold
    // torch's shadow climbing a wolf lands in the hot map, never re-baking cold.
    uint lcls = (Ld.y >> 25) & 1u;
    if (uLightClass == 0 && lcls != 0u) continue;      // cold pass: cold lights only (as ever)
    // hot pass: hot lights everywhere; cold lights FULLY on hot-receiver texels, and in DELTA
    // mode elsewhere (P3): walk HOT casters only — the coverage this writes is the mover's
    // ADDED occlusion of the cold light, which the light pass turns into a negative deposit.
    int casterMode = uLightClass == 0 ? 1 : ((lcls == 0u && !recvHot) ? 2 : 0);
    vec3 L = vec3(resolvedPos((Ld.w >> 4) & 255u, (Ld.x >> 8) & 255u, Ld.x & 255u, P),
                  float((Ld.y >> 16) & 255u));              // v3 resolved pos + z_offset
    float emitter = float((Ld.w >> 12) & 255u);             // emitter_radius (units) → penumbra width
    int reachT = int((Ld.w >> 20) & 0xfffu) / int(UPT) + 1; // reach (units) → tiles, +1 margin (brute box)
    // ONE shadow per texel + an ON-BILLBOARD flag. ON-BILLBOARD = the texel sits on a billboard (maskCov>0) → keep it, NOT
    // cut by the fine presence (a billboard texel isn't ground). We walk the ELEVATED thing point (the climbing
    // shadow, shT). Then — UNLESS the texel is entirely on the billboard — we ADD the GROUND shadow (shG): a straddle
    // texel has ground px inside it (a notch/edge) that the fine bake does NOT cut (it's on-billboard), so without
    // this they'd stay bright. Entirely-on-billboard texels cull the ground (no ground visible → no over-darken of
    // the interior). Ground texels store shG alone, which the LIGHT bake cuts by FINE presence (tight edge).
    // Value = u8 (0..255), 9th bit = on-billboard flag.
    float cd = 0.0, cov = 0.0; uint win = 0xffffu; bool onBillboard = maskCov > 0.0;
    if (uGProfile == 4) {                                   // G4: loop + record fetches, NO walkShadow
    } else if (onBillboard) {
      float ze = min(zElev, 0.9 * L.z);                     // keep the projection s bounded
      float sProj = L.z / (L.z - ze);
      vec2 Qt = (sProj > 0.0) ? L.xy + sProj * (P - L.xy) : P;
      cov = walkShadow(L, emitter, Qt, true, rbillboard, Rbase, casterMode, uCorridor, reachT, uData, uSurface, cd, win); // on-billboard (climbing) shT
      if (!allBillboard) {                                       // straddles ground → add ground shadow (darkens the ground px within)
        float cdG;
        uint winG; float shG = walkShadow(L, emitter, Pground, false, rbillboard, Rbase, casterMode, uCorridor, reachT, uData, uSurface, cdG, winG);
        if (shG > cov) win = winG;   // ground shadow wins the texel -> its caster is the one stored
        cov = max(cov, shG);                                // MAX not sum: identical where shT=0 (the bright px), no false over-dark where both overlap
        cd = max(cd, cdG);
      }
    } else {                                                // ground texel → GROUND shadow (fine bake cuts it by fine presence)
      cov = walkShadow(L, emitter, Pground, false, rbillboard, Rbase, casterMode, uCorridor, reachT, uData, uSurface, cd, win);
    }
    // v4 BINARY (2026-07-30) -- THE slot byte layout, and the only place it is defined:
    //
    //   bit 0      on-billboard flag  (unchanged)
    //   bit 1      OCCLUDED           (was a u7 coverage in bits 1-7)
    //   bits 2-7   RESERVED, written zero
    //
    // 16 slots x 8 bits = 128 bits exactly, so the shadow texel is still one uvec4. Coverage went from
    // 128 levels to 2 because the map only ever held 2: P0 measured 31 155 shadowed slot-samples with
    // ZERO partial values. The 6 reserved bits are the budget a later phase can spend -- on a packed
    // caster id, or on more than 16 slots per tile (I5) -- and they are written zero so that whatever
    // reads them next cannot inherit garbage.
    // v5: store the CASTER, not a value. SHADOW_NONE when nothing occluded -- which is also how the
    // reader learns "lit", so no separate coverage bit exists to fall out of sync with the id.
    uint sid = ((cov > 0.0 && win != 0xffffu) ? (win & 0x7fffu) : SHADOW_NONE)
             | (onBillboard ? 0x8000u : 0u);
    // MASKED assignment, not |=. The lanes are seeded to the sentinel (0x7fff7fff), and OR-ing a real id
    // into 0x7fff leaves 0x7fff for every id <= 0x7fff -- i.e. every slot would read back as "no caster"
    // and the world would render with no shadows at all. Clear this slot's half first.
    uint packed = sid << uint((slot & 1) == 0 ? 16 : 0);
    uint keep = (slot & 1) == 0 ? 0x0000ffffu : 0xffff0000u;   // preserve the OTHER slot in this lane
    int ch = slot >> 1;                                     // static branch (no dynamic write-subscript)
    if (ch == 0) o0 = (o0 & keep) | packed; else if (ch == 1) o1 = (o1 & keep) | packed;
    else if (ch == 2) o2 = (o2 & keep) | packed; else o3 = (o3 & keep) | packed;
  }
  fragColor = uvec4(o0, o1, o2, o3);
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
// v5 shadow texel decode — a LOCAL copy; this shader deliberately does not pull in GATHER_COMMON.
// Layout is defined once beside tileSlot there; keep the two in step.
const uint SHADOW_NONE = 0x7fffu;
float shadowOcc(uvec4 t, int i) {
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  uint v = ((i & 1) == 0 ? (w >> 16) : (w & 0xffffu)) & 0x7fffu;
  return v != SHADOW_NONE ? 1.0 : 0.0;
}
uint lane4(uvec4 v, int c) { return c == 0 ? v.x : (c == 1 ? v.y : (c == 2 ? v.z : v.w)); }
uint tileSlot(uvec4 t, int i) {   // v3: 8 slots/px — the self-address is gone (R=s0|s1 .. A=s6|s7)
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
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
  uvec4 shC = texelFetch(uShadow, texel, 0);                // cold class slots
  uvec4 shH = texelFetch(uShadowHot, texel, 0);             // hot class slots (a slot is nonzero in one only)
  vec3 acc = vec3(0.0);
  float any = 0.0;
  for (int slot = 0; slot < 8; slot++) {
    uint li = tileSlot(presLo, slot);   // F7: one presence word, 8 slots
    if (li == 0xffffu) continue;                            // empty slot
    float cvg = max(shadowOcc(shC, slot), shadowOcc(shH, slot));   // v5: occluded iff id != SHADOW_NONE
    if (cvg > 0.0) { acc += lightColour(int(li)) * cvg; any = max(any, cvg); }
  }
  if (any <= 0.0) { fragColor = vec4(0.0); return; }        // lit → transparent
  fragColor = vec4(clamp(acc, 0.0, 1.0), any);              // shadow tint × coverage
}
`;

// ── DECAY LIGHTMAP (lighting-feel P2) ─────────────────────────────────────────────────────────────
// A third, COARSE, EPHEMERAL lightmap: it decays toward zero every frame (one in-place mulConstant
// draw — F2) and light PARTICLES are splatted into it additively, fire-and-forget (F1/F4/F5). It is
// EXEMPT from the accumulators' exactness machinery BY DESIGN: it forgets, so nothing is ever
// subtracted or dirty-tracked. Anything persistent belongs in cold/hot instead — keep it that way.
/** The decay fade "shader" — output is irrelevant (`blendFunc(ZERO, CONSTANT_COLOR)` multiplies dst
 *  by the blend colour); the draw exists to touch every texel. */
const DECAY_FRAG = /* glsl */ `#version 300 es
precision highp float;
out vec4 oC;
void main() { oC = vec4(0.0); }
`;
const SPLAT_VERT = /* glsl */ `#version 300 es
in vec2 aUnit;                    // 0..1 quad
uniform vec2 uCenterUV;           // splat centre in decay-RT UV (CPU maps world → toroidal slot)
uniform vec2 uRadUV;              // radius in UV per axis
out vec2 vLocal;                  // -1..1 across the splat
void main() {
  vLocal = aUnit * 2.0 - 1.0;
  vec2 uv = uCenterUV + vLocal * uRadUV;
  gl_Position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
}
`;
// Additive radial glow, SHADOW-STAMPED at emit (F4): the decay RT shares the coarse shadow RT's
// exact texel geometry, so gl_FragCoord addresses both; the parent light's slot is found in the
// tile's presence and its u7 coverage multiplies the splat — particles inherit their light's
// shadows once, at write time, and flicker can never bleed into shadow.
const SPLAT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
in vec2 vLocal;
uniform vec3 uColor;
uniform float uIntensity;
uniform int uLightId;              // parent light id (presence slot search); < 0 = unstamped
uniform highp usampler2D uShadowStamp; // the parent's CLASS shadow RT (same texel geometry)
uniform highp usampler2D uData;    // presence (window mapping via the constants row)
out vec4 oGlow;
// Minimal helpers (the OVERLAY_FRAG pattern — do NOT pull in GATHER_COMMON for a splat).
const int PRESENCE_BASE = 196608, PRESENCE_HI_BASE = 327680, CONST_BASE = 1047552;
uvec4 fetchLin(highp usampler2D t, int i) { return texelFetch(t, ivec2(i & 1023, i >> 10), 0); }
int pmod(int a, int m) { return ((a % m) + m) % m; }
int fmod16(int v) { return ((v % 16) + 16) % 16; }
int foldTile(int wc, int wr) {
  int zx = fmod16(wc >> 4), zy = fmod16(wr >> 4), tx = fmod16(wc), ty = fmod16(wr);
  return ((zx >> 2) + zy * 4) * 1024 + (zx & 3) * 256 + ty * 16 + tx;
}
// v5 shadow texel decode — a LOCAL copy; this shader deliberately does not pull in GATHER_COMMON.
// Layout is defined once beside tileSlot there; keep the two in step.
const uint SHADOW_NONE = 0x7fffu;
float shadowOcc(uvec4 t, int i) {
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  uint v = ((i & 1) == 0 ? (w >> 16) : (w & 0xffffu)) & 0x7fffu;
  return v != SHADOW_NONE ? 1.0 : 0.0;
}
uint lane4(uvec4 v, int c) { return c == 0 ? v.x : (c == 1 ? v.y : (c == 2 ? v.z : v.w)); }
uint tileSlot(uvec4 t, int i) {
  int c = i >> 1;
  uint w = c == 0 ? t.x : (c == 1 ? t.y : (c == 2 ? t.z : t.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}
void main() {
  float r2 = dot(vLocal, vLocal);
  if (r2 >= 1.0) discard;
  float glow = (1.0 - r2) * (1.0 - r2);              // smooth radial falloff, C1 at the rim
  float stamp = 1.0;
  if (uLightId >= 0) {
    uvec4 C0 = fetchLin(uData, CONST_BASE);
    int uCols = int(C0.x & 0xFFFFu), uRows = int(C0.y >> 16), uSlot = int(C0.y & 0x3FFFu);
    int uWinCol = int(C0.z) >> 16, uWinRow = (int(C0.z) << 16) >> 16;
    ivec2 fc = ivec2(gl_FragCoord.xy);
    int sx = fc.x / uSlot, sy = fc.y / uSlot;
    int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols);
    int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
    int fold = foldTile(wc, wr);
    uvec4 presLo = fetchLin(uData, PRESENCE_BASE + fold);
    uvec4 sh = texelFetch(uShadowStamp, fc, 0);
    for (int slot = 0; slot < 8; slot++) {
      uint li = tileSlot(presLo, slot);   // F7: one presence word, 8 slots
      if (li != uint(uLightId)) continue;
      stamp = 1.0 - shadowOcc(sh, slot);            // (1 − the parent light's shadow coverage)
      break;
    }
  }
  oGlow = vec4(uColor * (uIntensity * glow * stamp), 0.0);
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

/** The lightmap DIFFERENTIAL (F11b.1) is designed but not wired — `uShadowPrev`/`uDataPrev` have no
 *  consumer. Until they do, the prev-shadow snapshot blit is pure overhead and is gated behind this. */
const DIFFERENTIAL_WIRED = false;

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
  /** DEBUG: orbit the lights every frame so the shadow recompute runs each frame — turns the FPS
   *  panel into a live gather-cost readout (the "hot lights" perf case). Toggle with {@link setOrbit}. */
  private orbit = false;            // orbit disabled — lights are static
  private readonly empty: Texture;
  private enabled = true;
  /** shadows-onto-billboards: receiver-elevation gain (sin65 by the world-geometry model). `__elevk` tunes the
   *  climb rate by eye; 0 collapses billboard shadows to the flat ground shadow. */
  private elevK = ELEV_K_DEFAULT;
  /** world-space-lighting (P1): TRUE-3D elliptical falloff (`__worldlight`) + the live-tunable N–S
   *  un-foreshorten factor (`__nsfactor`, default 1/cos65). Default ON so the corrected look shows; toggle
   *  off to A/B against the screen circle. */
  private worldLight = true;
  private nsInv = INV_COS_TILT_DEFAULT;
  /** shadow-polish P3: bilinear coarse-shadow upsample in the fine lightmap read (the
   *  user-requested filter; also the fine-offset consideration the shadow read lacked).
   *  `__shadowfilter(on?)` toggles for A/B against the old NEAREST look. */
  private shadowFilter = true;
  /** lightmap P0 debug (`__shownormal`): paint the atlas-frame billboard normal into the lightmap to verify the
   *  read (per-billboard, zoom-stable). Off in normal operation. */
  private showNormal = false;
  /** lightmap: the standing-billboard normal pitch (deg, `__pitchnormal`; default 90°−tilt, re-derived by
   *  `__tilt`). The corpus normals are RAW (camera-facing) and the world pitch is applied IN-SHADER — kept
   *  live rather than bake-committed (F7 reconsidered): it's ~free in the dirty-gated bake and keeps the
   *  world angle adjustable. Ground (rbillboard 0) is unpitched (flat-up); things pitch to perpendicular. */
  private normalPitchDeg = 90 - WORLD_TILT_DEG;
  /** lightmap: the LIGHTING receiver-mask seat (units, NW shift), DECOUPLED from the shadow's `RECV_ALIGN`.
   *  The shadow's (0.5, 1.5) offset clipped the lit sprite's SE edge (bottom + right) — the lighting coverage
   *  aligns to the RAW def anchor (the albedo draw), so this is 0. `__lightalign(x, y)` re-tunes by eye. */
  private lightAlignX = 0;
  private lightAlignY = 0;
  /** PROFILING staircase level for `GATHER_FRAG` (0 = full; 1–4 cut short). `__gprofile(n)`; leave at 0. */
  gatherProfile = 0;
  /** PROFILING staircase level INSIDE `walkShadow` (0 = full; 1–4 cut short). `__wprofile(n)`; leave at 0. */
  walkProfile = 0;
  /** PROFILING staircase level INSIDE `casterCover` (0 = full; 1–3 cut short). `__cprofile(n)`; leave at 0. */
  casterProfile = 0;
  /** Caster card lean — north offset as a fraction of `H·cos(tilt)`. **1.0 = the rigid parallel-to-view
   *  billboard** (card length exactly `H`); 0.5 was the shipped value, which is not a rigid rotation and
   *  makes the card ~0.87·H at 55°. `__lean(x)`. Feeds BOTH `uCardLean` and `buildCasters`' TILT — they
   *  must agree or casters are bucketed for a card they don't cast. See world-geometry F2. */
  cardLean = 1.0;
  /** texture-generalization P0 (D9): the tallest authored card in TILES — content-derived
   *  (max over thing sizes + tile heights; conifer 2 today). The walk's SOUTHERN dilation
   *  is `ceil(TILT · maxCardTiles)` rows: under occupancy registration (base line only),
   *  those are the only tiles whose occupants' cards can lean over a visited tile. Wired
   *  into the walks at P1; derived + probeable (`__gather.maxCardTiles` / `dilationRows()`)
   *  from P0 so the constant's plumbing is verified before consumers exist. */
  maxCardTiles = 2;
  setMaxCardTiles(tiles: number): void {
    this.maxCardTiles = Math.max(1, tiles);
  }
  /** texture-generalization P2 (D7): TILE KINDS enter presence. The bridge SHARES its live
   *  world-tile → kind map (by reference — no sync) plus the per-kind lane table (stride 6:
   *  linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows), stems, and the
   *  biome-tile object type. `tileSlotAt` turns those into each window tile's SLOT-0 word:
   *  `flags | set = definition_data | tileDefinitionFor(stem, lod)` — one def per (kind, lod),
   *  memoised per kind per pass. */
  private tileKindMap: Map<number, number> | null = null;
  private tileLanes: Float64Array | null = null;
  private tileStems: string[] = [];
  private tileTypeId = 0;
  /** DEBUG A/B (`__tileslots`): false = slot 0 stays empty (pre-P2 presence) — the acceptance
   *  oracle (tiles cast nothing + receivers filter on `set`, so shadows must be bit-identical). */
  tileSlotsOn = true;
  private readonly tileSlotWords = new Map<number, number>();
  private lastResolver: TextureResolver | null = null;
  private readonly tileLaneScratch: TileDefLanes = { linkedW: 0, linkedH: 0, pad: 0, rotation: 0, cast: 0, receives: 0, type: 0 };
  setTileKinds(kindAt: Map<number, number>, lanes: Float64Array, stems: string[], typeId: number): void {
    this.tileKindMap = kindAt;
    this.tileLanes = lanes;
    this.tileStems = stems;
    this.tileTypeId = typeId;
    this.rebakeAll(); // kinds/lanes changed (content hot-swap) — every slot-0 word may differ
  }
  /** The slot-0 word for world tile (wc, wr): the drawn kind's def + its authored flags, or 0. */
  private tileSlotAt(wc: number, wr: number, resolver: TextureResolver | null): number {
    if (!this.tileSlotsOn || !this.tileKindMap || !this.tileLanes) return 0;
    const kind = this.tileKindMap.get(((wc & 0xffff) << 16) | (wr & 0xffff));
    if (kind === undefined || kind === 0) return 0;
    const hit = this.tileSlotWords.get(kind);
    if (hit !== undefined) return hit;
    let word = 0;
    const stem = this.tileStems[kind - 1];
    const b = (kind - 1) * 6;
    if (stem && b + 6 <= this.tileLanes.length) {
      const s = this.tileLaneScratch;
      s.linkedW = this.tileLanes[b]; s.linkedH = this.tileLanes[b + 1]; s.pad = this.tileLanes[b + 2];
      s.rotation = this.tileLanes[b + 3]; s.cast = this.tileLanes[b + 4]; s.receives = this.tileLanes[b + 5];
      s.type = this.tileTypeId;
      const idx = this.coldData.tileDefinitionFor(kind, stem, s, resolver);
      word = (((s.cast ? SLOT_CAST : 0) | ((s.receives & 3) << SLOT_RECV_SHIFT)
        | (SET_DEFINITION_DATA << 24) | (idx & 0xffff)) >>> 0);
    }
    this.tileSlotWords.set(kind, word);
    return word;
  }
  /** The southern-dilation row count the D9 walks use — bounded by the LARGER of the
   *  content-authored tallest card and the tallest tight box any def actually minted
   *  (art can exceed the nominal card; a short dilation silently drops shadows). */
  dilationRows(): number {
    const tallestPx = Math.max(this.maxCardTiles * SQUARE, this.coldData.maxTightHpx);
    return Math.ceil(this.cardLean * Math.cos(this.worldTiltDeg * Math.PI / 180) * (tallestPx / SQUARE));
  }
  /** Falloff exponent — see {@link FALLOFF_EXP}. `__falloff(e)`; <1 lifts the mid-range, 1 = linear
   *  smoothstep, >1 darkens the outer pool. Cannot extend a light past its reach at any value. */
  falloff = FALLOFF_EXP;
  /** lighting-feel P1 — colour temperature over falloff. `__lighttemp(warm, sat)`; identity at (0, 1). */
  warmShift = 0.12;
  rimSat = 0.65;
  /** lighting-feel P1 — specular glints on things. `__glint(str, pow)`; off at str 0. */
  glintStr = 0.25;
  glintPow = 16;
  /** PROFILING staircase level for `LIGHT_FRAG` (0 = full shader; 1–4 cut it short at a named boundary).
   *  The lighting bake is ONE draw, so per-substep GPU timing is only obtainable by differencing these.
   *  `__profile(n)`; always leave it at 0. See `moving-lights` I9. */
  profile = 0;
  /** DEBUG (`__hideright`): blank the right half of each billboard's normal in `__shownormal` (alignment probe). */
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
  /** PREVIOUS-frame shadow, per class (F11b.1 / [I36]). The differential's OLD term must pair old light
   *  records with the OLD shadow — `shadow` lives in this RT, not in the data texture, and the gather
   *  regenerates it every frame. Without this, any frame where a caster moved would subtract a value that
   *  was never deposited and leave residue. ~3 MB each, against the data texture's 16 MB. */
  private coldShadowPrevRT: RenderTarget | null = null;
  private hotShadowPrevRT: RenderTarget | null = null;
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
   *  tile ≤7 `u16` billboard indices (`0` = empty); `castCount` the per-tile fill. `buildCasters` writes each
   *  tile via `coldData.writeBillboardPresence`. */
  /** texture-generalization P1 (D7): 4 FULL u32 occupant slots per window tile
   *  (`flags|set|index`), filled by BASE-LINE occupancy (D9). */
  private castSlots = new Uint32Array(0);
  private castCount = new Uint8Array(0);
  /** ORACLE toggle (`__fillmode`): true = the OLD extent registration (a coverage superset
   *  under the dilated walks) — bit-equal output vs base-line proves D9's coverage. */
  extentFill = false;

  /** #4 shadow_dirty — TWO per-class CPU mirrors (nonzero = recompute). Owner tracking is SHARED
   *  (pan exposes tiles for both). A COLD tile recomputes on a static-light or caster change or pan;
   *  a HOT tile on a dynamic-light move, caster change, or pan. The common frame (only one hot light
   *  moves) dirties HOT only → the cold shadow + lightmap are never re-baked. The GPU-side dirty
   *  TEXTURES are gone (standing-costs P2) — the mirrors now drive the dirty-rect draw geometry,
   *  which rasterizes exactly the dirty tiles, so no per-texel gate exists to feed. */
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
  /** Per-class dirty-tile counts from the last {@link buildDirty} — a class with 0 skips its ENTIRE
   *  {@link classPass} (blit + gather + lighting draw). Before this gate a fully static frame still
   *  submitted 2 blits + 2 gathers + 2 × 8.4 M-fragment fine draws whose every fragment discarded
   *  (standing-costs P1). */
  private coldDirtyCount = 0;
  private hotDirtyCount = 0;
  /** Cumulative dirty-slot bakes per class since load (pawn-render P0 — the
   *  cold-never-rebakes acceptance counter; read via `__bakes()`). [cold, hot]. */
  readonly bakeCounts: [number, number] = [0, 0];
  /** DEBUG (standing-costs P1 acceptance): class passes actually SUBMITTED last frame (0–2). */
  debugClassDraws = 0;
  /** P2 dirty-rect draw lists (standing-costs, [F1](../../../../docs/work/2026-07-27-lighting-standing-costs/forks.md#f1)):
   *  per class, the dirty mirror greedily row-merged into tile-aligned rects and emitted as raw NDC
   *  triangles (6 verts/rect, non-indexed). The gather + fine lighting draws rasterize ONLY these
   *  rects instead of a fullscreen quad discarding clean texels. NDC is exact: cols/rows are pow2,
   *  so every rect edge is a dyadic fraction landing on a texel boundary in both RT resolutions. */
  private coldRectPos = new Float32Array(0);
  private hotRectPos = new Float32Array(0);
  private coldRectVerts = 0;
  private hotRectVerts = 0;
  private dirtyGeo: Geometry | null = null;
  /** DEBUG (P2 acceptance): Σ rect areas in TILES per channel last build — must equal the dirty
   *  counts. Index 0 = cold, 1 = hot, 2 = receiver (P3). */
  debugRectTiles: [number, number, number] = [0, 0, 0];
  /** DEBUG (P2 acceptance): rects emitted per channel last build. */
  debugRectCount: [number, number, number] = [0, 0, 0];
  /** P3 receiver-dirty channel ([F2] in the stream's forks): fed ONLY by billboard/prim changes,
   *  owner change (pan/zoom), and force-all — NEVER by a light move. That separation is the whole
   *  win: light motion re-bakes lighting without re-deriving light-independent receiver geometry. */
  private receiverMirror = new Uint8Array(0);
  private receiverPending: [number, number, number, number][] = [];
  private forceReceiverDirty = true;
  private receiverRectPos = new Float32Array(0);
  private receiverRectVerts = 0;
  private receiverDirtyCount = 0;
  /** DEBUG (P3 acceptance): receiver-bake passes submitted last frame (0/1). */
  debugReceiverDraws = 0;
  private receiverCoarseRT: RenderTarget | null = null;
  private receiverFineRT: RenderTarget | null = null;
  /** lighting-feel P2 — the DECAY lightmap (coarse RGBA16F, F1) + its per-frame fade constant.
   *  Ephemeral by design: no dirty tracking, no exactness — it forgets. */
  private decayRT: RenderTarget | null = null;
  /** Decay time constant τ (seconds) — a splat falls to 1/e in τ. `__flicker(_, tau)`. Short τ is
   *  what makes flicker READ: equilibrium mean ≈ rate·E[intensity]·τ, and the visible fluctuation
   *  around it scales ~1/√(rate·τ) — long τ = steady bloom, short τ = breathing fire. */
  decayTau = 0.22;
  private lastDecayMs = 0;
  /** Master switch + strength for the v1 CPU flicker emitter (F5). `__flicker(on?, tau?, str?)`. */
  flickerOn = true;
  flickerStr = 0.05;
  /** DEBUG: emit from EVERY carried light regardless of its content flag (A/B aid). */
  flickerAll = false;
  /** DEBUG (P2 acceptance): splats emitted last frame. */
  debugSplats = 0;
  /** P6: walk the segment corridor (true) or the brute-force reach box (false). Brute is the
   *  validation baseline — `__corridor(false)` + `__shadowDiff()` must report 0 mismatches. */
  private corridor = true;
  /** P5 scoped dirty: world-tile rects `[x0,y0,x1,y1,cls]` (inclusive) queued by a light MOVE or caster
   *  change — the union of old + new reach. `cls`: 0 = cold only, 1 = hot only, 2 = both (a caster change
   *  affects every light that reaches it). Applied (∩ window) per class on top of the owner pass in
   *  {@link buildDirty}, then cleared. A moved light only changes shadow inside its old ∪ new reach. */
  private pendingRects: [number, number, number, number, number][] = [];

  private readonly recvCoarse: Program;
  private readonly recvFine: Program;
  private readonly decayProg: Program;
  private readonly splatProg: Program;
  private splatQuad: Geometry | null = null;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.gather = new Program(gl, FULLSCREEN_VERT, GATHER_FRAG, "shadow-gather");
    this.lighting = new Program(gl, FULLSCREEN_VERT, LIGHT_FRAG, "world-lighting");
    this.recvCoarse = new Program(gl, FULLSCREEN_VERT, RECEIVER_COARSE_FRAG, "receiver-coarse");
    this.recvFine = new Program(gl, FULLSCREEN_VERT, RECEIVER_FINE_FRAG, "receiver-fine");
    this.decayProg = new Program(gl, FULLSCREEN_VERT, DECAY_FRAG, "decay-fade");
    this.splatProg = new Program(gl, SPLAT_VERT, SPLAT_FRAG, "decay-splat");
    this.splatQuad = new Geometry(gl, this.splatProg, {
      aUnit: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.overlay = new Program(gl, OVERLAY_VERT, OVERLAY_FRAG, "shadow-overlay");
    this.gizmo = new Program(gl, GIZMO_VERT, GIZMO_FRAG, "shadow-gizmo");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    this.coldData = new ColdShadowData(renderer);
    (globalThis as unknown as { __cold: unknown }).__cold = this.coldData; // DEBUG
    // DEBUG: console toggle for the orbit (perf measurement) — `__orbit(false)` to freeze the lights.
    (globalThis as unknown as { __orbit: (on?: boolean) => boolean }).__orbit = (on?: boolean) => this.setOrbit(on);
    // DEBUG (P6): corridor↔brute toggle + the gather itself (for `debugReadShadow` diffing).
    (globalThis as unknown as { __corridor: (on?: boolean) => boolean }).__corridor = (on?: boolean) => this.setCorridor(on);
    // ORACLE (texture-generalization P1): toggle extent-vs-base-line registration + full rebake.
    (globalThis as unknown as { __fillmode: (extent?: boolean) => boolean }).__fillmode = (extent?: boolean) => {
      this.extentFill = extent ?? !this.extentFill;
      this.rebakeAll();
      return this.extentFill;
    };
    (globalThis as unknown as { __gather: ShadowGather }).__gather = this;
    // shadow-polish P3: bilinear coarse-shadow upsample A/B (no rebake needed — a read-side change).
    (globalThis as unknown as { __shadowfilter: (on?: boolean) => boolean }).__shadowfilter = (on?: boolean) => {
      this.shadowFilter = on ?? !this.shadowFilter;
      this.rebakeAll(); // the fine lightmap must redraw under the new read
      return this.shadowFilter;
    };
    // DEBUG (pawn-render P0): cumulative [cold, hot] dirty-slot bakes — the
    // cold-never-rebakes-while-the-wolf-moves acceptance counter.
    (globalThis as unknown as { __bakes: () => [number, number] }).__bakes = () => [...this.bakeCounts] as [number, number];
    // DEBUG (shadows-onto-billboards): tune the receiver-elevation gain live — bigger = the billboard shadow climbs
    // faster/higher up a billboard; 0 = flat ground shadow (the A/B baseline).
    (globalThis as unknown as { __elevk: (k?: number) => number }).__elevk = (k?: number) => this.setElevK(k);
    // DEBUG (world-space-lighting): toggle true-3D elliptical falloff vs the screen circle; tune the N–S
    // un-foreshorten factor (default 1/cos65 ≈ 2.366) by eye — the sim-check on cos65 vs sin65.
    (globalThis as unknown as { __worldlight: (on?: boolean) => boolean }).__worldlight = (on?: boolean) => {
      this.worldLight = on ?? !this.worldLight;
      this.rebakeAll();
      return this.worldLight;
    };
    (globalThis as unknown as { __nsfactor: (f?: number) => number }).__nsfactor = (f?: number) => {
      if (f !== undefined) { this.nsInv = f; this.rebakeAll(); }
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
        this.rebakeAll();
      }
      return this.worldTiltDeg;
    };
    // DEBUG A/B (texture-generalization P2): tile slot-0 presence on/off — off restores the
    // pre-P2 presence exactly (the shadow-identity acceptance oracle).
    (globalThis as unknown as { __tileslots: (on?: boolean) => boolean }).__tileslots = (on?: boolean) => {
      this.tileSlotsOn = on ?? !this.tileSlotsOn;
      this.rebakeAll();
      return this.tileSlotsOn;
    };
    // DEBUG probe (P3 acceptance): the D1 autotile cell over the MIRROR's slot-0 defs — must
    // equal the drawn prim's cell (one formula, two consumers).
    (globalThis as unknown as { __autocell: (wc: number, wr: number) => number }).__autocell =
      (wc: number, wr: number) => this.coldData.tileAutoCellMirror(wc, wr);
    // DEBUG probe (P2 acceptance): a tile KIND's minted def words + slot word at a world tile.
    (globalThis as unknown as { __tiledef: (wc: number, wr: number) => unknown }).__tiledef = (wc: number, wr: number) => {
      const word = this.tileSlotAt(wc, wr, this.lastResolver);
      if (word === 0) return { word: 0 };
      const idx = word & 0xffff;
      const m = this.coldData.debugDefWords(idx);
      return { word: word.toString(16), idx, cast: word >>> 31, receives: (word >>> 29) & 3, set: (word >>> 24) & 15, def: m };
    };
    // DEBUG (lightmap P0): paint the atlas-frame billboard normal into the lightmap — verify the read is per-billboard
    // and rock-stable across zoom (frame-indexed, not a world-coord composite read).
    (globalThis as unknown as { __shownormal: (on?: boolean) => boolean }).__shownormal = (on?: boolean) => {
      this.showNormal = on ?? !this.showNormal;
      this.rebakeAll();
      return this.showNormal;
    };
    // DEBUG (lightmap P1): the DEV in-shader normal pitch (deg) — eyeball the sign/magnitude while the corpus
    // is still raw; default 90−tilt. Bake-at-ingest (F7) retires this.
    (globalThis as unknown as { __pitchnormal: (deg?: number) => number }).__pitchnormal = (deg?: number) => {
      if (deg !== undefined) { this.normalPitchDeg = deg; this.rebakeAll(); }
      return this.normalPitchDeg;
    };
    // DEBUG (lightmap): the LIGHTING receiver-mask seat (units NW), decoupled from the shadow's RECV_ALIGN —
    // dial by eye to stop the lit sprite clipping its bottom/right edge. Returns [x, y].
    (globalThis as unknown as { __lightalign: (x?: number, y?: number) => number[] }).__lightalign = (x?: number, y?: number) => {
      if (x !== undefined) this.lightAlignX = x;
      if (y !== undefined) this.lightAlignY = y;
      this.rebakeAll();
      return [this.lightAlignX, this.lightAlignY];
    };
    // DEBUG (lighting-feel P2): the flicker emitter — on/off, decay τ (s), strength; `__flickerall`
    // force-emits from every carried light regardless of its content flag (A/B aid).
    (globalThis as unknown as { __flicker: (on?: boolean, tau?: number, str?: number) => unknown }).__flicker =
      (on?: boolean, tau?: number, str?: number) => {
        if (on !== undefined) this.flickerOn = on;
        if (tau !== undefined) this.decayTau = tau;
        if (str !== undefined) this.flickerStr = str;
        return { on: this.flickerOn, tau: this.decayTau, str: this.flickerStr };
      };
    (globalThis as unknown as { __flickerall: (on?: boolean) => boolean }).__flickerall = (on?: boolean) => {
      this.flickerAll = on ?? !this.flickerAll;
      return this.flickerAll;
    };
    // DEBUG (lighting-feel P1): colour temperature — warm shift at the core + rim saturation.
    // Identity (A/B off) = __lighttemp(0, 1). Bake-affecting → rebake.
    (globalThis as unknown as { __lighttemp: (warm?: number, sat?: number) => number[] }).__lighttemp =
      (warm?: number, sat?: number) => {
        if (warm !== undefined) this.warmShift = warm;
        if (sat !== undefined) this.rimSat = sat;
        this.rebakeAll();
        return [this.warmShift, this.rimSat];
      };
    // DEBUG (lighting-feel P1): specular glints on things. Off (A/B) = __glint(0). Bake-affecting → rebake.
    (globalThis as unknown as { __glint: (str?: number, pow?: number) => number[] }).__glint =
      (str?: number, pow?: number) => {
        if (str !== undefined) this.glintStr = str;
        if (pow !== undefined) this.glintPow = pow;
        this.rebakeAll();
        return [this.glintStr, this.glintPow];
      };
    // DEBUG (moving-lights I9): the LIGHT_FRAG profiling staircase — cut the fragment short at a named
    // boundary so a GPU timer can price each substep by difference. Always leave it at 0.
    (globalThis as unknown as { __profile: (n?: number) => number }).__profile = (n?: number) => {
      if (n !== undefined) this.profile = n;
      this.rebakeAll();
      return this.profile;
    };
    // DEBUG (moving-lights I10): the same staircase for the GATHER pass.
    (globalThis as unknown as { __gprofile: (n?: number) => number }).__gprofile = (n?: number) => {
      if (n !== undefined) this.gatherProfile = n;
      this.rebakeAll();
      return this.gatherProfile;
    };
    // DEBUG (moving-lights I11): the staircase INSIDE walkShadow — setup / traversal / fetch / unpack / test.
    (globalThis as unknown as { __wprofile: (n?: number) => number }).__wprofile = (n?: number) => {
      if (n !== undefined) this.walkProfile = n;
      this.rebakeAll();
      return this.walkProfile;
    };
    // DEBUG (moving-lights I15): the staircase inside casterCover — record fetches / quad gate / 16-tap loop.
    (globalThis as unknown as { __cprofile: (n?: number) => number }).__cprofile = (n?: number) => {
      if (n !== undefined) this.casterProfile = n;
      this.rebakeAll();
      return this.casterProfile;
    };
    // DEBUG: caster card lean (world-geometry F2). 1.0 = rigid parallel-to-view, 0.5 = the old shipped
    // value. Sets the shader uniform AND the CPU bucketing factor together — they must never diverge.
    (globalThis as unknown as { __lean: (x?: number) => number }).__lean = (x?: number) => {
      if (x !== undefined) { this.cardLean = x; this.rebakeAll(); }
      return this.cardLean;
    };
    // DEBUG: falloff exponent, live. <1 lifts the mid-range so a big-reach pool stays legible far out.
    (globalThis as unknown as { __falloff: (e?: number) => number }).__falloff = (e?: number) => {
      if (e !== undefined) { this.falloff = e; this.rebakeAll(); }
      return this.falloff;
    };
    // DEBUG (__hideright): blank the right half of each billboard's normal in __shownormal (alignment probe).
    (globalThis as unknown as { __hideright: (on?: boolean) => boolean }).__hideright = (on?: boolean) => {
      this.hideRight = on ?? !this.hideRight;
      this.rebakeAll();
      return this.hideRight;
    };
    // DEBUG (lightmap P3): scatter n STATIC lights in a grid around the seed to stress the dirty-gated bake —
    // static lights bake ONCE then never re-bake, so fps should hold (the many-lights caching payoff). Returns
    // the new total light count (capped at the N_LIGHTS bitfield ceiling).
    // P5 DEBUG: turn an already-placed billboard into a TORCH — the same object now presents as both
    // a sprite and a light, carried by one prim. This is the two-presentation case running through the
    // real placement path, and the stand-in for content until the DSL supplies a torch kind.
    // supplies a torch kind. `__torch()` with no id lights the first standing billboard it finds.
    (globalThis as unknown as { __torch: (id?: number) => unknown }).__torch = (id?: number) => {
      const standing = this.lastStanding;
      const p = id !== undefined ? standing.find((q) => q.id === id) : standing[0];
      if (!p) return { error: "no standing billboard", candidates: standing.length };
      p.light = { color: [1, 0.72, 0.35], intensity: 1, reach: 8 * SQUARE, emitterRadius: 16,
                  height: 24 * UNIT, castShadows: true, hot: false };
      // DELIBERATELY no `rebakeAll()`. The scoped dirty path must carry a new light on its own —
      // if this needs a force-all to show up, the front-door wiring is wrong, not the test.
      return { lit: p.id, tile: [Math.floor(p.x / SQUARE), Math.floor(p.y / SQUARE)] };
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
    // Nearest-N per in-window tile (dense window-local scratch; the GPU slot is the region-torus fold).
    // Every light in the world is CARRIED by a placed primitive (P5 — the debug array is gone), keyed
    // by its own `light_data` id.
    for (const [id, L] of this.coldData.carriedLights) {
      const k = id, r = L.reach;
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

  /** Rebuild the per-tile **caster buckets** (P1): allocate each standing caster's def+billboard, then
   *  drop its `billboard_data` index into every tile of its **base line** (anchor row × width cols) that
   *  the window covers, up to 8/tile. The corridor sweep reads these; casters are bucketed by where
   *  they *stand* (ground base), not the billboard's aerial bbox. Cheap enough to rebuild each frame
   *  for now; the O(1) re-bucket on move lands with the hot tier. */
  private buildCasters(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    if (this.castSlots.length !== cols * rows * PRIM_SLOTS) { // (was BILLBOARD_SLOTS — realloc'd every frame)
      this.castSlots = new Uint32Array(cols * rows * PRIM_SLOTS);
      this.castCount = new Uint8Array(cols * rows);
    }
    this.castSlots.fill(0); // 0 = empty (billboard sentinel)
    this.castCount.fill(0);
    this.lastResolver = resolver; // P2: the `__tiledef` probe mints with the live resolver
    const count = this.castCount;
    // The card is TILTED back 65° (see the shader), so its ground footprint spans from the base
    // (anchor y) UP to the top (anchor y − 0.5·H·cos65). The corridor crosses the caster anywhere in
    // that y-range (far shadow ↔ top, near ↔ base), so we must bucket every row it spans — bucketing
    // only the base row missed casters whose top sits in the row above (I-7).
    const TILT = this.cardLean * Math.cos(this.worldTiltDeg * Math.PI / 180); // lean·cos(tilt) of the height (must match uCardLean)
    // litSeen is NOT cleared here (hot-sync P1): `moverDirty` runs BEFORE tick each frame and
    // shares the same per-frame light-cascade dedup — the clear lives at the END of tick, so
    // the set spans exactly one frame across both entry points.
    this.carriedSeen.clear();
    this.lastStanding = standing;
    this.stepOrbit(standing);
    const seen = this.billboardSeen;
    seen.clear();
    for (const p of standing) {
      // ONE PRESENTATION PER BLOCK, each behind its OWN guard (P7/H4, user). This loop used to be
      // billboard code with everything else downstream of `if (def < 0) continue` — so a guard that
      // only means "not a caster" silently decided whether a LIGHT was processed too. Nothing here may
      // `continue`: a primitive presents as any combination, and one absent presentation must never
      // skip another.
      // ── LIGHT presentation ────────────────────────────────────────────────────────────────────
      if (p.light) {
        this.carriedSeen.add(p.id);            // resident this frame — the rest get released below
        const cl = this.coldData.carriedLightFor(p, p.light);
        // The front door: scoped cast region + record/presence invalidation, exactly as a debug light
        // gets. Routing here (rather than a private version counter) is why presence picks the torch
        // up at all — and why it does so WITHOUT a force-all.
        if (cl.changed) {
          const w = this.coldData.carriedLights.get(cl.id)!;
          // `cl.from` = where the light was, so the queued rect is the union old ∪ new. Omitting it
          // dirties only the NEW reach box and leaves a stale smear behind every mover (I1) — the
          // `from` parameter has existed for this since P5 and no call site ever passed it, because
          // until I3 was fixed nothing could move.
          this.markLightDirty({ x: w.x, y: w.y, reach: w.reach, dynamic: p.light.hot }, cl.from);
        }
      }
      // ── BILLBOARD presentation ────────────────────────────────────────────────────────────────
      const def = this.coldData.definitionFor(p, resolver);
      if (def < 0) continue;  // not a caster — and this is now the LAST thing in the loop body, so
                              // skipping it cannot skip another presentation. Add new presentations
                              // ABOVE this line, never below it.
      const inst = this.coldData.billboardDataFor(p, def, resolver);
      seen.add(p.id); // resident this frame — everything else gets freed (P2)
      // Bucket by the TIGHT opaque bbox (P3), not the full billboard box — matches the quad we actually cast.
      // A flipped (W-facing) billboard mirrors the box within the billboard rect, same as the gather mirrors `off.x`.
      const t = this.coldData.tightBoxOf(def) ?? { dx: 0, dy: 0, w: p.width, h: p.height };
      const tdx = p.flipX ? p.width - (t.dx + t.w) : t.dx;
      const tx = p.x + tdx, ty = p.y + t.dy;
      // A changed billboard (new immutable def — lod landed/zoom — or first sight) cascades its region.
      // pawn-render P2: HOT prims (movers) dirty class 1 only, and a MOVED prim's OLD box is
      // cascaded too (old ∪ new — else the wolf leaves a stale silhouette behind each step).
      const pcls = p.hot ? 1 : 2;
      const lb = this.lastBox.get(p.id);
      // hot-sync P1: a HOT prim's change reaching HERE means the unified moverDirty missed it
      // (should be first-sight/def-swap only, never a plain move — the P2 drill asserts 0
      // during walks via this counter).
      if (inst.changed && p.hot) this.debugBackstopMoves++;
      if (inst.changed) {
        if (lb !== undefined && (lb[0] !== tx || lb[1] !== ty)) {
          this.markBillboardDirty(lb[0], lb[1], lb[2], lb[3], pcls);
        }
        this.markBillboardDirty(tx, ty, t.w, t.h, pcls);
      }
      // P4: remembered so REMOVAL can dirty scopedly. MUTATE in place — allocating a fresh array per
      // prim per frame is ~1700 short-lived arrays a frame, i.e. GC pressure for no reason.
      if (lb === undefined) this.lastBox.set(p.id, [tx, ty, t.w, t.h, pcls]);
      else { lb[0] = tx; lb[1] = ty; lb[2] = t.w; lb[3] = t.h; lb[4] = pcls; }
      // D9 (texture-generalization P1): BASE-LINE occupancy — a caster registers on its anchor
      // row × every base COLUMN (width stays fill-time: a horizontal ray's walk dilation is
      // vertical and can never look a column sideways). The old TILT rows (the I-7 extent) are
      // NOT registered — the walks find tall southern casters via uDilateS instead.
      // ns-shadows: a rot-0/2 caster's VERTICAL card stands on its n-s ground trace — that IS
      // its base line (rows = the side frame's width centered on the anchor; single column).
      // ORACLE (`__fillmode`): `extentFill` re-enables the OLD extent registration (tilt rows +
      // the ns col pad) with the NEW slot encoding. Because the walk max-accumulates and the
      // southern dilation makes extent registration a coverage SUPERSET, bit-equal output
      // between the two modes ON ONE SETTLED PAGE proves base-line + dilation reaches every
      // caster the extent did — the reload-free bit-identity oracle (reloads are non-
      // deterministic: lod retention settles defs by arrival order).
      const side = (p.rotation === 0 || p.rotation === 2) ? this.coldData.casterDefOf(inst.idx) : null;
      let r0: number, r1: number, c0: number, c1: number;
      if (side) {
        const st = this.coldData.tightBoxOf(side.def);
        const ws = st ? st.w : p.width;
        const cx = p.x + p.width * 0.5, ay = p.y + p.height;
        // shadow-polish P1: the ground trace spans [ay − ws, ay] (anchor = the south tip),
        // matching casterCoverNS's recentered card — NOT centered on the anchor.
        r0 = Math.floor((ay - ws) / SQUARE); r1 = Math.floor(ay / SQUARE);
        const cc = Math.floor(cx / SQUARE);
        if (this.extentFill) { c0 = cc - 1; c1 = cc + 1; } else { c0 = c1 = cc; }
      } else {
        const baseY = ty + t.h; // the base line — the SAME row the old extent's r1 used
        r1 = Math.floor(baseY / SQUARE); // (keep the old row convention exactly)
        r0 = this.extentFill ? Math.floor((baseY - TILT * t.h) / SQUARE) : r1;
        c0 = Math.floor(tx / SQUARE); c1 = Math.floor((tx + t.w) / SQUARE);
      }
      // P1: every standing billboard casts + receives like a billboard (the def-authored
      // lanes flow into these flags at P2).
      const slotWord = (SLOT_CAST | (1 << SLOT_RECV_SHIFT) | (SET_BILLBOARD_DATA << 24) | (inst.idx & 0xffff)) >>> 0;
      for (let wr = r0; wr <= r1; wr++) {
        if (wr < winRow || wr >= winRow + rows) continue;
        for (let wc = c0; wc <= c1; wc++) {
          if (wc < winCol || wc >= winCol + cols) continue;
          const ti = (wr - winRow) * cols + (wc - winCol); // dense window-local tile
          const n = count[ti];
          if (n >= PRIM_SLOTS - 1) continue; // slots 1..3 — slot 0 is the TILE's (P2); full → drop (rare)
          this.castSlots[ti * PRIM_SLOTS + 1 + n] = slotWord;
          count[ti] = n + 1;
        }
      }
    }
    // P4: billboards that left `standing` (zone evicted / destroyed) free their slots — and each one
    // queues its LAST KNOWN extent before the record goes, which is what retires the caster-removal
    // force-all. A removed caster is no longer in `standing`, so its box is unrecoverable after the
    // fact; that is precisely why removal used to fall back to recomputing every tile.
    for (const [pid, box] of this.lastBox) {
      if (seen.has(pid)) continue;
      this.markPrimDirty(box[0], box[1], box[2], box[3], box[4] ?? 2);
      this.lastBox.delete(pid);
    }
    // P7/H1: release carried lights whose owner stopped presenting one — dirtying what each lit
    // BEFORE its record goes, so nothing stays baked in with no owner to cascade from.
    this.coldData.freeCarriedLightsExcept(this.carriedSeen, (b) => this.markLightDirty(b));
    this.coldData.freeBillboardsExcept(seen);
    // Write EVERY in-window tile (compare-write diffs). The region-torus fold is the GPU slot.
    // P2: slot 0 carries the TILE's self-describing def (set = definition_data) — minted once
    // per KIND per pass (the per-kind memo), zero allocation per tile.
    // P4 (I2): a slot-0 CHANGE (a kind's def upgraded when its atlas lod landed, or the tile's
    // kind changed) must dirty the BAKED lighting too — without this the tile keeps its flat
    // lod-0 shading until a window move happens to re-bake it. Changed tiles coalesce into ONE
    // union rect per pass (a def upgrade sweeps a whole kind; pan-time false positives land on
    // tiles the pan already dirtied).
    this.tileSlotWords.clear();
    let chX0 = 0, chY0 = 0, chX1 = -1, chY1 = -1;
    for (let wr = winRow; wr < winRow + rows; wr++)
      for (let wc = winCol; wc < winCol + cols; wc++) {
        const ti = (wr - winRow) * cols + (wc - winCol);
        const word = this.tileSlotAt(wc, wr, resolver);
        this.castSlots[ti * PRIM_SLOTS] = word;
        if (this.coldData.primSlot0(wc, wr) !== word) {
          if (chX1 < chX0) { chX0 = chX1 = wc; chY0 = chY1 = wr; }
          else {
            if (wc < chX0) chX0 = wc; if (wc > chX1) chX1 = wc;
            if (wr < chY0) chY0 = wr; if (wr > chY1) chY1 = wr;
          }
        }
        this.coldData.writePrimPresence(wc, wr, this.castSlots.subarray(ti * PRIM_SLOTS, ti * PRIM_SLOTS + PRIM_SLOTS));
      }
    if (chX1 >= chX0) this.pendingRects.push([chX0 - 1, chY0 - 1, chX1 + 1, chY1 + 1, 2]);
    // The flush lives in `tick`, AFTER `buildPresence` — presence now runs last and its writes must
    // land in the SAME scatter batch, or the GPU reads a presence map one frame behind the bake.
  }

  /** P4 — **the one legitimate force-all.** Placement is always scoped (a prim/light/removal queues
   *  its own rects), so this is reserved for changes with no region to scope from: a GLOBAL constant
   *  moved (`__tilt`, `__pitchnormal`, `__worldlight`, …) and every baked texel is now wrong. Keeping
   *  it named and separate is the point — it stops "recompute everything" being reached for as a
   *  shrug whenever the scoped path is inconvenient ([issues.md#i4]). */
  rebakeAll(): void {
    this.forceColdDirty = this.forceHotDirty = true;
    this.forceReceiverDirty = true; // P3: global constants (__tilt, __lightalign, …) shift receiver values too
    this.coldDirty = true;   // records may have changed with it
    this.lightsVer++;        // and so may per-tile presence
  }

  /** texture-generalization P3 (D1): a tile KIND changed — the 3×3 ring's autotile cells and
   *  receive modes may all have changed with it; dirty the ring for BOTH classes so the
   *  neighbors self-heal on the next bake (the slot-0 words themselves ride the compare-write). */
  tileKindDirty(tileX: number, tileY: number): void {
    this.pendingRects.push([tileX - 1, tileY - 1, tileX + 1, tileY + 1, 2]);
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

  /** The BILLBOARD DIRTY CASCADE applied to texture/def changes: a billboard changed (new immutable def —
   *  lod landed/zoomed — or first sight) → dirty the billboard's own tiles, then every light whose reach
   *  touches them, then those lights' full cast regions (their reach boxes — a shadow texel is only
   *  written inside its light's presence, so the reach box bounds the cast). `x/y/w/h` = the billboard's
   *  tight box in world px. `litSeen` dedupes light boxes within a frame (streaming floods). */
  private readonly litSeen = new Set<number>();
  /** Billboard ids resident this frame (P2) — anything allocated but absent gets freed via the free-list. */
  private readonly billboardSeen = new Set<number>();
  /** P7/H1: billboard ids presenting a LIGHT this frame; anything else gets its light released. */
  private readonly carriedSeen = new Set<number>();
  /** P4: each resident billboard's last known tight box, so REMOVAL can queue a scoped rect. Without
   *  it a departed caster's box is unrecoverable (it has left `standing`) — which is exactly why
   *  removal used to force-all every tile. */
  private readonly lastBox = new Map<number, [number, number, number, number, number]>();
  /** P5 debug: the last `standing` list, so `__torch()` can pick a real placed billboard. */
  private lastStanding: Primitive[] = [];
  /** DEBUG: shadow tiles marked dirty (recomputed) on the last frame. */
  debugDirtyTiles = 0;
  /** P4 — **the PRIM dirty front door.** A carrier changed (placed / moved / re-carried / freed) →
   *  dirty its own tiles, then every light whose reach touches them, then those lights' cast regions.
   *  `x/y/w/h` is the extent of the prim **and everything it carries**: moving a carrier moves its whole
   *  subtree, so a carrier's box must cover its children or their old tiles keep a stale shadow.
   *  Billboard and light changes both funnel here ([F6](forks.md#f6)). */
  private markPrimDirty(x: number, y: number, w: number, h: number, cls = 2): void {
    const x0 = Math.floor(x / SQUARE) - 1, y0 = Math.floor(y / SQUARE) - 1;
    const x1 = Math.floor((x + w) / SQUARE) + 1, y1 = Math.floor((y + h) / SQUARE) + 1;
    // pawn-render P2: a HOT prim's change dirties the HOT class ONLY (cls 1) — a wandering
    // wolf must never re-bake a cold map. Cold prims keep cls 2 (both classes at their tiles).
    this.pendingRects.push([x0, y0, x1, y1, cls]);
    this.receiverPending.push([x0, y0, x1, y1]); // P3: a billboard change re-bakes ITS receiver tiles
    for (const [id, L] of this.coldData.carriedLights) {
      if (this.litSeen.has(id)) continue;
      const cx = Math.min(Math.max(L.x, x), x + w), cy = Math.min(Math.max(L.y, y), y + h);
      if (Math.hypot(cx - L.x, cy - L.y) > L.reach + SQUARE) continue; // light can't see the prim
      this.litSeen.add(id);
      // A hot prim under ANY light routes the interaction to the HOT map — the cascaded
      // light rect follows the PRIM's class, not the light's.
      this.markLightDirty({ x: L.x, y: L.y, reach: L.reach, dynamic: false }, undefined,
        cls === 1 ? 1 : undefined);
    }
  }

  /** P4 — the BILLBOARD presentation changed (new def / lod / first sight). Its carrier's extent is
   *  what actually needs redoing, so this is the prim cascade under a name that says what moved. */
  private markBillboardDirty(x: number, y: number, w: number, h: number, cls = 2): void {
    this.markPrimDirty(x, y, w, h, cls);
  }

  /** P4 — **the LIGHT dirty front door.** Placement, movement, a prop change and removal all route
   *  here; pass `from` when the light moved so the union of old ∪ new reach is queued. The cold/hot
   *  **class is derived from the light itself** — callers no longer thread a `cls` argument, which is
   *  what let the routing drift out of step with `L.dynamic` in three separate places. */
  private markLightDirty(L: { x: number; y: number; reach: number; dynamic: boolean },
                         from?: { x: number; y: number }, clsOverride?: number): void {
    this.markLightMove(from?.x ?? L.x, from?.y ?? L.y, L.x, L.y, L.reach, clsOverride ?? (L.dynamic ? 1 : 0));
    // The bookkeeping a light change implies, DERIVED here rather than hand-set at each call site —
    // its record may differ (`coldDirty`) and the per-tile light lists may differ (`lightsVer`).
    // Those two used to be set by hand wherever someone remembered to; forgetting either is a silent
    // stale-bake, which is why they now hang off the one door every light change passes through.
    this.coldDirty = true;
    this.lightsVer++;
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
      this.coldMirror = new Uint8Array(cols * rows);
      this.hotMirror = new Uint8Array(cols * rows);
      this.receiverMirror = new Uint8Array(cols * rows);
      this.ownerCol = new Int32Array(cols * rows);
      this.ownerRow = new Int32Array(cols * rows);
      this.slotValid = new Uint8Array(cols * rows);
      this.dirtyCols = cols;
      this.dirtyRows = rows;
      forceCold = forceHot = true;
      // P2 rect lists: worst case is alternating dirty/clean along a row — ceil(cols/2) runs/row.
      const maxRects = rows * Math.ceil(cols / 2);
      this.coldRectPos = new Float32Array(maxRects * 12);
      this.hotRectPos = new Float32Array(maxRects * 12);
      this.receiverRectPos = new Float32Array(maxRects * 12);
      this.dirtyGeo?.destroy();
      this.dirtyGeo = new Geometry(this.renderer.gl, this.gather, {
        aPos: { data: this.coldRectPos, size: 2 },
      });
    }
    const forceRecv = this.forceReceiverDirty;
    this.forceReceiverDirty = false;
    const pm = (a: number, m: number): number => ((a % m) + m) % m;
    const baseC = pm(winCol, cols), baseR = pm(winRow, rows);
    this.coldMirror.fill(0); this.hotMirror.fill(0); this.receiverMirror.fill(0);
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
        if (forceRecv || ownerChanged) this.receiverMirror[si] = 1; // P3: new tiles need receiver values
      }
    }
    // P3: billboard/prim rects queued for the receiver channel (never light rects — F2).
    for (const [x0, y0, x1, y1] of this.receiverPending) {
      const c0 = Math.max(x0, winCol), c1 = Math.min(x1, winCol + cols - 1);
      const r0 = Math.max(y0, winRow), r1 = Math.min(y1, winRow + rows - 1);
      for (let wr = r0; wr <= r1; wr++)
        for (let wc = c0; wc <= c1; wc++) this.receiverMirror[pm(wr, rows) * cols + pm(wc, cols)] = 1;
    }
    this.receiverPending.length = 0;
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
    let dc = 0, cc = 0, hc = 0;
    for (let i = 0; i < this.coldMirror.length; i++) {
      if (this.coldMirror[i]) cc++;
      if (this.hotMirror[i]) hc++;
      if (this.coldMirror[i] || this.hotMirror[i]) dc++;
    }
    this.coldDirtyCount = cc;
    this.hotDirtyCount = hc;
    this.debugDirtyTiles = dc; // DEBUG: tiles recomputed this frame (cold ∪ hot)
    // P2: greedy row-merge each class's mirror into tile-aligned rects, emitted as raw NDC triangles.
    // Tile (sx, sy) spans the SAME fraction [sx/cols, (sx+1)/cols] × [sy/rows, (sy+1)/rows] of BOTH
    // the coarse shadow RT and the fine lightmap RT, so one vertex list serves both draws. cols/rows
    // are pow2 (SLOTS << lod), so every edge is dyadic → exact NDC → rect edges land on texel
    // boundaries and rasterization covers exactly the dirty tiles' texels.
    this.coldRectVerts = this.buildRects(this.coldMirror, cols, rows, this.coldRectPos, 0);
    this.hotRectVerts = this.buildRects(this.hotMirror, cols, rows, this.hotRectPos, 1);
    this.receiverRectVerts = this.buildRects(this.receiverMirror, cols, rows, this.receiverRectPos, 2);
    let rc = 0; for (let i = 0; i < this.receiverMirror.length; i++) if (this.receiverMirror[i]) rc++;
    this.receiverDirtyCount = rc;
  }

  /** P2 — row-merge `mirror`'s set tiles into rects and write NDC triangles into `out`.
   *  Returns the vertex count (6 per rect). `dbg` = index into the debug tallies. */
  private buildRects(mirror: Uint8Array, cols: number, rows: number, out: Float32Array, dbg: number): number {
    let v = 0, rects = 0, tiles = 0;
    for (let sy = 0; sy < rows; sy++) {
      const y0 = (sy / rows) * 2 - 1, y1 = ((sy + 1) / rows) * 2 - 1;
      for (let sx = 0; sx < cols; ) {
        if (!mirror[sy * cols + sx]) { sx++; continue; }
        let sx1 = sx;
        while (sx1 + 1 < cols && mirror[sy * cols + sx1 + 1]) sx1++;
        const x0 = (sx / cols) * 2 - 1, x1 = ((sx1 + 1) / cols) * 2 - 1;
        out[v * 2] = x0; out[v * 2 + 1] = y0;
        out[v * 2 + 2] = x1; out[v * 2 + 3] = y0;
        out[v * 2 + 4] = x1; out[v * 2 + 5] = y1;
        out[v * 2 + 6] = x0; out[v * 2 + 7] = y0;
        out[v * 2 + 8] = x1; out[v * 2 + 9] = y1;
        out[v * 2 + 10] = x0; out[v * 2 + 11] = y1;
        v += 6; rects++; tiles += sx1 - sx + 1;
        sx = sx1 + 1;
      }
    }
    this.debugRectCount[dbg] = rects;
    this.debugRectTiles[dbg] = tiles;
    return v;
  }

  get on(): boolean {
    return this.enabled;
  }
  /** Each orbiting carrier's ORIGIN, so the orbit is a bounded circle around where the prim actually
   *  stands rather than an unbounded drift that walks lights out of the world. */
  private readonly orbitHome = new Map<number, [number, number]>();
  private orbitPhase = 0;
  /** DEBUG: step the orbit one frame — displace every light-carrying prim around its origin and push
   *  BOTH the old and new boxes through the dirty front door.
   *
   *  This replaces the deleted debug light array ([I39](../../../../docs/work/2026-07-25-primitive-graph/issues.md)).
   *  Lights are carried by placed prims now, so making a light move means moving its CARRIER — there is
   *  no light to displace on its own. Marking the old box as well as the new one is not optional: skip it
   *  and the vacated tiles keep a stale shadow, because nothing else knows the caster left.
   *
   *  The toggle survived P5 while the thing it moved did not, so `__orbit(true)` returned `true` and moved
   *  nothing — a dead perf harness that read as a working one. Anything measuring motion cost through this
   *  should assert the frame is doing work (GPU time, not the dirty counter, which latches). */
  private stepOrbit(standing: Primitive[]): void {
    if (!this.orbit) return;
    this.orbitPhase += 0.05;
    const R = 0.75 * SQUARE;                       // orbit radius (world px) — under a tile, so a light
    for (const p of standing) {                    // stays in the room it lights
      if (!p.light) continue;
      let home = this.orbitHome.get(p.id);
      if (!home) { home = [p.x, p.y]; this.orbitHome.set(p.id, home); }
      this.markPrimDirty(p.x, p.y, SQUARE, SQUARE);          // vacated tiles — else a stale shadow persists
      const ph = this.orbitPhase + p.id;                     // per-prim phase so they don't move in lockstep
      p.x = home[0] + Math.cos(ph) * R;
      p.y = home[1] + Math.sin(ph) * R;
      this.markPrimDirty(p.x, p.y, SQUARE, SQUARE);          // newly occupied tiles
    }
  }
  /** DEBUG: turn the light orbit on/off (no arg = toggle). Off freezes the lights so the scene is
   *  static again (gather goes idle via dirty-gating); on drives a full recompute every frame. */
  setOrbit(on?: boolean): boolean {
    this.orbit = on ?? !this.orbit;
    if (!this.orbit) this.rebakeAll(); // one last clean recompute at the frozen positions
    return this.orbit;
  }
  /** P6: switch corridor ↔ brute walk (no arg = toggle) + recompute everything under the new path. */
  setCorridor(on?: boolean): boolean {
    this.corridor = on ?? !this.corridor;
    this.rebakeAll();
    return this.corridor;
  }
  /** DEBUG (shadows-onto-billboards): the receiver-elevation gain (no arg = read). Recomputes every tile. */
  setElevK(k?: number): number {
    if (k === undefined) return this.elevK;
    this.elevK = k;
    this.rebakeAll();
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
    this.coldShadowPrevRT?.destroy(); this.hotShadowPrevRT?.destroy();
    this.coldLightRT?.destroy(); this.hotLightRT?.destroy();
    const gl = this.renderer.gl;
    // FIXED SLOT GRID (work 2026-07-26-textile-slot): these ride the SAME 24×16 slot grid as the
    // G-buffer, each at its own texels-per-slot. Size is `SLOTS · texelsPerSlot` and is CONSTANT at every
    // zoom — it does NOT scale with `cols`, which is now `SLOTS << lod`. Per-TILE resolution degrades
    // with lod instead (`texelsPerSlot >> lod`), which is what stops the lightmap tracking zoom.
    const w = SLOTS_X * SHADOW_TEXELS, h = SLOTS_Y * SHADOW_TEXELS;
    // Shadow — SINGLE attachment: per-light u9 coverage. One RT per class; cleared to 0 (known-zero
    // persistence baseline, P3). The caster-row MRT attachment is deleted (standing-costs P1 — no reader).
    this.coldShadowRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint"] });
    this.hotShadowRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint"] });
    this.coldShadowPrevRT?.destroy(); this.hotShadowPrevRT?.destroy();
    this.coldShadowPrevRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint"] });
    this.hotShadowPrevRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint"] });
    for (const rt of [this.coldShadowRT, this.hotShadowRT, this.coldShadowPrevRT, this.hotShadowPrevRT]) {
      rt.bind();
      gl.clearBufferuiv(gl.COLOR, 0, new Uint32Array([0, 0, 0, 0]));
    }
    // Lightmap — lightmap P1 Step B: a FINE (TEXTILE_LIGHT/tile) single-attachment map holding the fully
    // accumulated per-light irradiance (Σ colour·falloff·(1−shadow)·N·L). 4× per axis of the coarse shadow;
    // att1 (aggregate dir) + att2 (unshadowed) are gone — subsumed by baking N·L (F5). NO ambient baked (the
    // blit adds it once over cold+hot). On the fixed grid this is `SLOTS · TEXTILE_LIGHT` = 2048×1024,
    // constant — it was `cols · TEXTILE_SQUARE` (world-sized), which is what made it 176 MB at zoom 0.25.
    // `TEXTILE_LIGHT`, NOT `TEXTILE_SQUARE`: all three fine maps are consumed by the LIGHTING pass, not by
    // the display, so they follow the pinned lighting resolution and do not grow with the art (work
    // `2026-07-28-square-128`). `receiverFineRT` belongs here too — it is receiver geometry sampled once
    // per lighting texel, so it has to match the lightmap it is read alongside.
    const fw = SLOTS_X * TEXTILE_LIGHT, fh = SLOTS_Y * TEXTILE_LIGHT;
    // lighting-feel P2: the decay lightmap — coarse (shadow-RT geometry), RGBA16F, cleared to zero.
    this.decayRT?.destroy();
    this.decayRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba16float"] });
    this.decayRT.bind();
    gl.clearBufferfv(gl.COLOR, 0, new Float32Array([0, 0, 0, 0]));
    // P3 receiver maps — persistent, baked under the receiver-dirty channel. Coarse (gather res):
    // baseY/id/allB/rcov as raw f32 bits; fine (lightmap res): id|presence only (R32UI, 33 MB).
    this.receiverCoarseRT?.destroy(); this.receiverFineRT?.destroy();
    this.receiverCoarseRT = new RenderTarget(gl, { width: w, height: h, formats: ["rgba32uint"] });
    this.receiverFineRT = new RenderTarget(gl, { width: fw, height: fh, formats: ["r32uint"] });
    this.receiverCoarseRT.bind();
    // Ground defaults (baseY −1, id 0, rcov 0) — every texel is force-baked before first read anyway.
    gl.clearBufferuiv(gl.COLOR, 0, new Uint32Array([0xbf800000, 0, 0, 0]));
    this.receiverFineRT.bind();
    gl.clearBufferuiv(gl.COLOR, 0, new Uint32Array([0, 0, 0, 0]));
    // RGBA32F, not RGBA8 (F11b): the lightmap is an ADDITIVE ACCUMULATOR. Each light's contribution is
    // QUANTISED to an integer 0..LIGHT_QUANT and blended in with `blendFunc(ONE, ONE)`; a light is
    // removed by emitting the same value negated. FP32 represents every integer below 2^24 exactly, so
    // that add/subtract pair cancels BIT-EXACTLY in any order — which is what makes an update invertible
    // instead of forcing a clear-and-recast. Measured: 4096 quantised add/subtract pairs return a texel
    // to exactly 0, while the same test on unquantised floats drifts by 1.8e-7.
    this.coldLightRT = new RenderTarget(gl, { width: fw, height: fh, formats: ["rgba32float"] });
    this.hotLightRT = new RenderTarget(gl, { width: fw, height: fh, formats: ["rgba32float"] });
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
    // P5: do NOT gate on the debug array. Content-carried lights are registered *by* `buildCasters`,
    // which runs below — so bailing before it runs would make a content-lit world unreachable
    // (nothing would ever discover the torches). Bail only when there is genuinely nothing to do.
    if (!this.enabled) return;
    if (this.coldData.carriedLights.size === 0 && standing.length === 0) return;

    // No light MOTION here: a light moves only because the primitive carrying it moved, and that is
    // the placement path's business (`markLightDirty`, from `buildCasters`). The bespoke debug light
    // array is GONE (P5) — every light in the world is now carried by a placed primitive, authored per
    // kind in content. The debug orbit is BACK (I39) but obeys that same rule: `stepOrbit` moves the
    // CARRIERS through the dirty front door, it does not move lights behind the placement path's back.
    // Same reasoning: `coldData.lights` counts only the debug array's records, so a world lit purely
    // by carried lights must not be turned away here either.
    if (win.cols === 0) return;
    if (this.coldData.carriedLights.size === 0 && standing.length === 0) return;

    this.ensureRT(win.cols, win.rows);
    // P3: the window mapping rides the data texture's constants row (compare-written — an
    // unchanged window costs nothing), through the same scatter path as every other write.
    // `slot` is the SHADOW map's per-TILE texel size at this lod. The RT is a fixed SLOTS·SHADOW_TEXELS,
    // and `cols` is SLOTS << lod, so texels-per-tile must shrink by the same factor for `fc / uSlot` to
    // keep addressing tiles: SHADOW_TEXELS >> lod. `win.lod` rides along so shaders read one mapping.
    this.coldData.setConstants(win.cols, win.rows, win.winCol, win.winRow, SHADOW_TEXELS >> win.lod,
                               this.coldData.lights, Math.round(this.worldTiltDeg * 100), win.lod);
    // ORDER IS LOAD-BEARING: casters BEFORE presence. `buildCasters` DISCOVERS content-carried lights
    // and queues their dirty rects; `buildDirty` (below) consumes those rects and `classPass` bakes the
    // tiles. If presence ran first it would still hold the OLD light set, so those tiles bake WITHOUT
    // the new light — and next frame presence is correct but the rects are already spent, so nothing
    // re-bakes. The light sits in presence, permanently unlit.
    // This was masked for a long time by the debug light array: an orbiting light re-dirtied tiles every
    // frame, so a later frame happened to bake with correct presence. Deleting the scaffold exposed it.
    this.buildCasters(standing, resolver, win); // P1 caster buckets + carried-light discovery
    this.buildPresence(win);                    // F5 per-tile light cull — must see what casters found
    // ONE scatter batch for everything both passes queued (defs, billboards, lights, buckets, presence).
    // NO force-all: each change queued its own scoped rects through a `markDirty` door.
    this.coldData.flush();
    this.buildDirty(win);    // #4: owner-change (pan) + per-class rebuild → cold/hot dirty; clean tiles persist
    // #4 COLD then HOT pass. Each is: gather (shadow-cold for that class) → lighting (its lightmap), gated
    // by that class's dirty texture (clean tiles `discard` → persist). A frame where only the green (hot)
    // light moves leaves cold-dirty EMPTY → both cold draws no-op (every fragment discards), so the static
    // lights + their shadows are never recomputed. Each gather is corridor↔brute identical within its class.
    // Standing-costs P1: a class with ZERO dirty tiles skips its whole pass. Correct because the pass
    // is dirty-gated end to end — with nothing dirty the gather leaves the shadow RT untouched and the
    // lighting draw discards every fragment, so submitting them only rasterizes prologues (~8.4 M fine
    // fragments/class) to change nothing. The prev snapshot stays valid for the same reason: an
    // untouched shadow RT means prev == current already.
    this.debugClassDraws = 0;
    this.debugReceiverDraws = 0;
    // P3: bake the receiver maps FIRST — the class passes read them this same frame. Runs only when
    // a billboard/prim changed, the window moved, or a force-all fired; a pure light move skips it.
    if (this.receiverRectVerts > 0 && this.dirtyGeo) {
      this.debugReceiverDraws = 1;
      this.dirtyGeo.update("aPos", this.receiverRectPos.subarray(0, this.receiverRectVerts * 2));
      this.renderer.draw({
        program: this.recvFine,
        geometry: this.dirtyGeo,
        count: this.receiverRectVerts,
        target: this.receiverFineRT!,
        blend: "none",
        textures: {
          uData: this.coldData.dataTexture,
          uSurface: this.coldData.surfacePage ?? this.empty,
        },
        uniforms: (p) => p.uVec2("uLightAlign", this.lightAlignX, this.lightAlignY),
      });
      this.renderer.draw({
        program: this.recvCoarse,
        geometry: this.dirtyGeo,
        count: this.receiverRectVerts,
        target: this.receiverCoarseRT!,
        blend: "none",
        textures: {
          uData: this.coldData.dataTexture,
          uSurface: this.coldData.surfacePage ?? this.empty,
        },
      });
    }
    if (this.coldDirtyCount > 0) {
      this.classPass(0, this.coldShadowRT!, this.coldLightRT!, this.coldShadowPrevRT);
      this.debugClassDraws++;
      this.bakeCounts[0] += this.coldDirtyCount;
    }
    if (this.hotDirtyCount > 0) {
      this.classPass(1, this.hotShadowRT!, this.hotLightRT!, this.hotShadowPrevRT);
      this.debugClassDraws++;
      this.bakeCounts[1] += this.hotDirtyCount;
    }
    // lighting-feel P2: fade the decay map + splat this frame's particles. Runs every frame — the
    // fade is one blend-state draw over 131 k texels and the splats are a handful of tiny quads;
    // neither touches the accumulators or the dirty machinery.
    this.decayAndSplat(win);
    // hot-sync P1: the light-cascade dedup spans ONE frame across moverDirty (pre-tick) and
    // buildCasters (in-tick) — reset here so next frame's pre-tick calls start fresh.
    this.litSeen.clear();
  }

  /** hot-sync P1 (F1 — ONE hot dirty): a MOVER changed visually this frame. Called by the
   *  Viewport from the mover's OWN eps crossing — the same event that re-bakes its sprite —
   *  so the record rewrite, the hot light/shadow rects, and the receiver rects all step from
   *  ONE position snapshot. `buildCasters` keeps its change-detection as a BACKSTOP (first
   *  sight, def swaps on zoom); with the record already rewritten here it sees changed=false
   *  on plain moves and originates nothing. hot-sync P3: `billboardDataFor` stamps the anchor's
   *  sub-unit fraction (1 px grain) AND snaps the hot prim's x/y to the record-decoded anchor
   *  before this method reads them — the record is the position authority; the rects below and
   *  the sprite bake both derive from what it stamped. */
  moverDirty(prim: Primitive, resolver: TextureResolver | null): void {
    const def = this.coldData.definitionFor(prim, resolver);
    if (def < 0) return;
    const inst = this.coldData.billboardDataFor(prim, def, resolver);
    this.debugMoverDirtyCalls++;
    if (!inst.changed) return; // sub-unit glide — nothing the lighting can express
    this.debugMoverDirtyChanges++;
    const t = this.coldData.tightBoxOf(def) ?? { dx: 0, dy: 0, w: prim.width, h: prim.height };
    const tdx = prim.flipX ? prim.width - (t.dx + t.w) : t.dx;
    const tx = prim.x + tdx, ty = prim.y + t.dy;
    const lb = this.lastBox.get(prim.id);
    if (lb !== undefined && (lb[0] !== tx || lb[1] !== ty)) {
      this.markBillboardDirty(lb[0], lb[1], lb[2], lb[3], 1); // the OLD box — no stale silhouette
    }
    this.markBillboardDirty(tx, ty, t.w, t.h, 1);
    if (lb === undefined) this.lastBox.set(prim.id, [tx, ty, t.w, t.h, 1]);
    else { lb[0] = tx; lb[1] = ty; lb[2] = t.w; lb[3] = t.h; lb[4] = 1; }
  }
  /** ui-select P0 (D2): the TIGHT box (world px, ABSOLUTE) of a standing prim's silhouette,
   *  for hit tests — def-resolved, mirrored for west-facing; full box when no tight data;
   *  null → not a billboard (no def). */
  tightBoxFor(prim: Primitive, resolver: TextureResolver | null): { x: number; y: number; w: number; h: number } | null {
    const def = this.coldData.definitionFor(prim, resolver);
    if (def < 0) return null;
    const t = this.coldData.tightBoxOf(def);
    if (!t) return { x: prim.x, y: prim.y, w: prim.width, h: prim.height };
    const dx = prim.flipX ? prim.width - (t.dx + t.w) : t.dx;
    return { x: prim.x + dx, y: prim.y + t.dy, w: t.w, h: t.h };
  }

  /** DEBUG (hot-sync): moverDirty call/changed counters — the P2 lockstep drill reads these. */
  debugMoverDirtyCalls = 0;
  debugMoverDirtyChanges = 0;
  /** DEBUG (hot-sync): hot-prim changes the BACKSTOP (buildCasters) originated — 0 during walks. */
  debugBackstopMoves = 0;

  /** The DECAY LIGHTMAP frame stage (lighting-feel P2): one in-place `dst *= k` fade (F2), then the
   *  v1 flicker emitter (F5) splats a jittered particle per flickering carried light, each
   *  shadow-stamped by its parent's class shadow RT (F4). Ephemeral by design — see the field docs. */
  private decayAndSplat(win: TileWindow): void {
    if (!this.decayRT || !this.splatQuad) return;
    const now = performance.now();
    const dt = this.lastDecayMs > 0 ? Math.min((now - this.lastDecayMs) / 1000, 0.25) : 1 / 60;
    this.lastDecayMs = now;
    const k = Math.exp(-dt / this.decayTau);
    this.renderer.draw({
      program: this.decayProg,
      geometry: this.fsQuad,
      target: this.decayRT,
      blend: "mulConstant",
      blendColor: [k, k, k, k],
    });
    this.debugSplats = 0;
    if (!this.flickerOn) return;
    const { cols, rows, winCol, winRow } = win;
    const pm = (a: number, m: number): number => ((a % m) + m) % m;
    for (const [id, L] of this.coldData.carriedLights) {
      if (!(L.flicker || this.flickerAll)) continue;
      // One splat per frame per light: jittered off the source, intensity noise. Radius rides the
      // light's reach a little so big lights breathe wider.
      const jr = 0.45 * SQUARE;
      const wx = L.x + (Math.random() * 2 - 1) * jr;
      const wy = L.y + (Math.random() * 2 - 1) * jr;
      const radiusPx = Math.min(L.reach * 0.45, 2.2 * SQUARE) * (0.8 + 0.4 * Math.random());
      // Deposit scaled by dt (nominal 60 fps = 1×) so the map's EQUILIBRIUM brightness is
      // frame-rate independent — one splat per frame deposits energy ∝ elapsed time, and the
      // time-based decay drains it symmetrically. Without this, 120 fps glows twice as bright.
      const intensity = this.flickerStr * L.intensity * (0.4 + 0.6 * Math.random()) * Math.min(dt * 60, 3);
      const tx = Math.floor(wx / SQUARE), ty = Math.floor(wy / SQUARE);
      if (tx < winCol || tx >= winCol + cols || ty < winRow || ty >= winRow + rows) continue;
      // World → toroidal-slot UV (the same `pm(tile, cols)` slot the dirty mirror uses). A splat
      // straddling the wrap seam clips at the seam — the seam lives in overscan, off-visible.
      const cu = (pm(tx, cols) + (wx / SQUARE - tx)) / cols;
      const cv = (pm(ty, rows) + (wy / SQUARE - ty)) / rows;
      this.renderer.draw({
        program: this.splatProg,
        geometry: this.splatQuad,
        target: this.decayRT,
        blend: "add",
        textures: {
          uShadowStamp: (L.hot ? this.hotShadowRT! : this.coldShadowRT!).textures[0],
          uData: this.coldData.dataTexture,
        },
        uniforms: (p) => {
          p.uVec2("uCenterUV", cu, cv);
          p.uVec2("uRadUV", radiusPx / (SQUARE * cols), radiusPx / (SQUARE * rows));
          p.uVec3("uColor", L.color[0], L.color[1], L.color[2]);
          p.uFloat("uIntensity", intensity);
          p.uInt("uLightId", id);
        },
      });
      this.debugSplats++;
    }
  }

  /** The decay lightmap texture for the display blit (coarse, RGBA16F, un-quantised irradiance). */
  get decayMap(): Texture | null { return this.decayRT?.textures[0] ?? null; }

  /** One class's shadow + lighting bake (#4). `cls` 0 = cold / 1 = hot; `dirty` gates it (clean → persist);
   *  the gather writes `shadowRT` (this class's per-light u9), the lighting reads it into `lightRT`. */
  private classPass(cls: number, shadowRT: RenderTarget, lightRT: RenderTarget,
                    shadowPrevRT: RenderTarget | null): void {
    // SNAPSHOT the shadow BEFORE the gather overwrites it ([I36]). Pairs with the data texture's pre-flush
    // copy: together they let a pass reproduce a light's OLD contribution exactly — old light records
    // against old shadow — which is what makes removal exact instead of approximate.
    // GATED OFF until the differential lands (standing-costs P1): `uShadowPrev` has NO consumer today —
    // nothing binds the prev RT — so this blit copied ~2 MB/class/frame to feed nothing. Whoever wires
    // the differential (accumulateLights' uDataPrev/uShadowPrev parameters) flips DIFFERENTIAL_WIRED.
    if (DIFFERENTIAL_WIRED && shadowPrevRT) {
      const gl = this.renderer.gl;
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, shadowRT.fbo);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, shadowPrevRT.fbo);
      gl.blitFramebuffer(0, 0, shadowRT.width, shadowRT.height, 0, 0, shadowRT.width, shadowRT.height,
                         gl.COLOR_BUFFER_BIT, gl.NEAREST);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
    }
    // P2 (standing-costs): rasterize ONLY the dirty rects. One NDC vertex list serves both draws
    // (tile fractions are resolution-independent). The per-texel dirty gate is retired — rect
    // coverage was proven bit-identical to the gated fullscreen build before the gate came out.
    const rectPos = cls === 0 ? this.coldRectPos : this.hotRectPos;
    const verts = cls === 0 ? this.coldRectVerts : this.hotRectVerts;
    if (verts === 0 || !this.dirtyGeo) return;
    this.dirtyGeo.update("aPos", rectPos.subarray(0, verts * 2));
    this.renderer.draw({
      program: this.gather,
      geometry: this.dirtyGeo,
      count: verts,
      target: shadowRT,
      blend: "none",
      textures: {
        uData: this.coldData.dataTexture, // defs | billboards | lights | presence | buckets
        uRecvCoarse: this.receiverCoarseRT!.textures[0], // P3: baked receiver (baseY | id|allB | rcov)
        uSurface: this.coldData.surfacePage ?? this.empty, // no page yet → defs have no frame → solid quads
      },
      uniforms: (p) => {
        p.uInt("uCorridor", this.corridor ? 1 : 0);
        p.uInt("uLightClass", cls);
        p.uInt("uGProfile", this.gatherProfile);
        p.uInt("uWProfile", this.walkProfile);
        p.uInt("uCProfile", this.casterProfile);
        p.uFloat("uCardLean", this.cardLean);
        { const r = this.worldTiltDeg * Math.PI / 180; p.uVec2("uTilt", Math.sin(r), Math.cos(r)); }
        p.uFloat("uElevK", this.elevK);
        // D9: the walks' southern occupancy dilation. In the ORACLE's extent mode the
        // dilation is ZERO — extent registration + undilated walks IS the pre-reshape
        // behavior exactly (extent + dilation would over-reach by span+dilation rows).
        p.uInt("uDilateS", this.extentFill ? 0 : this.dilationRows());
      },
    });
    this.renderer.draw({
      program: this.lighting,
      geometry: this.dirtyGeo,
      count: verts,
      target: lightRT,
      blend: "none",
      textures: {
        uData: this.coldData.dataTexture, // presence (sets 3/5) + light records (set 2)
        uRecvFine: this.receiverFineRT!.textures[0], // P3: baked receiver id (fine)
        uShadow: shadowRT.textures[0], // this class's shadow-cold (COARSE — the fine bake upsamples it)
        // pawn-render P3: the COLD class's shadow map — the hot pass's delta reference
        // (unused by the cold pass, which binds its own map twice).
        uShadowCold: this.coldShadowRT!.textures[0],
        uSurface: this.coldData.surfacePage ?? this.empty, // lightmap (CO-PACK): silhouette + normal quadrants share this page
      },
      uniforms: (p) => {
        p.uInt("uLightClass", cls);
        p.uInt("uWorldLight", this.worldLight ? 1 : 0);
        p.uFloat("uNsInv", this.nsInv);
        p.uFloat("uFalloff", this.falloff);
        p.uFloat("uWarmShift", this.warmShift);
        p.uFloat("uRimSat", this.rimSat);
        p.uFloat("uGlintStr", this.glintStr);
        p.uFloat("uGlintPow", this.glintPow);
        p.uInt("uShowNormal", this.showNormal ? 1 : 0);
        p.uFloat("uNormalPitch", this.normalPitchDeg * Math.PI / 180);
        p.uVec2("uLightAlign", this.lightAlignX, this.lightAlignY);
        p.uInt("uProfile", this.profile);
        p.uInt("uHideRight", this.hideRight ? 1 : 0);
        p.uInt("uShadowFilter", this.shadowFilter ? 1 : 0);
      },
    });
  }

  /** Decode shadow-cold over the world (debug `/overlayRT shadow-cold`). */
  drawOverlay(camera: Camera, win: TileWindow): void {
    if (!this.coldShadowRT || !this.hotShadowRT || win.cols === 0) return;
    // RENDER SCALE, not logical zoom (fixed 2026-07-27, `moving-lights` I17). `renderScale = zoom × cover`
    // and is what EVERY world↔screen transform uses — the display blit included. The overlay was building
    // its projection from `camera.zoom`, so it drew the world at 1/cover of the right size (≈0.66× on a
    // 2560-wide canvas) AND mis-scaled the anchor term, which is why it drifted further off the more you
    // panned from the origin and jumped whenever the cover fit changed. The fragment's world→texel mapping
    // was always correct; only this matrix was wrong.
    const w = camera.width, h = camera.height, rs = camera.renderScale;
    const ax = camera.anchorX, ay = camera.anchorY;
    // Cover the window's world rect.
    const x0 = win.winCol * SQUARE, y0 = win.winRow * SQUARE;
    const x1 = (win.winCol + win.cols) * SQUARE, y1 = (win.winRow + win.rows) * SQUARE;
    this.overlayPos.set([x0, y0, x1, y0, x1, y1, x0, y1]);
    this.overlayGeo!.update("aWorld", this.overlayPos);
    const proj = new Float32Array([(2 * rs) / w, 0, 0, 0, -(2 * rs) / h, 0,
                                   (-ax * 2 * rs) / w, (ay * 2 * rs) / h, 1]);
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
        p.uInt("uSlot", SHADOW_TEXELS >> win.lod); // per-TILE texels on the fixed grid, not the per-SLOT constant
      },
    });

    // Gizmos: a dot at each light + a ring at its radius (screen-constant thickness).
    // Also renderScale: this is "how many world px is one SCREEN px", so it must carry the cover fit or
    // the rings thicken/thin with the viewport instead of staying screen-constant.
    const pxWorld = 1 / rs;
    let gi = 0;
    for (const L of this.coldData.carriedLights.values()) {
      const col = LIGHT_COLORS[gi++ % LIGHT_COLORS.length];
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
    this.dirtyGeo?.destroy();
    this.coldShadowRT?.destroy(); this.hotShadowRT?.destroy();
    this.coldLightRT?.destroy(); this.hotLightRT?.destroy();
    this.receiverCoarseRT?.destroy(); this.receiverFineRT?.destroy();
    this.recvCoarse.destroy(); this.recvFine.destroy();
    this.decayRT?.destroy();
    this.decayProg.destroy(); this.splatProg.destroy(); this.splatQuad?.destroy();
    this.coldData.destroy();
  }
}
