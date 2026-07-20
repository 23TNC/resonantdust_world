//! shadow-tiered experiment — shaders. GLSL ES 1.00 float-mod (Pixi high-shader is ES 1.00). Bits live in
//! the RED byte, bits 0..4 = the 5 lights; A is never data.
//!
//! - MERGE: builds the new world bitfield in ONE pass — `cur-world = (prev-world with the dirty bits
//!   cleared) OR (prev-screen, remapped screen→world)`. Reads `prev-world` (main texture, at the buffer
//!   pixel) + `prev-screen` (a 2nd sampler, at the inverted buffer→world→screen position); writes
//!   `cur-world`. No feedback (cur-world is never sampled), no blend.
//! - DISPLAY: full-viewport decode of `cur-world` (sampled by the forward screen→world→buffer map) OR
//!   `cur-screen` (sampled directly) → 5 colours, additive overlap.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  defaultFilterVert,
  Filter,
  GlProgram,
  Shader,
  Texture,
  Matrix,
  UniformGroup,
} from "pixi.js";

// ── shared GLSL: decode RED byte's bits 0..4 → 5 colours ──────────────────────────
const DECODE = /* glsl */ `
  vec3 decodeBits(float n) {
    vec3 acc = vec3(0.0);
    if (mod(floor(n /  1.0), 2.0) > 0.5) acc += vec3(1.0, 0.25, 0.25);
    if (mod(floor(n /  2.0), 2.0) > 0.5) acc += vec3(0.25, 1.0, 0.30);
    if (mod(floor(n /  4.0), 2.0) > 0.5) acc += vec3(0.30, 0.55, 1.0);
    if (mod(floor(n /  8.0), 2.0) > 0.5) acc += vec3(1.0, 0.95, 0.25);
    if (mod(floor(n / 16.0), 2.0) > 0.5) acc += vec3(1.0, 0.35, 1.0);
    return acc;
  }
`;

// ── MERGE ─────────────────────────────────────────────────────────────────────────
const mergeBitGl = {
  name: "shadow-merge-bit",
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uScreen;   // prev-screen (screen space)
      uniform float uClearBits;    // which of bits 0..4 to clear from prev-world (0..31)
      uniform vec4 uMapA;          // winCol, winRow, cols, rows
      uniform vec4 uMapB;          // slotPx, fixedCW, fixedCH, SQUARE
      uniform vec4 uMapC;          // prevWinCol, prevWinRow, (unused, unused)
      uniform vec4 uCam;           // panX, panY, zoom, (unused)
      uniform vec2 uView;          // viewport w, h (px)
    `,
    main: /* glsl */ `
      // outColor = prev-world sampled at this buffer pixel (textureBit at vUV).
      float wN = floor(outColor.r * 255.0 + 0.5);
      for (int i = 0; i < 5; i++) {
        float bit = exp2(float(i));
        if (mod(floor(uClearBits / bit), 2.0) > 0.5 && mod(floor(wN / bit), 2.0) > 0.5) wN -= bit;
      }
      // Invert buffer pixel → world square → world px → cast-frame screen px → screen uv.
      float winCol = uMapA.x, winRow = uMapA.y, cols = uMapA.z, rows = uMapA.w;
      float slotPx = uMapB.x, fixedCW = uMapB.y, fixedCH = uMapB.z, sq = uMapB.w;
      float bx = vUV.x * fixedCW, by = vUV.y * fixedCH;
      float sc = floor(bx / slotPx) - 1.0, sr = floor(by / slotPx) - 1.0;      // slot col/row (interior)
      float fx = bx / slotPx - 1.0 - sc, fy = by / slotPx - 1.0 - sr;          // frac within the slot
      float wc = winCol + mod(sc - winCol, cols);                             // resident world square
      float wr = winRow + mod(sr - winRow, rows);
      // Leading-edge invalidation: if this slot's resident world square changed since prev-world was
      // written (the window panned), the stored bits belong to the evicted square — stale. Zero them so a
      // freshly-entered zone shows no ghost (shadows repopulate as lights recast). This is the per-square
      // re-bake the composites get for free; without it a slot silently reinterprets old bits as the new
      // square's shadow.
      float wcPrev = uMapC.x + mod(sc - uMapC.x, cols);
      float wrPrev = uMapC.y + mod(sr - uMapC.y, rows);
      if (abs(wc - wcPrev) > 0.5 || abs(wr - wrPrev) > 0.5) wN = 0.0;
      float wx = (wc + fx) * sq, wy = (wr + fy) * sq;
      vec2 suv = vec2(((wx + uCam.x) * uCam.z) / uView.x, ((wy + uCam.y) * uCam.z) / uView.y);
      float sN = 0.0;
      if (suv.x >= 0.0 && suv.x <= 1.0 && suv.y >= 0.0 && suv.y <= 1.0) sN = floor(texture(uScreen, suv).r * 255.0 + 0.5);
      outColor = vec4((wN + sN) / 255.0, 0.0, 0.0, 1.0);   // disjoint → sum == OR
    `,
  },
};

let mergeProgram: GlProgram | null = null;
function mergeProg(): GlProgram {
  if (!mergeProgram) mergeProgram = compileHighShaderGlProgram({ name: "shadow-merge", bits: [localUniformBitGl, textureBitGl, mergeBitGl, roundPixelsBitGl] });
  return mergeProgram;
}

export class ShadowMergeShader extends Shader {
  private _world: Texture = Texture.EMPTY;
  get texture(): Texture {
    return this._world;
  }
  set texture(v: Texture) {
    this._world = v;
    this.resources.uTexture = v.source;
    this.resources.uSampler = v.source.style;
  }
  set prevWorld(v: Texture) {
    this.texture = v;
  }
  set prevScreen(v: Texture) {
    this.resources.uScreen = v.source;
    this.resources.uScreenSampler = v.source.style;
  }
  setClear(bits: number): void {
    this.resources.mergeUniforms.uniforms.uClearBits = bits;
    this.resources.mergeUniforms.update();
  }
  setMapping(winCol: number, winRow: number, cols: number, rows: number, slotPx: number, fixedCW: number, fixedCH: number, sq: number, prevWinCol: number, prevWinRow: number): void {
    const u = this.resources.mergeUniforms.uniforms;
    u.uMapA = new Float32Array([winCol, winRow, cols, rows]);
    u.uMapB = new Float32Array([slotPx, fixedCW, fixedCH, sq]);
    u.uMapC = new Float32Array([prevWinCol, prevWinRow, 0, 0]);
    this.resources.mergeUniforms.update();
  }
  setCam(panX: number, panY: number, zoom: number, vw: number, vh: number): void {
    const u = this.resources.mergeUniforms.uniforms;
    u.uCam = new Float32Array([panX, panY, zoom, 0]);
    u.uView = new Float32Array([vw, vh]);
    this.resources.mergeUniforms.update();
  }
}

export function makeShadowMergeShader(): ShadowMergeShader {
  const e = Texture.EMPTY;
  return new ShadowMergeShader({
    glProgram: mergeProg(),
    resources: {
      uTexture: e.source,
      uSampler: e.source.style,
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      uScreen: e.source,
      uScreenSampler: e.source.style,
      mergeUniforms: new UniformGroup({
        uClearBits: { value: 0, type: "f32" },
        uMapA: { value: new Float32Array(4), type: "vec4<f32>" },
        uMapB: { value: new Float32Array(4), type: "vec4<f32>" },
        uMapC: { value: new Float32Array(4), type: "vec4<f32>" },
        uCam: { value: new Float32Array(4), type: "vec4<f32>" },
        uView: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      }),
    },
  });
}

// ── DISPLAY ───────────────────────────────────────────────────────────────────────
const displayBitGl = {
  name: "shadow-tdisplay-bit",
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uScreen;   // cur-screen (screen space)
      uniform sampler2D uLightData; // 5×5 light-data texture: column = light, row 3 = RGBA colour
      uniform vec4 uMapA;          // cols, rows, slotPx, fixedCW
      uniform vec4 uMapB;          // fixedCH, SQUARE, (unused, unused)
      uniform vec4 uCam;           // panX, panY, zoom, (unused)
      uniform vec2 uView;          // viewport w, h
      // Decode the RED-byte bitfield, colouring each set bit with that light's colour READ FROM the data
      // texture (row 3, texel-centre) — no hardcoded palette. This is the light-data-texture proof.
      vec3 decodeBitsTex(float n) {
        vec3 acc = vec3(0.0);
        for (int i = 0; i < 5; i++) {
          if (mod(floor(n / exp2(float(i))), 2.0) > 0.5)
            acc += texture(uLightData, vec2((float(i) + 0.5) / 5.0, 3.5 / 5.0)).rgb;
        }
        return acc;
      }
    `,
    main: /* glsl */ `
      // vUV = screen 0..1. Forward-map screen → world → buffer uv to sample cur-world (uTexture).
      float cols = uMapA.x, rows = uMapA.y, slotPx = uMapA.z, fixedCW = uMapA.w;
      float fixedCH = uMapB.x, sq = uMapB.y;
      float wx = vUV.x * uView.x / uCam.z - uCam.x;
      float wy = vUV.y * uView.y / uCam.z - uCam.y;
      vec2 buv = vec2((mod(wx / sq, cols) + 1.0) * slotPx / fixedCW, (mod(wy / sq, rows) + 1.0) * slotPx / fixedCH);
      float worldN = floor(texture(uTexture, buv).r * 255.0 + 0.5);
      float screenN = floor(texture(uScreen, vUV).r * 255.0 + 0.5);
      vec3 acc = decodeBitsTex(worldN) + decodeBitsTex(screenN);
      outColor = length(acc) < 0.01 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
    `,
  },
};

let displayProgram: GlProgram | null = null;
function displayProg(): GlProgram {
  if (!displayProgram) displayProgram = compileHighShaderGlProgram({ name: "shadow-tdisplay", bits: [localUniformBitGl, textureBitGl, displayBitGl, roundPixelsBitGl] });
  return displayProgram;
}

export class ShadowTDisplayShader extends Shader {
  private _world: Texture = Texture.EMPTY;
  get texture(): Texture {
    return this._world;
  }
  set texture(v: Texture) {
    this._world = v;
    this.resources.uTexture = v.source;
    this.resources.uSampler = v.source.style;
  }
  set curWorld(v: Texture) {
    this.texture = v;
  }
  set curScreen(v: Texture) {
    this.resources.uScreen = v.source;
    this.resources.uScreenSampler = v.source.style;
  }
  set lightData(v: Texture) {
    this.resources.uLightData = v.source;
    this.resources.uLightDataSampler = v.source.style;
  }
  setMapping(cols: number, rows: number, slotPx: number, fixedCW: number, fixedCH: number, sq: number): void {
    const u = this.resources.dispUniforms.uniforms;
    u.uMapA = new Float32Array([cols, rows, slotPx, fixedCW]);
    u.uMapB = new Float32Array([fixedCH, sq, 0, 0]);
    this.resources.dispUniforms.update();
  }
  setCam(panX: number, panY: number, zoom: number, vw: number, vh: number): void {
    const u = this.resources.dispUniforms.uniforms;
    u.uCam = new Float32Array([panX, panY, zoom, 0]);
    u.uView = new Float32Array([vw, vh]);
    this.resources.dispUniforms.update();
  }
}

export function makeShadowTDisplayShader(): ShadowTDisplayShader {
  const e = Texture.EMPTY;
  return new ShadowTDisplayShader({
    glProgram: displayProg(),
    resources: {
      uTexture: e.source,
      uSampler: e.source.style,
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      uScreen: e.source,
      uScreenSampler: e.source.style,
      uLightData: e.source,
      uLightDataSampler: e.source.style,
      dispUniforms: new UniformGroup({
        uMapA: { value: new Float32Array(4), type: "vec4<f32>" },
        uMapB: { value: new Float32Array(4), type: "vec4<f32>" },
        uCam: { value: new Float32Array(4), type: "vec4<f32>" },
        uView: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      }),
    },
  });
}

// ── DECODE FILTER ─────────────────────────────────────────────────────────────────
// A plain Pixi filter that decodes the RED-byte bitfield IN PLACE — used by the `/showRT`
// preview so the `shadow-a`/`shadow-b` thumbnails read as the same 5 per-light colours the
// on-screen shadow display uses, instead of the near-black raw bits. Same {@link DECODE} as
// the display, so both stay in lockstep. GL-only (the whole renderer is WebGL here).
const decodeFilterFrag = /* glsl */ `
  in vec2 vTextureCoord;
  out vec4 finalColor;
  uniform sampler2D uTexture;
  ${DECODE}
  void main() {
    float n = floor(texture(uTexture, vTextureCoord).r * 255.0 + 0.5);
    vec3 acc = decodeBits(n);
    finalColor = length(acc) < 0.01 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
  }
`;

/** A filter that colourises a red-byte shadow bitfield (bits 0..4 → 5 light colours), matching the
 *  on-screen shadow display. Stateless — one instance can drive both `shadow-a` and `shadow-b`
 *  thumbnails. */
export function makeShadowDecodeFilter(): Filter {
  return new Filter({
    glProgram: GlProgram.from({ vertex: defaultFilterVert, fragment: decodeFilterFrag, name: "shadow-decode-filter" }),
    resources: {},
  });
}
