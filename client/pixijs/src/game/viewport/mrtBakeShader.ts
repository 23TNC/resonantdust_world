//! The MERGED G-buffer bake shader (mrt-bakes B3) — ONE ES 3.00 fragment that writes all four channels in a
//! single pass via MRT, replacing the four separate bake shaders (material/surface/normal/depth). Every prim
//! is a universal material (B2), so there's one path + one shared silhouette `discard` on coverage.
//!
//! Outputs (attachments): 0 = albedo (material reconstruction × tint), 1 = surface (presence/ao/coverage),
//! 2 = normal (silhouette-keyed, flat-up fallback), 3 = zdepth_world (tile depth in B). The shared surface
//! map supplies BOTH the coverage/discard AND the surface output (I-8). Built via the high-shader ES 3.00
//! path so Pixi's transform/uniform plumbing rides along; the extra outs are declared with explicit
//! `layout(location=1..3)` while the template's `finalColor` is location 0 = albedo (I-9).

import { localUniformBitGl, roundPixelsBitGl, GlProgram, Shader, Texture, UniformGroup } from "pixi.js";
import { compileHighShaderGlProgramES300 } from "./es3HighShader";
import { OKLAB_GLSL } from "../lighting/oklab";

/** Material weight channels the reconstruction sums — the width of the `uCh*` arrays + the RGB channels of
 *  the `layers` map. (The 4th/alpha channel was dropped: data in alpha fights the premultiply/downscale.) */
export const PACKED_CHANNELS = 3;

/** A texture's atlas-page uv rect `[offU, offV, scaleU, scaleV]` (identity for a whole-page texture). */
function uvRect(t: Texture): Float32Array {
  const f = t.frame;
  return new Float32Array([f.x / t.source.width, f.y / t.source.height, f.width / t.source.width, f.height / t.source.height]);
}

const mrtBitGl = {
  name: "mrt-bake-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uResidual;               // albedo residual base (RGB)
      uniform sampler2D uLayers;                 // per-material weight map (RGB coefficients)
      uniform sampler2D uSurface;                // surface: R=height, G=ao, B=coverage (also the silhouette)
      uniform sampler2D uNoise;                  // tiling noise atlas
      uniform sampler2D uNormalTex;              // normal LOD (RGB; flat-up bg)
      uniform vec4 uResidualRect;                // residual uv rect on its page
      uniform vec4 uLayersRect;                  // layers uv rect
      uniform vec4 uSurfaceRect;                 // surface uv rect
      uniform vec4 uNormalRect;                  // normal uv rect
      uniform vec4 uChA[${PACKED_CHANNELS}];     // per channel: tint.rgb, hueSwing
      uniform vec4 uChB[${PACKED_CHANNELS}];     // per channel: chromaSwing, warmCoolBias, noiseRow, sampleSpace
      uniform vec4 uNoiseParams;                 // x=atlas rows, y=uv tiling, z=world px per noise tile
      uniform vec4 uWorldRect;                   // xy = prim world origin px, zw = prim world size px
      uniform vec3 uTint;                        // albedo OUTPUT multiply (white real, geoColor solid)
      uniform float uSeed;                       // stable per-instance seed
      uniform float uHasLayers;                  // 1 = add layer contributions
      uniform float uHasNormal;                  // 1 = sample uNormalTex, 0 = flat-up
      uniform float uTileDepth;                  // >=0 = thing depth; < 0 = ground (write black)

      layout(location = 1) out vec4 oSurface;    // location 0 = finalColor = albedo (template)
      layout(location = 2) out vec4 oNormal;
      layout(location = 3) out vec4 oDepth;

      ${OKLAB_GLSL}
    `,
    main: /* glsl */ `
      // Shared silhouette: coverage from the surface map's B. Solid materials carry a WHITE surface (cov=1),
      // so they never discard (full box); real things discard outside their silhouette (ground behind survives
      // in ALL four channels). One discard governs all outputs.
      float cov = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).b;
      if (cov < 0.5) discard;

      // --- attachment 0: albedo (material reconstruction × tint) ---
      vec3 base = texture(uResidual, uResidualRect.xy + vUV * uResidualRect.zw).rgb;
      vec3 outc = base;
      if (uHasLayers > 0.5) {
        vec3 weights = texture(uLayers, uLayersRect.xy + vUV * uLayersRect.zw).rgb;
        vec2 instanceOffset = fract(vec2(sin(uSeed * 127.1 + 311.7), sin(uSeed * 269.5 + 183.3)) * 43758.5453);
        for (int i = 0; i < ${PACKED_CHANNELS}; i++) {
          vec4 A = uChA[i];
          vec4 B = uChB[i];
          vec3 tint = A.rgb;
          float noiseRow = B.z;
          vec2 nuv = B.w < 0.5
            ? vUV * uNoiseParams.y + instanceOffset
            : (uWorldRect.xy + vUV * uWorldRect.zw) / max(uNoiseParams.z, 1.0);
          vec2 n = vec2(0.5);
          if (noiseRow >= 0.0) {
            float row = (noiseRow + fract(nuv.y)) / max(uNoiseParams.x, 1.0);
            n = texture(uNoise, vec2(fract(nuv.x), row)).rg;
          }
          vec3 jit = jitterHueChroma(tint, n.r, n.g, A.a, B.x, B.y);
          outc += weights[i] * jit;
        }
      }
      outColor = vec4(outc * uTint, 1.0);        // → finalColor (attachment 0)

      // --- attachment 1: surface (R=presence, G=ao, B=coverage) ---
      // Presence = "a thing is here": 1 for real things, 0 for ground/geo. That's exactly tileDepth>=0
      // (things carry a depth; ground/geo carry -1), so this matches the old per-case R byte-identically
      // (old: real R=1 hardcoded, flat R=0) — not the flat R=1 a naive presence=1 would give.
      vec3 ssurf = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).rgb;
      oSurface = vec4(uTileDepth >= 0.0 ? 1.0 : 0.0, ssurf.g, ssurf.b, 1.0);

      // --- attachment 2: normal (real, or flat-up) ---
      oNormal = uHasNormal > 0.5
        ? vec4(texture(uNormalTex, uNormalRect.xy + vUV * uNormalRect.zw).rgb, 1.0)
        : vec4(0.5, 0.5, 1.0, 1.0);

      // --- attachment 3: zdepth_world (tile depth in B; ground = black) ---
      oDepth = vec4(0.0, 0.0, uTileDepth < 0.0 ? 0.0 : uTileDepth, 1.0);
    `,
  },
};

let program: GlProgram | null = null;
function mrtProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgramES300({ name: "mrt-bake", bits: [localUniformBitGl, mrtBitGl, roundPixelsBitGl] });
    // The template declares `out vec4 finalColor;` with NO explicit location (it's the sole output normally).
    // With our extra layout(location=1..3) outs, ES 3.00 requires ALL fragment outputs to be explicitly
    // located — so pin finalColor to location 0 (= albedo). Patch the assembled source before the GL program
    // compiles lazily on first render (mrt-bakes I-9).
    const p = program as unknown as { fragment: string };
    p.fragment = p.fragment.replace("out vec4 finalColor;", "layout(location = 0) out vec4 finalColor;");
  }
  return program;
}

/** The merged bake material for one prim — set every channel's inputs, render once, write all four
 *  attachments. Pooled by {@link SquareCache}. */
export class MrtBakeShader extends Shader {
  private _residual: Texture = Texture.EMPTY;
  get texture(): Texture {
    return this._residual;
  }
  set texture(v: Texture) {
    this.residual = v;
  }

  set residual(v: Texture) {
    this._residual = v;
    this.resources.uResidual = v.source;
    this.resources.uResidualSampler = v.source.style;
    this.resources.mrtUniforms.uniforms.uResidualRect = uvRect(v);
    this.resources.mrtUniforms.update();
  }
  set layers(v: Texture | null) {
    const t = v ?? Texture.EMPTY;
    this.resources.uLayers = t.source;
    this.resources.uLayersSampler = t.source.style;
    const u = this.resources.mrtUniforms.uniforms;
    u.uLayersRect = v ? uvRect(v) : new Float32Array([0, 0, 1, 1]);
    u.uHasLayers = v ? 1 : 0;
    this.resources.mrtUniforms.update();
  }
  set surface(v: Texture) {
    this.resources.uSurface = v.source;
    this.resources.uSurfaceSampler = v.source.style;
    this.resources.mrtUniforms.uniforms.uSurfaceRect = uvRect(v);
    this.resources.mrtUniforms.update();
  }
  set noise(v: Texture) {
    this.resources.uNoise = v.source;
    this.resources.uNoiseSampler = v.source.style;
  }
  /** The normal LOD, or `null` → flat-up fallback (`uHasNormal = 0`). */
  set normalTex(v: Texture | null) {
    const t = v ?? Texture.EMPTY;
    this.resources.uNormalTex = t.source;
    this.resources.uNormalSampler = t.source.style;
    const u = this.resources.mrtUniforms.uniforms;
    u.uNormalRect = v ? uvRect(v) : new Float32Array([0, 0, 1, 1]);
    u.uHasNormal = v ? 1 : 0;
    this.resources.mrtUniforms.update();
  }
  /** Tile depth 0..1 for a thing, or `< 0` for ground (writes black depth). */
  set tileDepth(v: number) {
    this.resources.mrtUniforms.uniforms.uTileDepth = v;
    this.resources.mrtUniforms.update();
  }
  set tint(rgb: number) {
    this.resources.mrtUniforms.uniforms.uTint = new Float32Array([((rgb >> 16) & 0xff) / 255, ((rgb >> 8) & 0xff) / 255, (rgb & 0xff) / 255]);
    this.resources.mrtUniforms.update();
  }
  setChannels(chA: Float32Array, chB: Float32Array): void {
    const u = this.resources.mrtUniforms.uniforms;
    u.uChA = chA;
    u.uChB = chB;
    this.resources.mrtUniforms.update();
  }
  setNoiseGlobals(rows: number, uvTile: number, worldTile: number): void {
    this.resources.mrtUniforms.uniforms.uNoiseParams = new Float32Array([rows, uvTile, worldTile, 0]);
    this.resources.mrtUniforms.update();
  }
  setWorldRect(x: number, y: number, w: number, h: number): void {
    this.resources.mrtUniforms.uniforms.uWorldRect = new Float32Array([x, y, w, h]);
    this.resources.mrtUniforms.update();
  }
  setSeed(seed: number): void {
    this.resources.mrtUniforms.uniforms.uSeed = seed;
    this.resources.mrtUniforms.update();
  }
}

export function makeMrtBakeShader(): MrtBakeShader {
  const e = Texture.EMPTY;
  return new MrtBakeShader({
    glProgram: mrtProgram(),
    resources: {
      uResidual: e.source,
      uResidualSampler: e.source.style,
      uLayers: e.source,
      uLayersSampler: e.source.style,
      uSurface: e.source,
      uSurfaceSampler: e.source.style,
      uNoise: e.source,
      uNoiseSampler: e.source.style,
      uNormalTex: e.source,
      uNormalSampler: e.source.style,
      mrtUniforms: new UniformGroup({
        uResidualRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uLayersRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uSurfaceRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uNormalRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uChA: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uChB: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uNoiseParams: { value: new Float32Array([1, 1, 128, 0]), type: "vec4<f32>" },
        uWorldRect: { value: new Float32Array([0, 0, 64, 64]), type: "vec4<f32>" },
        uTint: { value: new Float32Array([1, 1, 1]), type: "vec3<f32>" },
        uSeed: { value: 0, type: "f32" },
        uHasLayers: { value: 0, type: "f32" },
        uHasNormal: { value: 0, type: "f32" },
        uTileDepth: { value: -1, type: "f32" },
      }),
    },
  });
}
