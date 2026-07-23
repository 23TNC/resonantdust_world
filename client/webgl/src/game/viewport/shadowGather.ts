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
import { ColdShadowData } from "./coldShadowData";
import { SQUARE, UNIT, TEXTILE_UNIT } from "./squareMath";

/** Lights this iteration — a ring of debug lights around the seed tile. `number`-typed so the
 *  isolate-one-light debug path (`MAX_LIGHTS = 1`) below isn't flagged as a constant comparison. */
const MAX_LIGHTS: number = 1;
/** Light height: 40 units = 160 world px = 2.5 tiles — above the tree billboard (2 tiles / 32 units).
 *  Lower = longer shadows. */
const LIGHT_Z = 40 * UNIT;
const LIGHT_REACH = 12 * SQUARE;     // illumination range (world px) — how far the light throws (12 tiles)
const LIGHT_EMITTER = 12;            // physical source size (world px = 3 units) — penumbra softness
const RING_RADIUS = 2 * SQUARE;
// The shadow map is the TEXTILE_UNIT map (map-model.md): 16 textiles/tile, 1 textile = 1 unit
// (= UNIT px). Sized `cols·TEXTILE_UNIT × rows·TEXTILE_UNIT`, toroidal like the cold cache window.
/** GLSL literals for the world constants (a tile is `SQUARE` world px; `1 unit = SQUARE/16` px). */
const SQF = SQUARE.toFixed(1);
const UNITF = UNIT.toFixed(4);
/** Lift the rendered shadow up (toward smaller world-y) by this many world units — a fragment shows
 *  shadow if the point this far BELOW it is shadowed, so the whole silhouette slides up. Tunable. */
const SHADOW_LIFT = 0.0;             // DEBUG: shadow offset OFF (isolating the per-tile miscalc)
const SHADOW_LIFTF = SHADOW_LIFT.toFixed(1);

interface Light {
  x: number;
  y: number;
  z: number;
  reach: number;         // illumination range (world px)
  emitterRadius: number; // physical source size (world px) — penumbra softness
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
const int DEF_BASE = 0, PRIM_BASE = 65536, LIGHT_BASE = 131072;
uvec4 fetchLin(highp usampler2D t, int i) { return texelFetch(t, ivec2(i & 1023, i >> 10), 0); }
vec2 decodePos(uint p) {          // position_anchor_reference → world UNITS
  uint region = (p >> 24) & 255u, zone = (p >> 16) & 255u, tile = (p >> 8) & 255u, anchor = p & 255u;
  uint wtx = (((region >> 4u) * RD + (zone >> 4u)) * ZD + (tile >> 4u));
  uint wty = (((region & 15u) * RD + (zone & 15u)) * ZD + (tile & 15u));
  return vec2(float(wtx * 16u + (anchor >> 4u)), float(wty * 16u + (anchor & 15u)));
}
// PURE QUAD placement (no texture sample, no u/v inversion). The caster is a flat 3D card: base on the
// ground (y = Yb, z = 0), top tilted north + elevated (y = Yt, z = Zt). Project its 4 corners from the
// light onto the ground → a convex ground quad; P is shadowed iff it lies inside that quad. The only
// division is the single top-corner projection factor k — no per-pixel /denom or /s to blow up.
//   card corners: base (A.x±W/2, Yb, 0)  ·  top (A.x±W/2, Yt, Zt)
//   ground(C) = L.xy + (L.z/(L.z - C.z)) * (C.xy - L.xy)   → base stays put, top scales by k
float cross2(vec2 a, vec2 b) { return a.x * b.y - a.y * b.x; }
float shadowCover(vec2 P, vec2 A, vec3 L, float W, float H) {
  float th = 65.0 * 3.14159265 / 180.0, ct = cos(th), st = sin(th);
  float Yt = A.y - 0.5 * H * ct, Zt = H * st;    // card top: tilted north + elevated
  float Yb = A.y;                                 // card base on the ground (z = 0)
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
float casterCover(uint primIdx, vec2 P, vec3 L, highp usampler2D data, sampler2D surf) {
  if (primIdx == 0u) return 0.0;
  uvec4 Pd = fetchLin(data, PRIM_BASE + int(primIdx));      // 1 px/record (F1): R position, G orient
  uint pos = Pd.x;
  uint orient = Pd.y;
  vec2 A = decodePos(pos);                                  // the prim's stored anchor (full-box base-centre)
  int defIdx = int((orient >> 6) & 0xffffu);
  uvec4 D = fetchLin(data, DEF_BASE + defIdx);
  // R: bbox size (EVEN units, stored /2) + frame span (tiles, stored −1). G: bbox top-left in the
  // frame (units, unsigned). B: frame origin (16-px grid) + page + lod exponent + 3x3 anchors.
  float W = float(((D.x >> 23) & 511u) * 2u);
  float H = float(((D.x >> 14) & 511u) * 2u);
  float spanU = float((((D.x >> 10) & 15u) + 1u) * 16u);    // frame world span (units)
  float ox = float((D.y >> 22) & 1023u), oy = float((D.y >> 12) & 1023u);
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
  float q = shadowCover(P, Ac, L, W, H);
  if (q <= 0.0) return 0.0;
  if (lod < 4u) return q;                                   // no silhouette resolved yet → solid quad
  // Invert the ground projection: card(s,t) → ground is linear in t (one division, no cliff).
  //   y(t) = Ac.y − 0.5·t·H·cosθ, z(t) = t·H·sinθ; ground(C) = L.xy + (L.z/(L.z−C.z))·(C.xy−L.xy)
  //   ⇒ t = L.z·(P.y − Ac.y) / (H·(sinθ·(P.y − L.y) − 0.5·L.z·cosθ))
  float th = 65.0 * 3.14159265 / 180.0, ct = cos(th), st = sin(th);
  float denom = H * (st * (P.y - L.y) - 0.5 * L.z * ct);
  if (abs(denom) < 1e-4) return q;                          // degenerate (grazing) — keep the solid quad
  float t = clamp(L.z * (P.y - Ac.y) / denom, 0.0, 1.0);
  float k = L.z / (L.z - t * H * st);                       // that row's projection factor
  float s = clamp(((P.x - L.x) / k + L.x - Ac.x) / W + 0.5, 0.0, 1.0);
  if (rot == 3u) s = 1.0 - s;                               // W-facing = mirrored E frame
  // Whole-px-per-unit sampling: ppu = 2^lod / spanU (a pow2 ≥ 1 by construction). Window top-left =
  // frame origin + offset·ppu − nudge (x signed +1024 — centers the opaque run; y unsigned, upward —
  // bottom-aligns it). Atlas rows are image-top-down; card t=0 is the sprite's BOTTOM row → v = 1−t.
  float ppu = float(1u << lod) / spanU;
  float fx = float((D.z >> 22) & 1023u) * 16.0, fy = float((D.z >> 12) & 1023u) * 16.0;
  float nx = float(int((D.w >> 20) & 4095u) - 2048);   // u12, +2048 bias — full either-direction range
  float ny = float(int((D.w >> 8) & 4095u) - 2048);
  vec2 uv = vec2(fx, fy) + vec2(ox, oy) * ppu - vec2(nx, ny) + vec2(s * W, (1.0 - t) * H) * ppu;
  return q * texelFetch(surf, ivec2(uv), 0).b;              // surface B = coverage (straight-alpha data)
}
`;

const FULLSCREEN_VERT = /* glsl */ `#version 300 es
in vec2 aPos;                    // NDC -1..1 (2 tris)
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

const GATHER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uData;       // THE unified data texture (defs | prims | lights bands)
uniform highp usampler2D uPresence;   // light_presence_cold — textile_tile map (1 textile/tile)
uniform highp usampler2D uDirty;      // shadow_dirty — textile_tile map (R8UI): .r nonzero = recompute
uniform highp usampler2D uCaster;     // caster buckets — textile_tile map (RGBA32UI = 8× u16 prim idx)
uniform sampler2D uSurface;           // the shared surface atlas page (F2) — silhouette coverage in B
uniform int uCols, uRows, uWinCol, uWinRow, uSlot;   // shadow-cold toroidal window; uSlot = TEXTILE_UNIT
uniform int uCorridor;                // P6: 1 = segment-DDA corridor walk, 0 = brute-force reach box
out uvec4 fragColor;
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlot, sy = fc.y / uSlot;                 // toroidal slot
  if (texelFetch(uDirty, ivec2(sx, sy), 0).r == 0u) discard; // clean tile → keep the persistent texel
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols); // → world tile
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlot)) / float(uSlot); // 0..1 within the tile
  float ly = (float(fc.y) - float(sy * uSlot)) / float(uSlot);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS
  P.y += ${SHADOW_LIFTF}; // lift the shadow up: test the ground point SHADOW_LIFT units below this one

  uvec4 pres = texelFetch(uPresence, ivec2(sx, sy), 0);     // 8× u16 nearest light indices (0xFFFF empty)
  uint out0 = 0u;                                            // per-SLOT 4-bit coverage: slot i at bit i*4
  for (int slot = 0; slot < 8; slot++) {
    uint pw = pres[slot >> 1];
    uint li = (slot & 1) == 0 ? (pw >> 16) : (pw & 0xffffu);
    if (li == 0xffffu) continue;                            // empty slot
    uvec4 Ld = fetchLin(uData, LIGHT_BASE + int(li));
    if ((Ld.z & 1u) == 0u) continue;                        // cast_shadows
    vec3 L = vec3(decodePos(Ld.x), float((Ld.z >> 24) & 255u));
    float cov = 0.0;
    if (uCorridor == 1) {
      // P6 CORRIDOR — the pure optimization, validated against the brute box below. Why the segment
      // suffices: P occluded by a caster means P = ground(C) = L.xy + f·(C.xy − L.xy) for a card
      // point C with f ≥ 1, so C.xy = L.xy + (P − L.xy)/f lies ON the segment light→P — and C.xy is
      // inside the caster's ground rect, which is EXACTLY what buildCasters buckets (I-7). So every
      // occluding caster has a bucketed tile on the segment. Walk the segment's tiles by sampling at
      // ≤1-tile steps + a ±1 cross pad (covers corner crossings + float edges — no wedge bug, no
      // step cap, same pmod slot mapping as the buckets). Constant loop bound; only m in the
      // condition (the GLSL loop-condition foot-gun).
      vec2 a = L.xy / UPT;                                   // light in tile coords
      vec2 b = P / UPT;                                      // texel in tile coords
      vec2 d = b - a;
      int nsteps = int(ceil(abs(d.x)) + ceil(abs(d.y))) + 1; // ≥ max-axis span → ≤1 tile between samples
      for (int m = 0; m <= 48; m++) {                        // constant bound (brute caps reach at 16 tiles too)
        if (m > nsteps) break;
        vec2 q = a + d * (float(m) / float(max(nsteps, 1)));
        ivec2 qt = ivec2(floor(q));
        for (int n = 0; n < 5; n++) {                        // cross pad: centre, ±x, ±y
          ivec2 o = qt + ivec2(n == 1 ? 1 : (n == 2 ? -1 : 0), n == 3 ? 1 : (n == 4 ? -1 : 0));
          ivec2 s = ivec2(pmod(o.x, uCols), pmod(o.y, uRows));
          uvec4 cb = texelFetch(uCaster, s, 0);
          for (int c = 0; c < 8; c++) {
            uint word = cb[c >> 1];
            uint primIdx = (c & 1) == 0 ? (word >> 16) : (word & 0xffffu);
            if (primIdx != 0u) cov = max(cov, casterCover(primIdx, P, L, uData, uSurface)); // MAX: idempotent per caster — visit-count must not matter (P6 identity)
          }
        }
      }
    } else {
      // BRUTE FORCE (the baseline the corridor must match): walk EVERY tile in the light's reach box
      // and test each bucketed caster's quad against P.
      int reachT = int((Ld.z >> 12) & 0xfffu) / int(UPT) + 1; // reach (units) → tiles, +1 margin
      ivec2 lc = ivec2(floor(L.xy / UPT));                     // light's tile
      ivec2 ls = ivec2(pmod(lc.x, uCols), pmod(lc.y, uRows));  // light's SLOT (rigid: caster-texel of the light tile)
      for (int dy = -16; dy <= 16; dy++) {
        if (dy < -reachT || dy > reachT) continue;
        for (int dx = -16; dx <= 16; dx++) {
          if (dx < -reachT || dx > reachT) continue;
          ivec2 s = ivec2(pmod(ls.x + dx, uCols), pmod(ls.y + dy, uRows)); // wrap within the caster texture
          uvec4 cb = texelFetch(uCaster, s, 0);
          for (int c = 0; c < 8; c++) {
            uint word = cb[c >> 1];
            uint primIdx = (c & 1) == 0 ? (word >> 16) : (word & 0xffffu);
            if (primIdx != 0u) cov = max(cov, casterCover(primIdx, P, L, uData, uSurface)); // MAX: idempotent per caster — visit-count must not matter (P6 identity)
          }
        }
      }
    }
    // Pack this SLOT's coverage as a 4-bit nibble at bit slot*4 (8 slots → 32 bits → R channel).
    uint nib = uint(cov * 15.0 + 0.5);
    out0 |= nib << uint(slot * 4);
  }
  fragColor = uvec4(out0, 0u, 0u, 0u);
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
uniform highp usampler2D uShadow;   // shadow-cold — textile_unit map: per-slot 4-bit coverage (8 nibbles in R)
uniform highp usampler2D uPresence; // textile_tile map: 8× u16 light indices per tile (slot → global light)
uniform int uCols, uRows, uWinCol, uWinRow, uSlot;  // uSlot = TEXTILE_UNIT
out vec4 fragColor;
int pmod(int a, int m) { return ((a % m) + m) % m; }
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
  uvec4 pres = texelFetch(uPresence, ivec2(sx, sy), 0);     // this tile's 8 slot → light index
  uint sh = texelFetch(uShadow, texel, 0).x;                // 8 nibbles: slot i coverage at bit i*4
  vec3 acc = vec3(0.0);
  float any = 0.0;
  for (int slot = 0; slot < 8; slot++) {
    uint pw = pres[slot >> 1];
    uint li = (slot & 1) == 0 ? (pw >> 16) : (pw & 0xffffu);
    if (li == 0xffffu) continue;                            // empty slot
    uint nib = (sh >> uint(slot * 4)) & 0xFu;
    if (nib > 0u) { float cvg = float(nib) / 15.0; acc += lightColour(int(li)) * cvg; any = max(any, cvg); }
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
  private readonly coldData: ColdShadowData;
  private lastCasterCount = -1;
  private coldDirty = true;

  /** shadow-cold RT — the **textile_unit map** (`cols·TEXTILE_UNIT × rows·TEXTILE_UNIT` RGBA32UI,
   *  world-space toroidal), resized when the tile window changes. */
  private shadowRT: RenderTarget | null = null;
  private rtCols = 0;
  private rtRows = 0;

  /** light presence — a **textile_tile map**: per-tile **8× `u16` light indices** (nearest-8 reaching
   *  lights, `0xFFFF` = empty), one `RGBA32UI` textile/tile (P5). Per-texel gather cost O(8), light-count
   *  independent.
   *  `slotDist` tracks each slot's distance² for the nearest-N eviction. CPU-built on light/window change. */
  private presenceTex: Texture;
  private presenceMirror = new Uint32Array(0);
  private slotDist = new Float32Array(0);
  private presSig = "";
  private lightsVer = 0;

  /** Per-tile caster buckets — a **textile_tile map** (`RGBA32UI` = 8× `u16` prim indices/tile).
   *  Replaces the per-light LUT: the corridor sweep reads these instead. CPU-built on caster/window change. */
  private casterTex: Texture;
  private casterMirror = new Uint32Array(0);
  private casterCols = 0;
  private casterRows = 0;

  /** shadow_dirty — a **textile_tile map** (`R8UI`, nonzero = recompute) + per-slot owner tracking for toroidal
   *  persistence: a slot recomputes when its world-tile owner changes (pan) or the cold data rebuilds. */
  private dirtyTex: Texture;
  private dirtyMirror = new Uint8Array(0);
  private ownerCol = new Int32Array(0);
  private ownerRow = new Int32Array(0);
  private slotValid = new Uint8Array(0);
  private dirtyCols = 0;
  private dirtyRows = 0;
  /** Sticky "recompute every tile next build" — set on a cold rebuild; survives a window-not-ready frame. */
  private forceDirty = true;
  /** P6: walk the segment corridor (true) or the brute-force reach box (false). Brute is the
   *  validation baseline — `__corridor(false)` + `__shadowDiff()` must report 0 mismatches. */
  private corridor = true;
  /** P5 scoped dirty: world-tile rects `[x0,y0,x1,y1]` (inclusive) queued by a light MOVE — the union
   *  of the light's old + new reach boxes. Applied (∩ window) on top of the owner pass in
   *  {@link buildDirty}, then cleared. Correct because a moved light can only change presence/shadow
   *  inside its old ∪ new reach; everything outside keeps its persistent texel. */
  private pendingRects: [number, number, number, number][] = [];

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.gather = new Program(gl, FULLSCREEN_VERT, GATHER_FRAG, "shadow-gather");
    this.overlay = new Program(gl, OVERLAY_VERT, OVERLAY_FRAG, "shadow-overlay");
    this.gizmo = new Program(gl, GIZMO_VERT, GIZMO_FRAG, "shadow-gizmo");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    this.presenceTex = new Texture(gl, { width: 1, height: 1, format: "rgba32uint" });
    this.casterTex = new Texture(gl, { width: 1, height: 1, format: "rgba32uint" });
    this.dirtyTex = new Texture(gl, { width: 1, height: 1, format: "r8uint" });
    this.coldData = new ColdShadowData(renderer);
    (globalThis as unknown as { __cold: unknown }).__cold = this.coldData; // DEBUG
    // DEBUG: console toggle for the orbit (perf measurement) — `__orbit(false)` to freeze the lights.
    (globalThis as unknown as { __orbit: (on?: boolean) => boolean }).__orbit = (on?: boolean) => this.setOrbit(on);
    // DEBUG (P6): corridor↔brute toggle + the gather itself (for `debugReadShadow` diffing).
    (globalThis as unknown as { __corridor: (on?: boolean) => boolean }).__corridor = (on?: boolean) => this.setCorridor(on);
    (globalThis as unknown as { __gather: ShadowGather }).__gather = this;
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

  /** Seed 6 lights in a ring around a tile (world px). */
  seed(tileX: number, tileY: number): void {
    const cx = (tileX + 0.5) * SQUARE, cy = (tileY + 0.5) * SQUARE;
    this.seedX = cx; this.seedY = cy;
    this.lights.length = 0;
    for (let k = 0; k < MAX_LIGHTS; k++) {
      const a = (k / MAX_LIGHTS) * Math.PI * 2;
      // A single light sits AT the seed tile; multi-light debug still fans out on the ring.
      const radius = MAX_LIGHTS === 1 ? 0 : RING_RADIUS;
      // Vary emitter size per light so the per-light softness is visible: k·(source growing round the ring).
      const emitter = LIGHT_EMITTER * (1 + k * 0.6);
      this.lights.push({ x: cx + Math.cos(a) * radius, y: cy + Math.sin(a) * radius, z: LIGHT_Z, reach: LIGHT_REACH, emitterRadius: emitter });
    }
    this.enabled = true;
    this.coldDirty = true;
    this.lightsVer++; // invalidates light_presence_cold
    this.forceDirty = true; // lights changed → recompute every tile
  }

  /** Rebuild the per-tile **presence** (P5) when the lights or window change: each tile gets the
   *  **nearest 8** lights whose (reach−1, F10) circle covers it, as `u16` indices (`0xFFFF` = empty).
   *  Nearest-N eviction via `slotDist`. This bounds the gather to O(8) lights/texel. */
  private buildPresence(win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    const sig = `${winCol},${winRow},${cols},${rows},${this.lightsVer}`;
    if (sig === this.presSig && this.presenceTex.width === cols) return;
    this.presSig = sig;
    if (this.presenceTex.width !== cols || this.presenceTex.height !== rows) {
      this.presenceTex.destroy();
      this.presenceTex = new Texture(this.renderer.gl, { width: cols, height: rows, format: "rgba32uint" });
      this.presenceMirror = new Uint32Array(cols * rows * 4);
      this.slotDist = new Float32Array(cols * rows * 8);
    }
    this.presenceMirror.fill(0xffffffff); // every u16 slot = 0xFFFF (empty)
    this.slotDist.fill(Infinity);
    const pmod = (a: number, m: number): number => ((a % m) + m) % m;
    const n = Math.min(this.lights.length, MAX_LIGHTS);
    for (let k = 0; k < n; k++) {
      const L = this.lights[k], r = L.reach; // full reach (reach−1 F10 trim removed per request)
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
          const si = pmod(wr, rows) * cols + pmod(wc, cols);
          // nearest-8: replace the farthest slot (or an empty Infinity one) iff this light is closer.
          let worst = -1, worstD = d2;
          for (let s = 0; s < 8; s++) { const sd = this.slotDist[si * 8 + s]; if (sd > worstD) { worstD = sd; worst = s; } }
          if (worst < 0) continue; // all 8 slots already closer → drop this light for this tile
          this.slotDist[si * 8 + worst] = d2;
          const base = si * 4 + (worst >> 1);
          if ((worst & 1) === 0) this.presenceMirror[base] = ((this.presenceMirror[base] & 0x0000ffff) | ((k & 0xffff) << 16)) >>> 0;
          else this.presenceMirror[base] = ((this.presenceMirror[base] & 0xffff0000) | (k & 0xffff)) >>> 0;
        }
      }
    }
    this.presenceTex.upload(this.presenceMirror);
  }

  /** Rebuild the per-tile **caster buckets** (P1): allocate each standing caster's def+prim, then
   *  drop its `prim_data` index into every tile of its **base line** (anchor row × width cols) that
   *  the window covers, up to 8/tile. The corridor sweep reads these; casters are bucketed by where
   *  they *stand* (ground base), not the billboard's aerial bbox. Cheap enough to rebuild each frame
   *  for now; the O(1) re-bucket on move lands with the hot tier. */
  private buildCasters(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    if (this.casterCols !== cols || this.casterRows !== rows) {
      this.casterTex.destroy();
      this.casterTex = new Texture(this.renderer.gl, { width: cols, height: rows, format: "rgba32uint" });
      this.casterMirror = new Uint32Array(cols * rows * 4);
      this.casterCols = cols;
      this.casterRows = rows;
    }
    this.casterMirror.fill(0);
    const count = new Uint8Array(cols * rows);
    const pm = (a: number, m: number): number => ((a % m) + m) % m;
    // The card is TILTED back 65° (see the shader), so its ground footprint spans from the base
    // (anchor y) UP to the top (anchor y − 0.5·H·cos65). The corridor crosses the caster anywhere in
    // that y-range (far shadow ↔ top, near ↔ base), so we must bucket every row it spans — bucketing
    // only the base row missed casters whose top sits in the row above (I-7).
    const TILT = 0.5 * Math.cos(65 * Math.PI / 180); // 0.5·cos65 ≈ 0.211 of the height, leaned back
    this.litSeen.clear();
    for (const p of standing) {
      const def = this.coldData.definitionFor(p, resolver);
      if (def < 0) continue; // no textureName → not a caster
      const inst = this.coldData.primDataFor(p, def);
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
          const si = pm(wr, rows) * cols + pm(wc, cols);
          const n = count[si];
          if (n >= 8) continue; // tile full — drop the rest (rare)
          const base = si * 4 + (n >> 1);
          if ((n & 1) === 0) this.casterMirror[base] = ((this.casterMirror[base] & 0x0000ffff) | ((inst.idx & 0xffff) << 16)) >>> 0;
          else this.casterMirror[base] = ((this.casterMirror[base] & 0xffff0000) | (inst.idx & 0xffff)) >>> 0;
          count[si] = n + 1;
        }
      }
    }
    this.casterTex.upload(this.casterMirror);
    // A loose→tight def upgrade (surface resolved late) rewrites def/prim → re-dirty so the tighter
    // shadow recomputes (otherwise the dirty-gate keeps the stale loose shadow).
    // Upload changed def/prim textures. NO force-all: def swaps + new prims already queued
    // their scoped rects via the prim dirty cascade (markPrimChange).
    this.coldData.flush();
  }

  /** Queue the scoped dirty rect for a light move (P5): the union box of the old + new reach,
   *  +1 tile margin, in world tiles. Applied by {@link buildDirty} this frame. */
  private markLightMove(ox: number, oy: number, nx: number, ny: number, reach: number): void {
    const x0 = Math.floor((Math.min(ox, nx) - reach) / SQUARE) - 1;
    const y0 = Math.floor((Math.min(oy, ny) - reach) / SQUARE) - 1;
    const x1 = Math.floor((Math.max(ox, nx) + reach) / SQUARE) + 1;
    const y1 = Math.floor((Math.max(oy, ny) + reach) / SQUARE) + 1;
    this.pendingRects.push([x0, y0, x1, y1]);
  }

  /** The PRIM DIRTY CASCADE applied to texture/def changes: a prim changed (new immutable def —
   *  lod landed/zoomed — or first sight) → dirty the prim's own tiles, then every light whose reach
   *  touches them, then those lights' full cast regions (their reach boxes — a shadow texel is only
   *  written inside its light's presence, so the reach box bounds the cast). `x/y/w/h` = the prim's
   *  tight box in world px. `litSeen` dedupes light boxes within a frame (streaming floods). */
  private readonly litSeen = new Set<number>();
  private markPrimChange(x: number, y: number, w: number, h: number): void {
    const x0 = Math.floor(x / SQUARE) - 1, y0 = Math.floor(y / SQUARE) - 1;
    const x1 = Math.floor((x + w) / SQUARE) + 1, y1 = Math.floor((y + h) / SQUARE) + 1;
    this.pendingRects.push([x0, y0, x1, y1]);
    for (let k = 0; k < this.lights.length; k++) {
      if (this.litSeen.has(k)) continue;
      const L = this.lights[k];
      const cx = Math.min(Math.max(L.x, x), x + w), cy = Math.min(Math.max(L.y, y), y + h);
      if (Math.hypot(cx - L.x, cy - L.y) > L.reach + SQUARE) continue; // light can't see the prim
      this.litSeen.add(k);
      this.markLightMove(L.x, L.y, L.x, L.y, L.reach); // the light's whole cast region
    }
  }

  /** Rebuild `shadow_dirty` for this frame: a slot is dirty when its world-tile **owner changed** (pan /
   *  resize), `forceAll` (the cold data rebuilt — lights/casters changed), or it falls in a queued
   *  light-move rect (P5 scoped dirty). Clean slots `discard` in the gather, so `shadow-cold` persists
   *  (F6). Mirrors `SquareCache.markStale`'s per-slot owner tracking. */
  private buildDirty(win: TileWindow): void {
    const { cols, rows, winCol, winRow } = win;
    let forceAll = this.forceDirty;
    this.forceDirty = false;
    if (this.dirtyCols !== cols || this.dirtyRows !== rows) {
      this.dirtyTex.destroy();
      this.dirtyTex = new Texture(this.renderer.gl, { width: cols, height: rows, format: "r8uint" });
      this.dirtyMirror = new Uint8Array(cols * rows);
      this.ownerCol = new Int32Array(cols * rows);
      this.ownerRow = new Int32Array(cols * rows);
      this.slotValid = new Uint8Array(cols * rows);
      this.dirtyCols = cols;
      this.dirtyRows = rows;
      forceAll = true;
    }
    const pm = (a: number, m: number): number => ((a % m) + m) % m;
    const baseC = pm(winCol, cols), baseR = pm(winRow, rows);
    this.dirtyMirror.fill(0);
    for (let sy = 0; sy < rows; sy++) {
      const wr = winRow + pm(sy - baseR, rows);
      for (let sx = 0; sx < cols; sx++) {
        const wc = winCol + pm(sx - baseC, cols);
        const si = sy * cols + sx;
        if (forceAll || this.slotValid[si] === 0 || this.ownerCol[si] !== wc || this.ownerRow[si] !== wr) {
          this.dirtyMirror[si] = 1;
          this.ownerCol[si] = wc;
          this.ownerRow[si] = wr;
          this.slotValid[si] = 1;
        }
      }
    }
    // P5 scoped dirty: mark every window tile inside a queued light-move rect.
    for (const [x0, y0, x1, y1] of this.pendingRects) {
      const c0 = Math.max(x0, winCol), c1 = Math.min(x1, winCol + cols - 1);
      const r0 = Math.max(y0, winRow), r1 = Math.min(y1, winRow + rows - 1);
      for (let wr = r0; wr <= r1; wr++)
        for (let wc = c0; wc <= c1; wc++) this.dirtyMirror[pm(wr, rows) * cols + pm(wc, cols)] = 1;
    }
    this.pendingRects.length = 0;
    this.dirtyTex.upload(this.dirtyMirror);
  }

  get on(): boolean {
    return this.enabled;
  }
  /** DEBUG: turn the light orbit on/off (no arg = toggle). Off freezes the lights so the scene is
   *  static again (gather goes idle via dirty-gating); on drives a full recompute every frame. */
  setOrbit(on?: boolean): boolean {
    this.orbit = on ?? !this.orbit;
    if (!this.orbit) this.forceDirty = true; // one last clean recompute at the frozen positions
    return this.orbit;
  }
  /** P6: switch corridor ↔ brute walk (no arg = toggle) + recompute everything under the new path. */
  setCorridor(on?: boolean): boolean {
    this.corridor = on ?? !this.corridor;
    this.forceDirty = true;
    return this.corridor;
  }
  /** DEBUG (P6): read the shadow-cold RT back (RGBA32UI) — the corridor↔brute identity diff. */
  debugReadShadow(): Uint32Array | null {
    if (!this.shadowRT) return null;
    const gl = this.renderer.gl;
    const out = new Uint32Array(this.shadowRT.width * this.shadowRT.height * 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadowRT.fbo);
    gl.readPixels(0, 0, this.shadowRT.width, this.shadowRT.height, gl.RGBA_INTEGER, gl.UNSIGNED_INT, out);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return out;
  }
  toggle(): boolean {
    this.enabled = !this.enabled;
    return this.enabled;
  }

  private ensureRT(cols: number, rows: number): void {
    if (this.shadowRT && cols === this.rtCols && rows === this.rtRows) return;
    this.shadowRT?.destroy();
    this.shadowRT = new RenderTarget(this.renderer.gl, {
      width: Math.max(1, cols * TEXTILE_UNIT), height: Math.max(1, rows * TEXTILE_UNIT), formats: ["rgba32uint"],
    });
    this.shadowRT.clearInt(0, 0, 0, 0); // start persistence from a known-zero buffer (P3)
    this.rtCols = cols;
    this.rtRows = rows;
  }

  /** Rebuild the cold data (on change) + recompute the DIRTY tiles of the shadow-cold bitfield. */
  tick(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    if (!this.enabled || this.lights.length === 0) return;

    // DEBUG orbit: rotate the ring each frame so the whole shadow field recomputes every frame — the
    // FPS panel then reads the real per-frame gather cost (the moving-light / P8 case). Dial MAX_LIGHTS
    // to see light-count scaling; the per-tile 8-cap means cost only rises where >8 lights overlap.
    if (this.orbit) {
      this.orbitPhase += 0.01;
      const n = this.lights.length;
      for (let k = 0; k < n; k++) {
        const a = (k / n) * Math.PI * 2 + this.orbitPhase;
        const L = this.lights[k], ox = L.x, oy = L.y;
        L.x = this.seedX + Math.cos(a) * RING_RADIUS;
        L.y = this.seedY + Math.sin(a) * RING_RADIUS;
        this.markLightMove(ox, oy, L.x, L.y, L.reach); // P5 scoped dirty — NOT force-all
      }
      this.coldDirty = true;   // light records changed
      this.lightsVer++;        // presence changed
    }
    if (this.coldDirty || standing.length !== this.lastCasterCount) {
      const coldLights = this.lights.slice(0, MAX_LIGHTS).map((L, k) => ({
        x: L.x, y: L.y, z: L.z, reach: L.reach, emitterRadius: L.emitterRadius,
        color: LIGHT_COLORS[k % LIGHT_COLORS.length], intensity: 1, castShadows: true,
      }));
      this.coldData.buildLights(coldLights);
      const removed = standing.length < this.lastCasterCount;
      const lightsChanged = this.coldDirty;
      this.lastCasterCount = standing.length;
      this.coldDirty = false;
      // Scoped dirty: a light MOVE queued its own rects; NEW casters cascade per prim
      // (markPrimChange on first sight). Force-all only for caster REMOVAL (stale shadows with no
      // owner to cascade from) or a light change with no scoped rects (e.g. a re-seed).
      if (removed || (lightsChanged && this.pendingRects.length === 0)) this.forceDirty = true;
    }
    if (this.coldData.lights === 0 || win.cols === 0) return;

    this.ensureRT(win.cols, win.rows);
    this.buildPresence(win);                    // F5 per-tile light cull (rebuilt on light/window change)
    this.buildCasters(standing, resolver, win); // P1 per-tile caster buckets (the corridor reads these)
    this.buildDirty(win);    // P3: owner-change (pan) + rebuild → dirty; clean tiles persist
    // Single pass; each fragment `discard`s clean tiles (shadow-cold persists) — no clear, no ping-pong.
    this.renderer.draw({
      program: this.gather,
      geometry: this.fsQuad,
      target: this.shadowRT!,
      blend: "none",
      textures: {
        uData: this.coldData.dataTexture,
        uPresence: this.presenceTex,
        uCaster: this.casterTex,
        uDirty: this.dirtyTex,
        uSurface: this.coldData.surfacePage ?? this.empty, // no page yet → defs have no frame → solid quads
      },
      uniforms: (p) => {
        p.uInt("uCols", win.cols);
        p.uInt("uRows", win.rows);
        p.uInt("uWinCol", win.winCol);
        p.uInt("uWinRow", win.winRow);
        p.uInt("uSlot", TEXTILE_UNIT);
        p.uInt("uCorridor", this.corridor ? 1 : 0);
      },
    });
  }

  /** Decode shadow-cold over the world (debug `/overlayRT shadow-cold`). */
  drawOverlay(camera: Camera, win: TileWindow): void {
    if (!this.shadowRT || win.cols === 0) return;
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
      textures: { uShadow: this.shadowRT.textures[0], uPresence: this.presenceTex },
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
    this.overlay.destroy();
    this.gizmo.destroy();
    this.empty.destroy();
    this.presenceTex.destroy();
    this.casterTex.destroy();
    this.dirtyTex.destroy();
    this.shadowRT?.destroy();
    this.coldData.destroy();
  }
}
