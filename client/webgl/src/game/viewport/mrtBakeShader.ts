//! The MERGED G-buffer bake shader (webgl port) — ONE ES 3.00 fragment that writes all four
//! channels in a single pass via MRT. Outputs: 0 = albedo (material reconstruction × tint),
//! 1 = surface (presence/ao/coverage), 2 = normal (silhouette-keyed, flat-up fallback),
//! 3 = zdepth_world (tile depth in B). Every prim is a universal material (B2): one path +
//! one shared silhouette `discard` on coverage.
//!
//! Ported from the pixijs high-shader to a self-contained engine `Program`: the vertex maps a
//! unit quad → slot clip space via `uModel` (the SquareCache folds prim world rect + world→slot
//! + slot→clip into it), replacing Pixi's transform boilerplate; the four outs are declared
//! `layout(location=0..3)` directly (no `finalColor` patch). Fragment logic is verbatim.
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Program, type Texture } from "../../gl";
import { OKLAB_GLSL } from "../lighting/oklab";

/** Material weight channels the reconstruction sums — the width of the `uCh*` arrays + the RGB
 *  channels of the `layers` map. */
export const PACKED_CHANNELS = 3;

/** Identity uv rect (whole-page texture): `[offU, offV, scaleU, scaleV]`. Atlas sub-frames
 *  (from the real resolver) override this in W4c. */
const IDENTITY_RECT = (): Float32Array => new Float32Array([0, 0, 1, 1]);

const MRT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // unit quad 0..1 — IS the UV (the SquareCache's unitQuad has no aUV)
uniform mat3 uModel;            // unit quad -> slot clip space
out vec2 vUV;
void main() {
  vUV = aPosition;              // 0..1 across the quad → the material-map sub-frame UV
  vec3 p = uModel * vec3(aPosition, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const MRT_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uResidual;               // albedo residual base (RGB)
uniform sampler2D uLayers;                 // per-material weight map (RGB coefficients)
uniform sampler2D uSurface;                // surface: R=height, G=ao, B=coverage (also the silhouette)
uniform sampler2D uNoise;                  // tiling noise atlas
uniform sampler2D uNormalTex;              // normal LOD (RGB; flat-up bg)
uniform vec4 uResidualRect;
uniform vec4 uLayersRect;
uniform vec4 uSurfaceRect;
uniform vec4 uNormalRect;
uniform vec4 uChA[${PACKED_CHANNELS}];     // per channel: tint.rgb, hueSwing
uniform vec4 uChB[${PACKED_CHANNELS}];     // per channel: chromaSwing, warmCoolBias, noiseRow, sampleSpace
uniform vec4 uChC[${PACKED_CHANNELS}];     // material-system P1: detailRow, detailAmp, detailScale, placementMode
uniform vec4 uNoiseParams;                 // x=atlas rows, y=uv tiling, z=world px per noise tile
uniform vec4 uWorldRect;                   // xy = prim world origin px, zw = prim world size px
uniform vec3 uTint;                        // albedo OUTPUT multiply (white real, geoColor solid)
uniform float uSeed;                       // stable per-instance seed
uniform float uHasLayers;                  // 1 = add layer contributions
uniform float uHasNormal;                  // 1 = sample uNormalTex, 0 = flat-up
uniform float uTileDepth;                  // >=0 = thing depth; < 0 = ground (write black)
uniform int uEmissiveOn;                   // lighting-feel P3: 1 = relay the leaf's R (emissive mask) into A

layout(location = 0) out vec4 oAlbedo;
layout(location = 1) out vec4 oSurface;
layout(location = 2) out vec4 oNormal;
layout(location = 3) out vec4 oDepth;

${OKLAB_GLSL}

void main() {
  // Shared silhouette: coverage from the surface map's B. Solid materials carry a WHITE surface
  // (cov=1), so they never discard (full box); real things discard outside their silhouette.
  float cov = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).b;
  if (cov < 0.5) discard;

  // attachment 0: albedo (material reconstruction × tint)
  vec3 base = texture(uResidual, uResidualRect.xy + vUV * uResidualRect.zw).rgb;
  vec3 outc = base;
  // Per-instance noise offset — shared by the colour jitter AND the normal detail (one seed).
  vec2 instanceOffset = fract(vec2(sin(uSeed * 127.1 + 311.7), sin(uSeed * 269.5 + 183.3)) * 43758.5453);
  vec3 layerWeights = vec3(0.0);
  if (uHasLayers > 0.5) {
    layerWeights = texture(uLayers, uLayersRect.xy + vUV * uLayersRect.zw).rgb;
    vec3 weights = layerWeights;
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
  oAlbedo = vec4(outc * uTint, 1.0);

  // attachment 1: surface (R=presence, G=ao, B=coverage). Presence = tileDepth>=0.
  // A MUST stay 1.0: bakes ALPHA-BLEND (prims composite over their tile's ground in the same
  // composite), so an attachment's alpha is its BLEND FACTOR — writing data here multiplies the
  // whole write by it. Learned the hard way (lighting-feel P3: A=0 collapsed world coverage to
  // nothing). Spare DATA lanes live in attachments whose alpha stays 1 — the emissive relay rides
  // oDepth.r below.
  vec3 ssurf = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).rgb;
  oSurface = vec4(uTileDepth >= 0.0 ? 1.0 : 0.0, ssurf.g, ssurf.b, 1.0);

  // attachment 2: normal (real, or flat-up) + per-channel MATERIAL DETAIL (material-system P3).
  // Generated normals are smooth ("plastic"); each layer channel may carry a tiling detail field
  // (uChC: row, amp, scale) RNM-blended on, weighted by the channel's layer weight so needle
  // detail perturbs foliage and never trunk. Amp 0 (or no layers) = the base normal untouched —
  // the identity contract. A stays 1 (attachment alpha is a BLEND FACTOR — lighting-feel F3).
  vec3 nrm = uHasNormal > 0.5
    ? texture(uNormalTex, uNormalRect.xy + vUV * uNormalRect.zw).rgb * 2.0 - 1.0
    : vec3(0.0, 0.0, 1.0);
  if (uHasLayers > 0.5) {
    for (int i = 0; i < ${PACKED_CHANNELS}; i++) {
      vec4 C = uChC[i];
      float w = layerWeights[i];
      if (C.x < 0.0 || C.y <= 0.0 || w <= 0.0) continue;
      // The field's two decorrelated channels give an (x, y) tilt — the standard 2-channel bump.
      vec2 duv = vUV * uNoiseParams.y * max(C.z, 1e-3) + instanceOffset;
      float drow = (C.x + fract(duv.y)) / max(uNoiseParams.x, 1.0);
      vec2 dn2 = texture(uNoise, vec2(fract(duv.x), drow)).rg * 2.0 - 1.0;
      float a = C.y * w;
      vec3 det = normalize(vec3(dn2 * a, 1.0));
      // RNM (reoriented normal mapping): rotate the detail into the base normal's frame.
      vec3 t = nrm + vec3(0.0, 0.0, 1.0);
      vec3 u2 = det * vec3(-1.0, -1.0, 1.0);
      nrm = normalize(t * dot(t, u2) - u2 * t.z);
    }
  }
  oNormal = vec4(nrm * 0.5 + 0.5, 1.0);

  // attachment 3: zdepth_world (tile depth in B; ground = black). R relays the EMISSIVE mask
  // (lighting-feel P3): the leaf surface's reserved R channel, gated to REAL surface maps
  // (solid/geo materials bind fills whose R = 255 and must not glow). R was unused; alpha stays 1.
  oDepth = vec4(uEmissiveOn == 1 ? ssurf.r : 0.0, 0.0, uTileDepth < 0.0 ? 0.0 : uTileDepth, 1.0);
}
`;

/** The merged bake material for one prim — set every channel's inputs, then the SquareCache
 *  draws the unit quad into the 4-attachment MRT scratch with `uModel` placing it. Ported to
 *  hold state + apply it to an engine `Program` (Pixi's `Shader` resource groups are gone). */
export class MrtBakeShader {
  readonly program: Program;
  private residual: Texture | null = null;
  private layers: Texture | null = null;
  private surface: Texture | null = null;
  private noise: Texture | null = null;
  private normalTex: Texture | null = null;
  private residualRect = IDENTITY_RECT();
  private layersRect = IDENTITY_RECT();
  private surfaceRect = IDENTITY_RECT();
  private normalRect = IDENTITY_RECT();
  private chA = new Float32Array(PACKED_CHANNELS * 4);
  private chB = new Float32Array(PACKED_CHANNELS * 4);
  private chC = new Float32Array(PACKED_CHANNELS * 4);
  private noiseParams = new Float32Array([1, 1, 128, 0]);
  private worldRect = new Float32Array([0, 0, 64, 64]);
  private tint = new Float32Array([1, 1, 1]);
  private seed = 0;
  private hasLayers = 0;
  private hasNormal = 0;
  private tileDepth = -1;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, MRT_VERT, MRT_FRAG, "mrt-bake");
  }

  setResidual(t: Texture, rect = IDENTITY_RECT()): void {
    this.residual = t;
    this.residualRect = rect;
  }
  setLayers(t: Texture | null, rect = IDENTITY_RECT()): void {
    this.layers = t;
    this.layersRect = rect;
    this.hasLayers = t ? 1 : 0;
  }
  setSurface(t: Texture, rect = IDENTITY_RECT()): void {
    this.surface = t;
    this.surfaceRect = rect;
    // lighting-feel P3: the emissive relay (surface leaf R → composite A) is gated to REAL surface
    // maps. Solid/geo materials bind the 1×1 white fill, whose R = 255 would otherwise mark the
    // whole ground self-lit. Width > 1 IS the "real map" discriminator.
    this.emissiveOn = t.width > 1 ? 1 : 0;
  }
  private emissiveOn = 0;
  setNoise(t: Texture | null): void {
    this.noise = t;
  }
  /** The normal LOD, or null → flat-up fallback (`uHasNormal = 0`). */
  setNormal(t: Texture | null, rect = IDENTITY_RECT()): void {
    this.normalTex = t;
    this.normalRect = rect;
    this.hasNormal = t ? 1 : 0;
  }
  setTint(rgb: number): void {
    this.tint[0] = ((rgb >> 16) & 0xff) / 255;
    this.tint[1] = ((rgb >> 8) & 0xff) / 255;
    this.tint[2] = (rgb & 0xff) / 255;
  }
  setChannels(chA: Float32Array, chB: Float32Array, chC?: Float32Array): void {
    this.chA = chA;
    this.chB = chB;
    if (chC) this.chC = chC;
  }
  setNoiseGlobals(rows: number, uvTile: number, worldTile: number): void {
    this.noiseParams[0] = rows;
    this.noiseParams[1] = uvTile;
    this.noiseParams[2] = worldTile;
  }
  setWorldRect(x: number, y: number, w: number, h: number): void {
    this.worldRect[0] = x;
    this.worldRect[1] = y;
    this.worldRect[2] = w;
    this.worldRect[3] = h;
  }
  setSeed(seed: number): void {
    this.seed = seed;
  }
  setTileDepth(v: number): void {
    this.tileDepth = v;
  }

  /** The five sampler bindings (unset ones → `empty`). Insertion order = texture-unit order. */
  textures(empty: Texture): Record<string, Texture> {
    return {
      uResidual: this.residual ?? empty,
      uLayers: this.layers ?? empty,
      uSurface: this.surface ?? empty,
      uNoise: this.noise ?? empty,
      uNormalTex: this.normalTex ?? empty,
    };
  }

  /** Push every non-sampler uniform. The caller sets `uModel` + binds the textures. */
  apply(p: Program): void {
    p.uVec4("uResidualRect", this.residualRect[0], this.residualRect[1], this.residualRect[2], this.residualRect[3]);
    p.uVec4("uLayersRect", this.layersRect[0], this.layersRect[1], this.layersRect[2], this.layersRect[3]);
    p.uVec4("uSurfaceRect", this.surfaceRect[0], this.surfaceRect[1], this.surfaceRect[2], this.surfaceRect[3]);
    p.uVec4("uNormalRect", this.normalRect[0], this.normalRect[1], this.normalRect[2], this.normalRect[3]);
    p.uVec4Array("uChA", this.chA);
    p.uVec4Array("uChB", this.chB);
    p.uVec4Array("uChC", this.chC); // silently no-op until the shader consumes it (P3)
    p.uVec4("uNoiseParams", this.noiseParams[0], this.noiseParams[1], this.noiseParams[2], this.noiseParams[3]);
    p.uVec4("uWorldRect", this.worldRect[0], this.worldRect[1], this.worldRect[2], this.worldRect[3]);
    p.uVec3("uTint", this.tint[0], this.tint[1], this.tint[2]);
    p.uInt("uEmissiveOn", this.emissiveOn);
    p.uFloat("uSeed", this.seed);
    p.uFloat("uHasLayers", this.hasLayers);
    p.uFloat("uHasNormal", this.hasNormal);
    p.uFloat("uTileDepth", this.tileDepth);
  }

  destroy(): void {
    this.program.destroy();
  }
}
