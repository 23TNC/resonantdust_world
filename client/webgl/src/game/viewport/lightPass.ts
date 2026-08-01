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
import { LIGHT_SLOTS, LIGHT_SCALE, TILE_SLOTS } from "./records";
import { REACH_GLSL } from "./lightReach";
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
uniform highp usampler2D uPrim;      // prim_data   — one px per prim
uniform highp usampler2D uLight;     // light       — 8 u16 prim indices per TILE
uniform int uLightW;                 // lighting texels per axis of ONE tile
uniform vec2 uWindowOrigin;          // world tile of the map's (0,0)
uniform int uMapW;                   // one slot block's width in texels
out vec4 fragColor;

${REACH_GLSL}

const int   LIGHT_SLOTS = ${LIGHT_SLOTS};
const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
const float UPT         = ${UNITS_PER_TILE}.0;
const int   TILE_DIM    = 256;

int pmod(int a, int m) { return ((a % m) + m) % m; }

uvec4 fetchPrim(uint idx) {
  return texelFetch(uPrim, ivec2(int(idx) & 1023, int(idx) >> 10), 0);
}

void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  // F3: the slot map is LIGHT_SLOTS x wider, so x carries BOTH the texel and which light it is.
  // BLOCKED, not interleaved: slot s owns x in [s*W, (s+1)*W). A slot is then a RECTANGLE, which is
  // what lets the delta path scissor to one light without touching the other seven.
  int slot = fc.x / uMapW;
  int tx   = fc.x % uMapW;
  int ty   = fc.y;

  // texel -> world position, in units
  int tileX = int(uWindowOrigin.x) + tx / uLightW;
  int tileY = int(uWindowOrigin.y) + ty / uLightW;
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
  uint  intensity = rec.z & 0x3ffu;
  float reach     = reachFromIntensity(intensity);
  if (reach <= 0.0) { fragColor = vec4(0.0); return; }

  float d = distance(P, Lpos);
  if (d >= reach) { fragColor = vec4(0.0); return; }

  // The SAME falloff reachFromIntensity inverts: L(d) = I / (1 + (d/d0)^2).
  float I  = float(intensity) / INTENSITY_MAX;
  float at = I / (1.0 + (d / REACH_FALLOFF_UNITS) * (d / REACH_FALLOFF_UNITS));

  // colour.4 is the emitted light colour (ALPHA's low byte); .1-.3 are material tints.
  float c4 = float(rec.w & 0xffu) / 255.0;
  vec3 tint = vec3(1.0, 0.85 + 0.15 * c4, 0.6 + 0.4 * c4);

  // F2: stored at 1/LIGHT_SCALE so 4x overbright survives a 0..1 fixed-point format.
  fragColor = vec4(clamp(tint * at / LIGHT_SCALE, 0.0, 1.0), 1.0);
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
uniform int uPrimIndex;
uniform vec2 uWindowOrigin;
out vec4 fragColor;

${REACH_GLSL}

const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
const float UPT = ${UNITS_PER_TILE}.0;

uvec4 fetchPrim(uint idx) { return texelFetch(uPrim, ivec2(int(idx) & 1023, int(idx) >> 10), 0); }

void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  // This draw is SCISSORED to one slot's block, so gl_FragCoord.x carries the block offset
  // (slot*uMapW + tx). Strip it: the world position depends on the texel, not on which slot owns it.
  int tx = fc.x % uMapW;
  int tileX = int(uWindowOrigin.x) + tx / uLightW;
  int tileY = int(uWindowOrigin.y) + fc.y / uLightW;
  vec2 inTile = (vec2(float(tx % uLightW), float(fc.y % uLightW)) + 0.5) / float(uLightW);
  vec2 P = (vec2(float(tileX), float(tileY)) + inTile) * UPT;

  if (uPrimIndex == 0) { fragColor = vec4(0.0); return; }
  uvec4 rec = fetchPrim(uint(uPrimIndex));
  if (((rec.z >> 26) & 3u) == 0u) { fragColor = vec4(0.0); return; }
  vec2  Lpos      = vec2(float(rec.x >> 16), float(rec.x & 0xffffu));
  uint  intensity = rec.z & 0x3ffu;
  float reach     = reachFromIntensity(intensity);
  float d = distance(P, Lpos);
  if (reach <= 0.0 || d >= reach) { fragColor = vec4(0.0); return; }
  float I  = float(intensity) / INTENSITY_MAX;
  float at = I / (1.0 + (d / REACH_FALLOFF_UNITS) * (d / REACH_FALLOFF_UNITS));
  float c4 = float(rec.w & 0xffu) / 255.0;
  vec3 tint = vec3(1.0, 0.85 + 0.15 * c4, 0.6 + 0.4 * c4);
  fragColor = vec4(clamp(tint * at / LIGHT_SCALE, 0.0, 1.0), 1.0);
}
`;

export class LightPass {
  readonly slotRT: RenderTarget;
  readonly sumRT: RenderTarget;
  private readonly slotProg: Program;
  private readonly sumProg: Program;
  private readonly deltaProg: Program;
  private readonly slotOneProg: Program;
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
    this.slotProg = new Program(gl, FULLSCREEN_VERT, SLOT_FRAG, "light-slots");
    this.sumProg = new Program(gl, FULLSCREEN_VERT, SUM_FRAG, "light-sum");
    this.deltaProg = new Program(gl, FULLSCREEN_VERT, DELTA_FRAG, "light-delta");
    this.slotOneProg = new Program(gl, FULLSCREEN_VERT, SLOT_ONE_FRAG, "light-slot-one");
    this.quad = new Geometry(gl, this.slotProg, {
      aPos: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
  }

  /** Recompute every slot (ONE draw), then re-sum. */
  run(renderer: Renderer, prim: Texture, light: Texture,
      originTileX: number, originTileY: number): void {
    renderer.draw({
      program: this.slotProg, geometry: this.quad, target: this.slotRT, blend: "none",
      textures: { uPrim: prim, uLight: light },
      uniforms: (p) => {
        p.uInt("uLightW", LIGHT_TEXELS);
        p.uInt("uMapW", LIGHT_W);
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
              originTileX: number, originTileY: number): void {
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
        p.uInt("uLightW", LIGHT_TEXELS); p.uInt("uMapW", LIGHT_W);
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
    this.deltaProg.destroy(); this.slotOneProg.destroy(); this.quad.destroy();
  }
}

/** Levels -> irradiance, for whatever samples {@link LightPass.lightmap}. */
export const LIGHT_READ_SCALE = LIGHT_SCALE / 1023;

export { LIGHT_W, LIGHT_H, LIGHT_TEXELS, TILE_SLOTS, SQUARE };
