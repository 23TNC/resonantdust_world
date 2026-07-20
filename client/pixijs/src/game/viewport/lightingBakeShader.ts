//! The COLD-LIGHT lightmap bake (lighting P1) — a derived `SquareCache` composite. One rect at a time:
//! a `W×H` quad samples that rect's already-baked **normal** slot (via `textureBit`) and writes the
//! static-light SUM into the `cold-lightmap` composite —
//!
//!   cold_lightmap = ambient + Σ coldColor·brightness · max(N·L, 0) · falloff²
//!
//! Baked in WORLD space (`world = uRectWorld + vLocal`, panning-free) and only when a rect is
//! geometry- or light-dirty (amortized). The display then does `lit = albedo × (cold_lightmap + Σ hot)`.
//! Cold shadows (`uColdShadow`) + the standing-object normal tilt are a later increment (lighting P4);
//! this first pass is `ambient + Σ N·L·falloff` on the baked normal.
//!
//! A Pixi v8 high-shader Mesh program (same bits as the display `lightingShader`); we keep the RAW
//! quad `vUV` to sample the normal composite, and the mesh's main texture IS the normal slot.
//!
//! GOTCHA (as the other bake shaders): a GLSL compile error draws the mesh BLACK with only a
//! console.error; a backtick inside a GLSL comment closes the template literal.

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
import { BITFIELD_GLSL } from "../lighting/bitfield";

/** Max COLD (static, baked) lights summed into the lightmap. High because they cost nothing per
 *  frame (baked once); the loop breaks on `uLightCount`. Structured 32 to mirror the dynamic pool. */
export const MAX_COLD_LIGHTS = 32;

const lightBakeBitGl = {
  name: "cold-light-bake-bit",
  vertex: {
    header: /* glsl */ `out vec2 vLocal; out vec2 vRawUv;`,
    // aPosition is the rect-local px (0..W, 0..H) of the scratch quad; vRawUv is the UNtransformed
    // quad UV (0..1) — `textureBit`'s vUV is already mapped into the normal slot, so the coldShadow
    // sample must use this raw UV (× uShadowRect) or it double-applies the slot offset.
    main: /* glsl */ `vLocal = aPosition; vRawUv = aUV;`,
  },
  fragment: {
    header: /* glsl */ `
      uniform vec4 uLightData[${MAX_COLD_LIGHTS}];  // xy world px, z height, w radius px
      uniform vec4 uLightColor[${MAX_COLD_LIGHTS}]; // rgb colour, a brightness
      uniform float uLightCount;
      uniform vec3 uAmbient;                         // ambient floor (colour × intensity), never shadowed
      uniform float uNormalYSign;                    // flip normal Y into the screen convention (-1)
      uniform vec2 uRectWorld;                       // this rect's world origin (world px)
      uniform sampler2D uColdShadow;                 // cold-shadow 32-bit occlusion BITFIELD (bit i = cold light i shadowed)
      uniform vec4 uShadowRect;                      // this square's slot in the coldShadow composite (offset.xy, scale.zw)
      in vec2 vLocal;
      in vec2 vRawUv;                                // untransformed quad UV (0..1) for the coldShadow slot
      ${BITFIELD_GLSL}
    `,
    main: /* glsl */ `
      // outColor = the NORMAL slot (textureBit). Near-black texels (empty/cleared cell) → flat-up +Z
      // so they light like flat ground; else decode straight.
      vec3 nrm = length(outColor.rgb) < 0.02 ? vec3(0.0, 0.0, 1.0) : outColor.rgb * 2.0 - 1.0;
      vec3 N = normalize(vec3(nrm.x, nrm.y * uNormalYSign, nrm.z));
      vec2 world = uRectWorld + vLocal;
      // Half-Lambert wrap — KEEP IN SYNC with the display shader's LIGHT_WRAP (cold + hot light the
      // same surface): ndotl = max((N·L + w)/(1+w), 0), w>0 softens the terminator.
      const float LIGHT_WRAP = 0.4;
      const float SHADOW_STRENGTH = 0.85;             // 1 = a shadow fully removes its light's term
      // The first 3 cold lights (R/G/B) lose their term where the baked cold-shadow covers this
      // fragment; a 4th+ cold light casts no shadow (no lane). Ambient is never shadowed.
      vec4 csh = texture(uColdShadow, uShadowRect.xy + vRawUv * uShadowRect.zw);
      vec3 sum = uAmbient;
      for (int i = 0; i < ${MAX_COLD_LIGHTS}; i++) {
        if (float(i) >= uLightCount) break;
        vec4 ld = uLightData[i];
        vec3 toLight = vec3(ld.xy - world, ld.z);     // fragment → light (world px + height)
        float atten = clamp(1.0 - length(toLight.xy) / max(ld.w, 1.0), 0.0, 1.0);
        atten *= atten;                               // quadratic falloff
        float ndotl = max((dot(N, normalize(toLight)) + LIGHT_WRAP) / (1.0 + LIGHT_WRAP), 0.0);
        int shc = i / 8; int shb = i - shc * 8;         // cold light i → bitfield channel shc, bit shb
        float sh = bf_bit(bf_byte(csh, shc), shb);      // 1 = occluded (all 32 cold lights)
        sum += uLightColor[i].rgb * uLightColor[i].a * ndotl * atten * (1.0 - sh * SHADOW_STRENGTH);
      }
      outColor = vec4(sum, 1.0);                       // OPAQUE — the composite is opaque light data
    `,
  },
};

let program: GlProgram | null = null;
function lightBakeProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      // textureBit BEFORE the bake bit: outColor must hold the normal sample when it runs.
      name: "cold-light-bake",
      bits: [localUniformBitGl, textureBitGl, lightBakeBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** Unpack `0xRRGGBB` × intensity → `[r,g,b]` 0..1. */
function rgb(color: number, intensity = 1): Float32Array {
  return new Float32Array([
    (((color >> 16) & 0xff) / 255) * intensity,
    (((color >> 8) & 0xff) / 255) * intensity,
    ((color & 0xff) / 255) * intensity,
  ]);
}

/** The cold-light lightmap bake material. Bind the rect's NORMAL slot via `texture`, set the cold
 *  lights + ambient + the rect's world origin, then render a rect-local quad into the lightmap slot. */
export class LightingBakeShader extends Shader {
  private _normal: Texture = Texture.EMPTY;

  /** The NORMAL slot — the mesh's MAIN texture (`textureBit` samples it into `outColor`). */
  set normal(value: Texture) {
    this._normal = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  /** `Mesh` requires a `TextureShader` (expose `texture`); the main texture IS the normal slot. */
  get texture(): Texture {
    return this._normal;
  }
  set texture(value: Texture) {
    this.normal = value;
  }
  /** This rect's world origin (world px) — `world = uRectWorld + vLocal`. Set per rect. */
  setRectWorld(x: number, y: number): void {
    this.resources.bakeUniforms.uniforms.uRectWorld = new Float32Array([x, y]);
    this.resources.bakeUniforms.update();
  }
  /** Map the quad's `vUV` (0..1) to this square's NORMAL **slot** within the toroidal composite —
   *  `(sx,sy)` slot offset, `(sw,sh)` slot size, in composite-UV (px / composite px). `textureBit`
   *  reads `uTexture` through this matrix, so a full-quad samples just the slot. Set per square. */
  setNormalUv(sx: number, sy: number, sw: number, sh: number): void {
    const g = this.resources.textureUniforms;
    (g.uniforms.uTextureMatrix as Matrix).set(sw, 0, 0, sh, sx, sy);
    g.update();
  }
  /** The baked cold-shadow composite (R/G/B = cold light 0/1/2 coverage). */
  set coldShadow(value: Texture) {
    this.resources.uColdShadow = value.source;
    this.resources.uColdShadowSampler = value.source.style;
  }
  /** This square's slot in the coldShadow composite (`vUV·zw + xy`, composite-UV). Set per square. */
  setShadowRect(sx: number, sy: number, sw: number, sh: number): void {
    this.resources.bakeUniforms.uniforms.uShadowRect = new Float32Array([sx, sy, sw, sh]);
    this.resources.bakeUniforms.update();
  }
  /** The ambient floor (`0xRRGGBB` × intensity). */
  setAmbient(color: number, intensity: number): void {
    this.resources.bakeUniforms.uniforms.uAmbient = rgb(color, intensity);
    this.resources.bakeUniforms.update();
  }
  /** The cold lights: `data`/`color` are {@link MAX_COLD_LIGHTS}·4-float arrays (data = xy,z,radius;
   *  color = rgb,brightness); only the first `count` are read. */
  setLights(data: Float32Array, color: Float32Array, count: number): void {
    const u = this.resources.bakeUniforms.uniforms;
    u.uLightData = data;
    u.uLightColor = color;
    u.uLightCount = count;
    this.resources.bakeUniforms.update();
  }
}

/** A fresh cold-light bake material (its own uniform group). */
export function makeLightingBakeShader(): LightingBakeShader {
  const empty = Texture.EMPTY;
  return new LightingBakeShader({
    glProgram: lightBakeProgram(),
    resources: {
      uTexture: empty.source,
      uSampler: empty.source.style,
      uColdShadow: empty.source,
      uColdShadowSampler: empty.source.style,
      // Identity — vUV = aUV (the normal composite stores it upright).
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      bakeUniforms: new UniformGroup({
        uAmbient: { value: new Float32Array([0.0, 0.0, 0.0]), type: "vec3<f32>" },
        uNormalYSign: { value: -1, type: "f32" },
        uRectWorld: { value: new Float32Array([0, 0]), type: "vec2<f32>" },
        uShadowRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uLightData: { value: new Float32Array(MAX_COLD_LIGHTS * 4), type: "vec4<f32>", size: MAX_COLD_LIGHTS },
        uLightColor: { value: new Float32Array(MAX_COLD_LIGHTS * 4), type: "vec4<f32>", size: MAX_COLD_LIGHTS },
        uLightCount: { value: 0, type: "f32" },
      }),
    },
  });
}
