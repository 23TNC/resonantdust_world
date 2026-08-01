//! The lighting pass (lighting-rework P3) — per-light slots, and the summed map the display reads.
//!
//! Two maps, because the write side and the read side want opposite things ([F1](forks.md#f1)):
//!
//!   · **slots** — `LIGHT_SLOTS` px per texel, one light each. Makes removal EXACT: clear the slot,
//!     no subtraction, no quantisation. This is what the old design could not do — nothing stored a
//!     light's own contribution, so `DIFFERENTIAL_WIRED` stayed false and 4 MiB of prev-buffers idled
//!     from creation to deletion.
//!   · **sum** — one texel per texel. The display samples ONE texture instead of adding 8 back
//!     together per screen pixel (~24 M fetches/frame at 2560×1172, purely to undo the split).
//!
//! Updating the sum is one blended draw of `new − old`, and `old` is free because it is sitting in
//! the slot. That is the whole trick.
//!
//! **One draw for all 8 lights** ([F3](forks.md#f3)). The slot map is `LIGHT_SLOTS`× wider than the
//! lighting grid, so a fragment derives its light from `x & 7` and touches exactly one slot. No MRT:
//! it hung Chrome twice in the previous stream and was never root-caused (strip I1).

import { Program, Geometry, RenderTarget, type Renderer, type Texture } from "../../gl";
import { LIGHT_SLOTS, LIGHT_SCALE, TILE_SLOTS, LIGHT_LANES_GLSL, OCCLUSION_GLSL } from "./records";
import { SQUARE, UNITS_PER_TILE, SLOTS_X, SLOTS_Y, TEXTILE_LIGHT } from "./squareMath";

/** Lighting texels per tile — the pinned lighting resolution, independent of the art dial. */
const LIGHT_TEXELS = TEXTILE_LIGHT;
/** The lighting grid, in texels. Fixed: it rides the slot grid, so zoom reprojects rather than resizes. */
const LIGHT_W = SLOTS_X * LIGHT_TEXELS;
const LIGHT_H = SLOTS_Y * LIGHT_TEXELS;

const FULLSCREEN_VERT = /* glsl */ `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

/** Writes ONE light per fragment into its own slot column. */
const SLOT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform sampler2D uSurfaceAtlas;   // the shared co-packed page — the SILHOUETTE lives here
uniform sampler2D uSurfaceAtlas2;  // P3: the second page (defs carry their page in anchor.x)
uniform sampler2D uNormalCold;     // lighting-visual P2: the BAKED normal composites — cold tier
uniform sampler2D uNormalWarm;     //   warm tier (movers), slot-aligned with cold
uniform sampler2D uSurfaceWarm;    //   warm surface: B = warm coverage, the warm-over-cold key
uniform int uSlotCols;             // the display torus: slots per axis (tiles) ...
uniform int uSlotRows;
uniform int uSlotPx;               // ... and texels per slot at the CURRENT partition; 0 = unbound
uniform highp usampler2D uPrim;      // prim_data   — one px per prim
uniform highp usampler2D uLight;     // light       — 8 u16 prim indices per TILE
uniform highp usampler2D uDef;       // definition_data — the caster's card
uniform highp usampler2D uShadow;    // the gather's output: WHICH caster occludes, per unit per light
uniform highp usampler2D uReceiver;  // P3: which receiver owns each lighting texel (the receiver map)
uniform int uRefine;                 // 0 = no shadows, 1 = refine at THIS resolution (F12)
uniform int uDebugGate;              // 1 = emit the gate's selectivity instead of light
uniform int uLightW;                 // lighting texels per TILE (TEXTILE_LIGHT >> lod)
uniform int uUnitT;                  // shadow texels per TILE (TEXTILE_UNIT >> lod)
uniform int uCols;                   // window extent in TILES — the slot torus modulus
uniform int uRows;
uniform vec2 uWindowOrigin;          // world tile of the window's start (for the unwrap)
uniform int uMapW;                   // one slot block's width in texels
out vec4 fragColor;

${LIGHT_LANES_GLSL}

const int   LIGHT_SLOTS = ${LIGHT_SLOTS};
const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
const float UPT         = ${UNITS_PER_TILE}.0;
const int   TILE_DIM    = 256;

int pmod(int a, int m) { return ((a % m) + m) % m; }

uvec4 fetchPrim(uint idx) {
  return texelFetch(uPrim, ivec2(int(idx) & 1023, int(idx) >> 10), 0);
}
uvec4 fetchDef(uint block, uint rot) {
  uint px = block * 16u + rot;
  return texelFetch(uDef, ivec2(int(px) & 1023, int(px) >> 10), 0);
}
uint shadowSlot(uvec4 v, int i) {
  uint w = i < 2 ? v.x : (i < 4 ? v.y : (i < 6 ? v.z : v.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}
${OCCLUSION_GLSL}

// lighting-visual P2: the normal at world point P, from the BAKED normal composites -- the same
// screen-space image the blit resolves pixels from, warm-over-cold by warm coverage (the blit's
// rule). The composite is addressed by the display torus (slot = tile mod cols/rows, +1 apron
// slot, uSlotPx texels per tile at the partition the CACHE chose -- passed in, never recomputed:
// shadows-on-prims taught that slotPx is zoom-chosen). Sampling at the texel's OWN position means
// whatever is DRAWN there owns the normal, so tiles with authored maps, the analytic wall
// normals, per-material detail and mover facings are all inherited from the bake for free.
vec3 sceneNormalAt(vec2 Pu) {
  if (uSlotPx <= 0) { return vec3(0.0, 0.0, 1.0); }          // composites unbound (debug paths)
  vec2 tile = Pu / UPT;
  int tX = int(floor(tile.x)), tY = int(floor(tile.y));
  vec2 f = tile - vec2(float(tX), float(tY));
  vec2 texel = (vec2(float(pmod(tX, uSlotCols) + 1), float(pmod(tY, uSlotRows) + 1)) + f)
             * float(uSlotPx);
  // warm is slot-aligned with cold (the blit samples both at ONE uv) -- one uv serves all three
  vec2 uv = texel / vec2(textureSize(uNormalCold, 0));
  float wcov = texture(uSurfaceWarm, uv).b;
  vec3 n = wcov > 0.5 ? texture(uNormalWarm, uv).rgb : texture(uNormalCold, uv).rgb;
  return normalize(n * 2.0 - 1.0);
}

// THE REFINE (F12): re-test the STORED caster identity at THIS texel -- 64/tile against the
// gather's 16/tile -- through the SAME shared occlusion solve the gather runs (P3: one
// definition, no drift), with the lit point possibly ELEVATED (a billboard receiver).
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  // F3: the slot map is LIGHT_SLOTS x wider, so x carries BOTH the texel and which light it is.
  // BLOCKED, not interleaved: slot s owns x in [s*W, (s+1)*W). A slot is then a RECTANGLE, which is
  // what lets the delta path scissor to one light without touching the other seven.
  int slot = fc.x / uMapW;
  int tx   = fc.x % uMapW;
  int ty   = fc.y;

  // texel -> world position, in units. lighting-visual P4: the map is on the SLOT TORUS (the
  // composites' machinery — a tile's texel block sits at mod(tile, cols) · uLightW, texture size
  // fixed, texels-per-tile halving per lod). The UNWRAP recovers the unique window tile with this
  // residue, exactly as fillDisplay assigns it.
  int tileX = int(uWindowOrigin.x) + pmod(tx / uLightW - int(uWindowOrigin.x), uCols);
  int tileY = int(uWindowOrigin.y) + pmod(ty / uLightW - int(uWindowOrigin.y), uRows);
  vec2 inTile = (vec2(float(tx % uLightW), float(ty % uLightW)) + 0.5) / float(uLightW);
  vec2 P = (vec2(float(tileX), float(tileY)) + inTile) * UPT;   // world UNITS

  // this tile's light set — 8 u16 prim indices, 2 per channel
  uvec4 L = texelFetch(uLight, ivec2(pmod(tileX, TILE_DIM), pmod(tileY, TILE_DIM)), 0);
  uint packed = slot < 2 ? L.x : (slot < 4 ? L.y : (slot < 6 ? L.z : L.w));
  uint idx = (slot & 1) == 0 ? (packed >> 16) : (packed & 0xffffu);

  // index 0 is the global sentinel — an empty slot contributes nothing (F8)
  if (idx == 0u) { fragColor = vec4(0.0); return; }

  uvec4 rec = fetchPrim(idx);
  // F9: a u16 index can be RECYCLED. Verify the record still emits before trusting the slot —
  // one mask against a word already fetched, so the check is effectively free.
  uint emitType = (rec.z >> 26) & 3u;
  if (emitType == 0u) { fragColor = vec4(0.0); return; }

  vec2  Lpos      = vec2(float(rec.x >> 16), float(rec.x & 0xffffu));   // unit.x | unit.y
  uint  intensity = intensityFromB(rec.z);
  float reach     = reachUnitsFromB(rec.z);
  if (intensity == 0u) { fragColor = vec4(0.0); return; }

  // P3: the RESOLVED receiver. A billboard texel is lit at its CARD's plan position with its
  // height up the card, and shadow-tested to that ELEVATED point -- the climbing shadow. The
  // ground (receiver 0) keeps the texel's own position at height 0.
  vec2 Puse = P;
  float targetH = 0.0;
  uint recvIdx = texelFetch(uReceiver, ivec2(fc.x % uMapW, fc.y), 0).r;
  if (recvIdx != 0u) {
    uvec4 rrec = fetchPrim(recvIdx);
    if (((rrec.z >> 28) & 3u) != 0u) {
      float baseY = float(rrec.x & 0xffffu);
      targetH = max(0.0, baseY - P.y);
      Puse = vec2(P.x, baseY);
    }
  }

  float d = distance(Puse, Lpos);
  if (d >= reach) { fragColor = vec4(0.0); return; }

  // Falloff within the stored reach — reach-scaled, zero AT the boundary (P4, shared).
  float I  = float(intensity) / INTENSITY_MAX * LIGHT_SCALE;   // the lane spans the 0..4 overbright range
  float at = lightFalloff(d, reach, I);

  // P5: per-light N x L (the FINE-lightmap model — shading bakes into the slot, the summed map
  // inherits it, the display stays one fetch). lighting-visual P2: N is sampled from the baked
  // normal composites at the texel's OWN position P -- the composite is screen-space, so the
  // drawn pixel standing there (card or ground) IS the surface being lit. Only the light
  // DIRECTION's frame branches on receiver-ness. A wrap floor keeps backsides readable.
  float Lz2 = float(rec.y >> 24);
  vec3 nrm = sceneNormalAt(P);
  float ndotl;
  if (recvIdx != 0u) {
    // sprite frame: x right, y up the card, z toward the viewer (world south)
    vec3 ldir = normalize(vec3(Lpos.x - Puse.x, Lz2 - targetH, Puse.y - Lpos.y));
    ndotl = clamp(dot(nrm, ldir), 0.0, 1.0);
  } else {
    // ground frame: x right, y north (up-screen), z up -- the flat-up fallback decodes to
    // (0,0,1), reproducing the old overhead/grazing falloff exactly; authored tile normals tilt it
    vec3 ldir = normalize(vec3(Lpos.x - P.x, P.y - Lpos.y, Lz2));
    ndotl = clamp(dot(nrm, ldir), 0.0, 1.0);
  }
  at *= mix(0.25, 1.0, ndotl);

  // Emitters carry their DSL light RGB in colour.1-3 (lighting-correctness P1b — the torch's
  // authored colour); an all-zero RGB (the debug lights) falls back to the warmth ramp on .4.
  vec3 tint = vec3(float(rec.w >> 24), float((rec.w >> 16) & 0xffu), float((rec.w >> 8) & 0xffu)) / 255.0;
  if (tint == vec3(0.0)) {
    float c4 = float(rec.w & 0xffu) / 255.0;
    tint = vec3(1.0, 0.85 + 0.15 * c4, 0.6 + 0.4 * c4);
  }

  // F5 GATE: only a texel whose UNIT holds a caster for this light does any refine work. Interior
  // and fully-lit texels do no fetch and no test -- the gate is the whole reason this is affordable.
  bool gated = false, occluded = false;
  if (uRefine == 1) {
    // P4: the shadow buffer rides the SAME slot torus at uUnitT texels/tile. The lit point can
    // sit south of the fragment's own tile (a billboard's base), so guard the window bounds.
    int utx = int(floor(Puse.x / UPT)), uty = int(floor(Puse.y / UPT));
    if (utx >= int(uWindowOrigin.x) && utx < int(uWindowOrigin.x) + uCols &&
        uty >= int(uWindowOrigin.y) && uty < int(uWindowOrigin.y) + uRows) {
      int ux = pmod(utx, uCols) * uUnitT + int((Puse.x - float(utx) * UPT) * float(uUnitT) / UPT);
      int uy = pmod(uty, uRows) * uUnitT + int((Puse.y - float(uty) * UPT) * float(uUnitT) / UPT);
      uvec4 sh = texelFetch(uShadow, ivec2(ux * 3, uy), 0);   // px 0 = ground casters
      uint caster = shadowSlot(sh, slot);
      if (caster != 0u) {
        gated = true;
        occluded = occludesAt(caster, Lpos, float(rec.y >> 24), Puse, targetH);
      }
    }
  }
  if (uDebugGate == 1) { fragColor = vec4(gated ? 1.0 : 0.0, occluded ? 1.0 : 0.0, 1.0, 1.0); return; }
  if (occluded) { fragColor = vec4(0.0); return; }
  // F2: stored at 1/LIGHT_SCALE so 4x overbright survives a 0..1 fixed-point format.
  // NaN/Inf guard (P6): clamp() is UNDEFINED on NaN, so a single bad record could otherwise write a
  // NaN into a slot -- and the delta path would then blend it into the summed map, where it poisons
  // every later add and cannot be withdrawn. Compare against itself first: NaN != NaN.
  vec3 v = tint * at / LIGHT_SCALE;
  if (!(v.r == v.r) || !(v.g == v.g) || !(v.b == v.b)) { fragColor = vec4(0.0); return; }
  fragColor = vec4(clamp(v, 0.0, 1.0), 1.0);
}
`;

/** Sums the slots into the map the display reads — one fetch per screen pixel instead of eight. */
const SUM_FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uSlots;
uniform int uMapW;
out vec4 fragColor;
const int LIGHT_SLOTS = ${LIGHT_SLOTS};
const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  vec3 acc = vec3(0.0);
  for (int s = 0; s < LIGHT_SLOTS; s++) {
    acc += floor(texelFetch(uSlots, ivec2(s * uMapW + fc.x, fc.y), 0).rgb * 1023.0 + 0.5);
  }
  fragColor = vec4(acc, 1.0);   // QUANTISATION LEVELS, matching the delta path's alphabet
}
`;

/** The DELTA pass ([F1](forks.md#f1)) — the whole point of storing per-light slots.
 *
 *  Emits `new - old` for ONE slot, blended `ONE, ONE` into the sum. `old` is free because it is
 *  sitting in the slot, which is exactly what the previous design could not do: nothing stored a
 *  light's own contribution, so `DIFFERENTIAL_WIRED` stayed false and its prev-buffers idled unused
 *  from creation to deletion.
 *
 *  Removal is `new = 0`, so the delta is `-old` and the sum lands back where it started — no
 *  subtract-the-light bookkeeping, no quantisation scheme to make cancellation exact. */
const DELTA_FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uSlots;
uniform int uMapW;
uniform int uSlot;
uniform float uSign;          // -1 to withdraw the slot's current value, +1 to deposit it
out vec4 fragColor;
const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  vec3 v = texelFetch(uSlots, ivec2(uSlot * uMapW + fc.x, fc.y), 0).rgb;
  // Deposit the slot's QUANTISATION LEVEL (an integer 0..1023), not its float value.
  //
  // FP32 represents every integer below 2^24 exactly, so a deposit and its later withdrawal cancel
  // BIT-EXACTLY in any order. Depositing the float instead left one ULP behind (measured 5.96e-8 =
  // 2^-24) because a + (-x) + x re-rounds. 8 slots x 1023 levels = 8184, nowhere near 2^24.
  //
  // This is the old accumulator's LIGHT_QUANT, and F1 was WRONG to say the constraint had no reason
  // to exist. It has none for the SLOTS -- those are overwritten, never accumulated. The SUM is
  // still an accumulator, and an accumulator that must invert still needs an exact alphabet.
  fragColor = vec4(floor(v * 1023.0 + 0.5) * uSign, 0.0);
}
`;

/** Writes ONE slot's new value (no blending) — the second half of a delta update. */
const SLOT_ONE_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uPrim;
uniform int uLightW;
uniform int uMapW;
uniform int uCols;        // P4: the slot torus modulus (window tiles)
uniform int uRows;
uniform int uPrimIndex;
uniform vec2 uWindowOrigin;
out vec4 fragColor;

${LIGHT_LANES_GLSL}

const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
const float UPT = ${UNITS_PER_TILE}.0;

int pmod(int a, int m) { return ((a % m) + m) % m; }
uvec4 fetchPrim(uint idx) { return texelFetch(uPrim, ivec2(int(idx) & 1023, int(idx) >> 10), 0); }

void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  // This draw is SCISSORED to one slot's block, so gl_FragCoord.x carries the block offset
  // (slot*uMapW + tx). Strip it: the world position depends on the texel, not on which slot owns it.
  int tx = fc.x % uMapW;
  // P4: slot-torus unwrap — same rule as the full slot pass.
  int tileX = int(uWindowOrigin.x) + pmod(tx / uLightW - int(uWindowOrigin.x), uCols);
  int tileY = int(uWindowOrigin.y) + pmod(fc.y / uLightW - int(uWindowOrigin.y), uRows);
  vec2 inTile = (vec2(float(tx % uLightW), float(fc.y % uLightW)) + 0.5) / float(uLightW);
  vec2 P = (vec2(float(tileX), float(tileY)) + inTile) * UPT;

  if (uPrimIndex == 0) { fragColor = vec4(0.0); return; }
  uvec4 rec = fetchPrim(uint(uPrimIndex));
  if (((rec.z >> 26) & 3u) == 0u) { fragColor = vec4(0.0); return; }
  vec2  Lpos      = vec2(float(rec.x >> 16), float(rec.x & 0xffffu));
  uint  intensity = intensityFromB(rec.z);
  float reach     = reachUnitsFromB(rec.z);
  float d = distance(P, Lpos);
  if (intensity == 0u || d >= reach) { fragColor = vec4(0.0); return; }
  float I  = float(intensity) / INTENSITY_MAX * LIGHT_SCALE;   // the lane spans the 0..4 overbright range
  float at = lightFalloff(d, reach, I);
  // Emitters carry their DSL light RGB in colour.1-3 (lighting-correctness P1b — the torch's
  // authored colour); an all-zero RGB (the debug lights) falls back to the warmth ramp on .4.
  vec3 tint = vec3(float(rec.w >> 24), float((rec.w >> 16) & 0xffu), float((rec.w >> 8) & 0xffu)) / 255.0;
  if (tint == vec3(0.0)) {
    float c4 = float(rec.w & 0xffu) / 255.0;
    tint = vec3(1.0, 0.85 + 0.15 * c4, 0.6 + 0.4 * c4);
  }
  fragColor = vec4(clamp(tint * at / LIGHT_SCALE, 0.0, 1.0), 1.0);
}
`;

/** The RECEIVER map (lighting-rework P5) — which surface owns each lighting texel, computed ONCE.
 *
 *  [I2](issues.md#i2): the design selects the receiver *inside* the light loop and `break`s on the
 *  stored pair BEFORE testing coverage, so a texel covered only by `r6` can resolve to `r7` for one
 *  light and `r6` for another — the same texel lit as two different surfaces, which renders as a
 *  pixel missing one light's contribution.
 *
 *  Coverage is **geometry**: it cannot depend on which light is being evaluated. So it is tested
 *  first, and — being light-independent — it is computed once per texel here rather than up to 8
 *  times inside the loop. `presence` is layer-sorted, so walking 7..1 and taking the first hit is
 *  "topmost wins" by construction. */
const RECEIVER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform sampler2D uSurfaceAtlas;   // P3: coverage is the SILHOUETTE, not the box
uniform sampler2D uSurfaceAtlas2;
uniform highp usampler2D uPrim;
uniform highp usampler2D uDef;
uniform highp usampler2D uPresence;
uniform vec2 uWindowOrigin;
uniform int uLightW;
uniform int uCols;        // P4: the slot torus modulus (window tiles)
uniform int uRows;
uniform int uDilateY;
out uvec4 fragColor;

const float UPT = ${UNITS_PER_TILE}.0;
const int   TILE_DIM = 256;

int pmod(int a, int m) { return ((a % m) + m) % m; }
uvec4 fetchPrim(uint i) { return texelFetch(uPrim, ivec2(int(i) & 1023, int(i) >> 10), 0); }
uvec4 fetchDef(uint b, uint r) { uint px = b * 16u + r; return texelFetch(uDef, ivec2(int(px) & 1023, int(px) >> 10), 0); }
uint slotOf(uvec4 v, int i) {
  uint w = i < 2 ? v.x : (i < 4 ? v.y : (i < 6 ? v.z : v.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}

${OCCLUSION_GLSL}

// Does receiver r's SPRITE cover world point P? (P3 — the box alone lit ground pixels inside a
// billboard's rectangle as if they were the card: the slab artifact.) Frame-anchored placement,
// west mirrored at sample, then the silhouette decides.
bool covers(uint r, vec2 P) {
  if (r == 0u) return false;
  uvec4 rec = fetchPrim(r);
  if (((rec.z >> 28) & 3u) == 0u) return false;              // receive_type 0 -- not a receiver
  vec2 C = vec2(float(rec.x >> 16), float(rec.x & 0xffffu));
  uint prot = (rec.z >> 10) & 0xfu;
  uvec4 d = fetchDef(rec.y & 0xffffu, prot);
  int subXi = int(d.y >> 24);
  int subWi = int((d.y >> 8) & 0xffu) + 1, subHi = int(d.y & 0xffu) + 1;
  int spanI = int((d.x >> 4) & 0xfu) + 1;
  float fu = float(spanI * 16);
  // P1: bottom-aligned — the receiver's card occupies [0, subH] from its base.
  float hTop = float(subHi);
  float h = C.y - P.y;
  if (h < 0.0 || h > hTop) return false;
  float left = prot == 3u ? C.x + fu * 0.5 - float(subXi + subWi) : C.x - fu * 0.5 + float(subXi);
  if (P.x < left || P.x > left + float(subWi)) return false;
  float frac = (P.x - left) / float(subWi);
  if (prot == 3u) { frac = 1.0 - frac; }
  return silhouetteHit(d, frac, (hTop - h) / float(subHi), false);
}

void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  // P4: slot-torus unwrap — same rule as the slot pass.
  int tileX = int(uWindowOrigin.x) + pmod(fc.x / uLightW - int(uWindowOrigin.x), uCols);
  int tileY = int(uWindowOrigin.y) + pmod(fc.y / uLightW - int(uWindowOrigin.y), uRows);
  vec2 inTile = (vec2(float(fc.x % uLightW), float(fc.y % uLightW)) + 0.5) / float(uLightW);
  vec2 P = (vec2(float(tileX), float(tileY)) + inTile) * UPT;

  // VERTICAL OVERHANG. A prim registers in the tile of its BASE, but its card rises NORTH from
  // there -- so a texel standing on the card sits in a tile whose presence does not list it. Measured
  // directly: prim 15's base is at unit y 672, the top edge of tile row 42, and its 12-unit card
  // covers rows 41..42; a texel at y 665 reads row 41 and finds nothing.
  //
  // So scan the texel's own row and the rows SOUTH of it, far enough to cover the tallest card. Only
  // south, and only in y: the card rises, so the tiles that can own this texel are at or below it.
  // Same shape as the caster walk's x-dilation, one axis over.
  for (int dy = 0; dy <= uDilateY; dy++) {
    uvec4 pres = texelFetch(uPresence, ivec2(pmod(tileX, TILE_DIM), pmod(tileY + dy, TILE_DIM)), 0);
    // TOPMOST FIRST -- presence is layer-sorted, so 7..1 is descending. COVERAGE decides; the stored
    // pair is never consulted here, which is what makes the answer the same for every light.
    for (int k = 7; k >= 1; k--) {
      uint cand = slotOf(pres, k);
      if (covers(cand, P)) { fragColor = uvec4(cand, 0u, 0u, 0u); return; }
    }
  }
  fragColor = uvec4(0u);                                     // the ground (presence slot 0)
}
`;

export class LightPass {
  readonly slotRT: RenderTarget;
  readonly sumRT: RenderTarget;
  /** Which receiver owns each lighting texel — computed once, read by all 8 light fragments. */
  readonly receiverRT: RenderTarget;
  private readonly slotProg: Program;
  private readonly sumProg: Program;
  private readonly deltaProg: Program;
  private readonly slotOneProg: Program;
  private readonly recvProg: Program;
  private readonly quad: Geometry;

  constructor(private readonly gl: WebGL2RenderingContext) {
    // 8x wide: one column per light, so ONE draw covers every slot (F3).
    this.slotRT = new RenderTarget(gl, {
      width: LIGHT_W * LIGHT_SLOTS, height: LIGHT_H, formats: ["rgb10a2unorm"],
    });
    // RGBA32F, not 16F. The exactness acceptance (add a light, remove it, land bit-identically)
    // needs the sum to represent every value a slot can produce WITHOUT rounding: 8 slots x 1024
    // quantisation levels needs 13 mantissa bits, and FP16 has 11. FP32's 24 covers it with room.
    // Costs 32 MiB against 16F's 8 -- see the stream's D2.
    this.sumRT = new RenderTarget(gl, { width: LIGHT_W, height: LIGHT_H, formats: ["rgba32float"] });
    this.receiverRT = new RenderTarget(gl, { width: LIGHT_W, height: LIGHT_H, formats: ["r32uint"] });
    this.slotProg = new Program(gl, FULLSCREEN_VERT, SLOT_FRAG, "light-slots");
    this.sumProg = new Program(gl, FULLSCREEN_VERT, SUM_FRAG, "light-sum");
    this.deltaProg = new Program(gl, FULLSCREEN_VERT, DELTA_FRAG, "light-delta");
    this.slotOneProg = new Program(gl, FULLSCREEN_VERT, SLOT_ONE_FRAG, "light-slot-one");
    this.recvProg = new Program(gl, FULLSCREEN_VERT, RECEIVER_FRAG, "light-receiver");
    this.quad = new Geometry(gl, this.slotProg, {
      aPos: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
  }

  /** Compute the receiver map — ONE draw, light-independent, read by all 8 light fragments. */
  receivers(renderer: Renderer, prim: Texture, def: Texture, presence: Texture, atlas: Texture,
            originTileX: number, originTileY: number, atlas2?: Texture,
            win?: { cols: number; rows: number; lod: number }): void {
    const lightW = LIGHT_TEXELS >> (win?.lod ?? 0);
    renderer.draw({
      program: this.recvProg, geometry: this.quad, target: this.receiverRT, blend: "none",
      textures: { uPrim: prim, uDef: def, uPresence: presence, uSurfaceAtlas: atlas, uSurfaceAtlas2: atlas2 ?? atlas },
      uniforms: (p) => {
        p.uInt("uLightW", lightW);
        p.uInt("uCols", win?.cols ?? SLOTS_X);
        p.uInt("uRows", win?.rows ?? SLOTS_Y);
        p.uInt("uDilateY", 2);          // tallest authored card, in tiles; content-derived later
        p.uVec2("uWindowOrigin", originTileX, originTileY);
      },
    });
  }

  /** Recompute every slot (ONE draw), then re-sum. */
  run(renderer: Renderer, prim: Texture, light: Texture,
      originTileX: number, originTileY: number,
      opt: { def?: Texture; shadow?: Texture; atlas?: Texture; atlas2?: Texture; refine?: boolean;
             debugGate?: boolean;
             // P4: the window's torus extent in TILES + the lod (texels-per-tile halve per step).
             win?: { cols: number; rows: number; lod: number };
             // lighting-visual P2: the baked normal composites + the display torus that addresses
             // them. slotPx is the CACHE'S current partition — never recomputed here; 0 = unbound
             // (the shader falls back to flat-up).
             normalCold?: Texture; normalWarm?: Texture; surfaceWarm?: Texture;
             slotCols?: number; slotRows?: number; slotPx?: number } = {}): void {
    const lod = opt.win?.lod ?? 0;
    renderer.draw({
      program: this.slotProg, geometry: this.quad, target: this.slotRT, blend: "none",
      textures: { uPrim: prim, uLight: light,
                  uDef: opt.def ?? prim, uShadow: opt.shadow ?? prim,
                  uSurfaceAtlas: opt.atlas ?? prim,
                  uSurfaceAtlas2: opt.atlas2 ?? opt.atlas ?? prim,
                  uNormalCold: opt.normalCold ?? opt.atlas ?? prim,
                  uNormalWarm: opt.normalWarm ?? opt.atlas ?? prim,
                  uSurfaceWarm: opt.surfaceWarm ?? opt.atlas ?? prim,
                  uReceiver: this.receiverRT.textures[0] },
      uniforms: (p) => {
        p.uInt("uLightW", LIGHT_TEXELS >> lod);
        p.uInt("uUnitT", UNITS_PER_TILE >> lod);
        p.uInt("uMapW", LIGHT_W);
        p.uInt("uCols", opt.win?.cols ?? SLOTS_X);
        p.uInt("uRows", opt.win?.rows ?? SLOTS_Y);
        p.uInt("uRefine", opt.refine ? 1 : 0);
        p.uInt("uDebugGate", opt.debugGate ? 1 : 0);
        p.uInt("uSlotCols", opt.slotCols ?? 0);
        p.uInt("uSlotRows", opt.slotRows ?? 0);
        p.uInt("uSlotPx", opt.slotPx ?? 0);
        p.uVec2("uWindowOrigin", originTileX, originTileY);
      },
    });
    renderer.draw({
      program: this.sumProg, geometry: this.quad, target: this.sumRT, blend: "none",
      textures: { uSlots: this.slotRT.textures[0] },
      uniforms: (p) => { p.uInt("uMapW", LIGHT_W); },
    });
  }

  /** Change ONE light: **withdraw, write, deposit**. `primIndex === 0` removes it.
   *
   *  Three draws, and every term is READ from the slot rather than predicted:
   *
   *    1. add `-slot` to the sum  — withdraws exactly what is currently deposited
   *    2. write the slot          — the hardware quantises however it likes
   *    3. add `+slot` to the sum  — deposits exactly what is now stored
   *
   *  The obvious two-draw version (emit `new - old`, then write `new`) is NOT exact: the delta uses
   *  the freshly-computed float while the slot keeps its `RGB10_A2` rounding, so a later removal
   *  subtracts a different number and the sum drifts by a quantum. Predicting the hardware's rounding
   *  in software was tried and still disagreed in the last bit — measured 0.0039, one full quantum.
   *  **Reading the stored value cannot be wrong about it.**
   *
   *  Still one slot: the other seven, and the whole rest of the sum, are never touched. */
  updateLight(renderer: Renderer, prim: Texture, slot: number, primIndex: number,
              originTileX: number, originTileY: number,
              win?: { cols: number; rows: number; lod: number }): void {
    const gl = this.gl;
    const deposit = (sign: number): void => {
      renderer.draw({
        program: this.deltaProg, geometry: this.quad, target: this.sumRT, blend: "add",
        textures: { uSlots: this.slotRT.textures[0] },
        uniforms: (p) => {
          p.uInt("uMapW", LIGHT_W); p.uInt("uSlot", slot); p.uFloat("uSign", sign);
        },
      });
    };
    deposit(-1);                                   // 1. withdraw what is there
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(slot * LIGHT_W, 0, LIGHT_W, LIGHT_H);
    renderer.draw({                                // 2. rewrite the slot
      program: this.slotOneProg, geometry: this.quad, target: this.slotRT, blend: "none",
      textures: { uPrim: prim },
      uniforms: (p) => {
        p.uInt("uLightW", LIGHT_TEXELS >> (win?.lod ?? 0)); p.uInt("uMapW", LIGHT_W);
        p.uInt("uCols", win?.cols ?? SLOTS_X); p.uInt("uRows", win?.rows ?? SLOTS_Y);
        p.uInt("uPrimIndex", primIndex);
        p.uVec2("uWindowOrigin", originTileX, originTileY);
      },
    });
    gl.disable(gl.SCISSOR_TEST);
    deposit(+1);                                   // 3. deposit what is now there
  }

  /** The map the display samples. Holds **quantisation levels**, not irradiance: multiply by
   *  `LIGHT_READ_SCALE` on read. Storing levels is what makes add/remove bit-exact. */
  get lightmap(): Texture { return this.sumRT.textures[0]; }
  get slots(): Texture { return this.slotRT.textures[0]; }
  get size(): { w: number; h: number; slots: number } {
    return { w: LIGHT_W, h: LIGHT_H, slots: LIGHT_SLOTS };
  }

  destroy(): void {
    this.slotRT.destroy(); this.sumRT.destroy();
    this.slotProg.destroy(); this.sumProg.destroy();
    this.deltaProg.destroy(); this.slotOneProg.destroy(); this.recvProg.destroy();
    this.receiverRT.destroy(); this.quad.destroy();
  }
}

/** Levels -> irradiance, for whatever samples {@link LightPass.lightmap}. */
export const LIGHT_READ_SCALE = LIGHT_SCALE / 1023;

export { LIGHT_W, LIGHT_H, LIGHT_TEXELS, TILE_SLOTS, SQUARE };
