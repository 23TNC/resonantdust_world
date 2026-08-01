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
  int slot = fc.x % LIGHT_SLOTS;
  int tx   = fc.x / LIGHT_SLOTS;
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
out vec4 fragColor;
const int LIGHT_SLOTS = ${LIGHT_SLOTS};
const float LIGHT_SCALE = ${LIGHT_SCALE}.0;
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  vec3 acc = vec3(0.0);
  for (int s = 0; s < LIGHT_SLOTS; s++) {
    acc += texelFetch(uSlots, ivec2(fc.x * LIGHT_SLOTS + s, fc.y), 0).rgb;
  }
  fragColor = vec4(acc * LIGHT_SCALE, 1.0);   // undo the store scale ONCE, here
}
`;

export class LightPass {
  readonly slotRT: RenderTarget;
  readonly sumRT: RenderTarget;
  private readonly slotProg: Program;
  private readonly sumProg: Program;
  private readonly quad: Geometry;

  constructor(private readonly gl: WebGL2RenderingContext) {
    // 8x wide: one column per light, so ONE draw covers every slot (F3).
    this.slotRT = new RenderTarget(gl, {
      width: LIGHT_W * LIGHT_SLOTS, height: LIGHT_H, formats: ["rgb10a2unorm"],
    });
    // RGBA16F for the sum: it holds up to LIGHT_SLOTS x 4.0, past what a fixed-point 0..1 can carry.
    this.sumRT = new RenderTarget(gl, { width: LIGHT_W, height: LIGHT_H, formats: ["rgba16float"] });
    this.slotProg = new Program(gl, FULLSCREEN_VERT, SLOT_FRAG, "light-slots");
    this.sumProg = new Program(gl, FULLSCREEN_VERT, SUM_FRAG, "light-sum");
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
        p.uVec2("uWindowOrigin", originTileX, originTileY);
      },
    });
    renderer.draw({
      program: this.sumProg, geometry: this.quad, target: this.sumRT, blend: "none",
      textures: { uSlots: this.slotRT.textures[0] },
    });
  }

  get lightmap(): Texture { return this.sumRT.textures[0]; }
  get slots(): Texture { return this.slotRT.textures[0]; }
  get size(): { w: number; h: number; slots: number } {
    return { w: LIGHT_W, h: LIGHT_H, slots: LIGHT_SLOTS };
  }

  destroy(): void {
    this.slotRT.destroy(); this.sumRT.destroy();
    this.slotProg.destroy(); this.sumProg.destroy(); this.quad.destroy();
  }
}

export { LIGHT_W, LIGHT_H, LIGHT_TEXELS, TILE_SLOTS, SQUARE };
