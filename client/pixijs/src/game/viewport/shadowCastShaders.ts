//! shadow-cast experiment — the two custom shaders. Both are GLSL ES 1.00 float-mod (Pixi's high-shader
//! is ES 1.00 — see design/rendering-platform.md); the bitfield lives in the RED byte, bits 0–4 = the 5
//! lights. The ALPHA channel is never used for data (A held at 1).
//!
//! - COMBINE: read `src` bitfield (main texture) + `mask` (the dirty light's shadow coverage) → write the
//!   updated bitfield: clear bit `k`, then set it where the mask covers; the other 4 bits carry through.
//!   Read `src`, write `dest` (ping-pong) — never the bound target (no feedback loop).
//! - DISPLAY: decode the RED byte's 5 bits → 5 distinct colours (additive overlap).

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  Matrix,
  UniformGroup,
} from "pixi.js";

// ── COMBINE ──────────────────────────────────────────────────────────────────────
const combineBitGl = {
  name: "shadow-combine-bit",
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uMask;   // the dirty light's shadow coverage (R > 0.5 = shadowed)
      uniform float uBit;        // which light bit (0..4) this update rewrites
    `,
    main: /* glsl */ `
      // outColor = src bitfield (textureBit). Rewrite ONLY bit uBit; carry the other bits through.
      float n = floor(outColor.r * 255.0 + 0.5);      // the RED byte (5 light bits)
      float p = exp2(uBit);                            // 2^k
      float had = mod(floor(n / p), 2.0);              // was bit k set?
      n = n - had * p;                                 // clear bit k
      float m = texture(uMask, vUV).r;                 // this light's coverage here
      n = n + (m > 0.5 ? p : 0.0);                     // set bit k where shadowed
      outColor = vec4(n / 255.0, 0.0, 0.0, 1.0);       // RED = bitfield, A = 1 (never data)
    `,
  },
};

let combineProgram: GlProgram | null = null;
function combineProg(): GlProgram {
  if (!combineProgram) {
    combineProgram = compileHighShaderGlProgram({
      name: "shadow-combine",
      bits: [localUniformBitGl, textureBitGl, combineBitGl, roundPixelsBitGl],
    });
  }
  return combineProgram;
}

export class ShadowCombineShader extends Shader {
  private _field: Texture = Texture.EMPTY;
  /** The SOURCE bitfield — the mesh's MAIN texture. */
  get texture(): Texture {
    return this._field;
  }
  set texture(value: Texture) {
    this._field = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  set field(value: Texture) {
    this.texture = value;
  }
  /** The dirty light's shadow coverage. */
  set mask(value: Texture) {
    this.resources.uMask = value.source;
    this.resources.uMaskSampler = value.source.style;
  }
  /** Which light bit (0..4) this pass rewrites. */
  setBit(k: number): void {
    this.resources.combineUniforms.uniforms.uBit = k;
    this.resources.combineUniforms.update();
  }
}

export function makeShadowCombineShader(): ShadowCombineShader {
  const e = Texture.EMPTY;
  return new ShadowCombineShader({
    glProgram: combineProg(),
    resources: {
      uTexture: e.source,
      uSampler: e.source.style,
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      uMask: e.source,
      uMaskSampler: e.source.style,
      combineUniforms: new UniformGroup({ uBit: { value: 0, type: "f32" } }),
    },
  });
}

// ── DISPLAY ──────────────────────────────────────────────────────────────────────
const displayBitGl = {
  name: "shadow-display-bit",
  fragment: {
    main: /* glsl */ `
      // outColor = the bitfield (textureBit). Decode RED bits 0..4 → 5 colours, additive overlap.
      float n = floor(outColor.r * 255.0 + 0.5);
      vec3 acc = vec3(0.0);
      if (mod(floor(n /  1.0), 2.0) > 0.5) acc += vec3(1.0, 0.25, 0.25); // light 0 — red
      if (mod(floor(n /  2.0), 2.0) > 0.5) acc += vec3(0.25, 1.0, 0.30); // light 1 — green
      if (mod(floor(n /  4.0), 2.0) > 0.5) acc += vec3(0.30, 0.55, 1.0); // light 2 — blue
      if (mod(floor(n /  8.0), 2.0) > 0.5) acc += vec3(1.0, 0.95, 0.25); // light 3 — yellow
      if (mod(floor(n / 16.0), 2.0) > 0.5) acc += vec3(1.0, 0.35, 1.0);  // light 4 — magenta
      outColor = length(acc) < 0.01 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
    `,
  },
};

let displayProgram: GlProgram | null = null;
function displayProg(): GlProgram {
  if (!displayProgram) {
    displayProgram = compileHighShaderGlProgram({
      name: "shadow-display",
      bits: [localUniformBitGl, textureBitGl, displayBitGl, roundPixelsBitGl],
    });
  }
  return displayProgram;
}

export class ShadowDisplayShader extends Shader {
  private _field: Texture = Texture.EMPTY;
  get texture(): Texture {
    return this._field;
  }
  set texture(value: Texture) {
    this._field = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  set field(value: Texture) {
    this.texture = value;
  }
}

export function makeShadowDisplayShader(): ShadowDisplayShader {
  const e = Texture.EMPTY;
  return new ShadowDisplayShader({
    glProgram: displayProg(),
    resources: {
      uTexture: e.source,
      uSampler: e.source.style,
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
    },
  });
}
