//! The material BAKE shader — the `albedo` channel's per-prim material, and now the SINGLE
//! albedo path for a real-tier thing. It reconstructs the display colour from the `albedo` map
//! (the residual base, RGB) plus the `layers` map (up to 3 per-material weight channels, RGB),
//! adding per-material hue/chroma VARIATION in OKLab (L held fixed → reads as pigment, not
//! light). NO-ALPHA model: the composite is OPAQUE colour (`out = (rgb, 1)`); the visual alpha
//! (coverage) is NOT baked in — the display shader applies `surface.B` itself. The silhouette is
//! a HARD `discard` on `surface.B` (AA off), so the opaque write lets the slot blit REPLACE
//! cleanly. When a thing has no `layers` map (`uHasLayers = 0`) the sum is just the residual
//! (== the full albedo). See `material.ts`, `oklab.ts`, and `docs/lighting.md`.
//!
//! A Pixi v8 high-shader Mesh program. We deliberately DROP the stock `textureBit`: it folds the
//! main texture's atlas matrix into `vUV`, but we sample FOUR differently-framed atlas
//! sub-textures (residual, layers, surface, noise), so we keep `vUV` as the RAW quad uv (0..1)
//! and map each with its own `uv-rect` uniform. `localUniformBit` supplies the transform +
//! `gl_Position`; `roundPixelsBit` snaps like the sprite path.
//!
//! Reconstruction: `out.rgb = residual + Σ layersᵢ·(jitter(tintᵢ) − tintᵢ)` (straight space),
//! `out.a = 1` (opaque); the display applies coverage from `surface.B`.
//!
//! GOTCHAS (as the lighting shader): a GLSL compile error draws the mesh BLACK with only a
//! console.error; `packed` is a GLSL reserved word (uniforms are `uLayers…`).

import {
  localUniformBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  UniformGroup,
} from "pixi.js";
import { compileHighShaderGlProgramES300 } from "./es3HighShader";
import { OKLAB_GLSL } from "../lighting/oklab";

/** Material weight channels the shader reconstructs — the width of the `uCh*` uniform arrays
 *  and the RGB channels of the `layers` map (the 4th/alpha channel was dropped: data in an
 *  alpha channel fights the upload-premultiply + downscale path). */
export const PACKED_CHANNELS = 3;

/** A texture's atlas-page uv rect `[offsetU, offsetV, scaleU, scaleV]` — maps the quad's
 *  raw uv (0..1) onto its sub-region of the `LodPool` page. Identity `[0,0,1,1]` for a
 *  whole-page texture. */
function uvRect(t: Texture): Float32Array {
  const f = t.frame;
  const w = t.source.width;
  const h = t.source.height;
  return new Float32Array([f.x / w, f.y / h, f.width / w, f.height / h]);
}

const materialBitGl = {
  name: "material-bake-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uResidual;               // albedo map = residual base (RGB, no alpha)
      uniform sampler2D uLayers;                 // per-material weight map (RGB coefficients)
      uniform sampler2D uSurface;                // surface map (its B = soft coverage/transparency)
      uniform sampler2D uNoise;                  // tiling noise atlas (rows = fields; RG = 2 decorrelated fields)
      uniform vec4 uResidualRect;                // residual uv rect on its page (offset.xy, scale.zw)
      uniform vec4 uLayersRect;                  // layers uv rect on its page
      uniform vec4 uSurfaceRect;                 // surface uv rect on its page
      uniform vec4 uChA[${PACKED_CHANNELS}];     // per channel: tint.rgb, hueSwing (radians)
      uniform vec4 uChB[${PACKED_CHANNELS}];     // per channel: chromaSwing, warmCoolBias, noiseRow (-1 = none), sampleSpace (0 uv | 1 world)
      uniform vec4 uNoiseParams;                 // x = atlas row count, y = uv tiling, z = world px per noise tile
      uniform vec3 uTint;                        // OUTPUT multiply — white for real materials (no-op), geoColor for a SOLID material
      uniform vec4 uWorldRect;                   // xy = prim world origin px, zw = prim world size px (world-space noise)
      uniform float uSeed;                       // STABLE per-instance seed (cell hash for static, objectId for movers)
      uniform float uHasLayers;                  // 1 = add the layer contributions, 0 = residual only

      ${OKLAB_GLSL}
    `,
    main: /* glsl */ `
      // CANONICAL reconstruction (split_layers): out = residual + Σ layersᵢ·jitter(tintᵢ), then
      // the visual alpha from surface.B applied LAST. Residual (RGB) is the leftover after the
      // weighted materials were stripped; each layer re-adds a material's contribution,
      // hue/chroma-jittered. At identity (tintᵢ = split_layers' base, zero swing) this rebuilds
      // the albedo. No layers → residual is already the full albedo.
      float cov = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).b;    // coverage
      if (cov < 0.5) discard;                       // HARD silhouette (AA off) — ground behind survives
      vec3 base = texture(uResidual, uResidualRect.xy + vUV * uResidualRect.zw).rgb;
      vec3 outc = base;
      if (uHasLayers > 0.5) {
        vec3 weights = texture(uLayers, uLayersRect.xy + vUV * uLayersRect.zw).rgb; // r,g,b coefficients
        // Per-instance UV offset from the STABLE seed (NOT the live position) — so identical
        // sprites don't share one noise pattern, and a mover's pattern doesn't swim as it walks.
        vec2 instanceOffset = fract(vec2(
          sin(uSeed * 127.1 + 311.7),
          sin(uSeed * 269.5 + 183.3)) * 43758.5453);
        for (int i = 0; i < ${PACKED_CHANNELS}; i++) {
          vec4 A = uChA[i];
          vec4 B = uChB[i];
          vec3 tint = A.rgb;
          float noiseRow = B.z;
          // UV-space noise rides the sprite (+ per-instance offset); world-space pins it to
          // the ground so tiles of one kind don't all show an identical patch.
          vec2 nuv = B.w < 0.5
            ? vUV * uNoiseParams.y + instanceOffset
            : (uWorldRect.xy + vUV * uWorldRect.zw) / max(uNoiseParams.z, 1.0);
          vec2 n = vec2(0.5);                       // a missing field → 0.5 → no perturbation
          if (noiseRow >= 0.0) {
            float row = (noiseRow + fract(nuv.y)) / max(uNoiseParams.x, 1.0);
            n = texture(uNoise, vec2(fract(nuv.x), row)).rg;
          }
          vec3 jit = jitterHueChroma(tint, n.r, n.g, A.a, B.x, B.y);
          outc += weights[i] * jit;                 // re-add this material's (jittered) contribution
        }
      }
      outColor = vec4(outc * uTint, 1.0);          // OPAQUE colour × tint (white for real; geoColor for a solid material)
    `,
  },
};

let program: GlProgram | null = null;
function materialProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgramES300({
      name: "material-bake",
      bits: [localUniformBitGl, materialBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** The material bake material for one prim. Bind the RESIDUAL base ({@link residual}), the
 *  LAYERS map ({@link layers}), the SURFACE map ({@link surface}, its B = the visual alpha),
 *  the shared NOISE atlas ({@link noise}), then push the per-channel params + world rect each
 *  bake. Pooled by {@link SquareCache} — one instance per concurrent material prim in a square,
 *  so each holds its own per-prim uniforms. */
export class MaterialBakeShader extends Shader {
  private _residual: Texture = Texture.EMPTY;

  /** `Mesh` requires its shader to be a `TextureShader` (expose `texture`). The mesh's
   *  "main" texture is the residual, so this aliases {@link residual}. */
  get texture(): Texture {
    return this._residual;
  }
  set texture(value: Texture) {
    this.residual = value;
  }

  /** The `albedo` (residual) LOD — the reconstruction base — sets its sampler + uv rect. */
  set residual(value: Texture) {
    this._residual = value;
    this.resources.uResidual = value.source;
    this.resources.uResidualSampler = value.source.style;
    this.resources.materialUniforms.uniforms.uResidualRect = uvRect(value);
    this.resources.materialUniforms.update();
  }
  /** The `layers` weight map LOD (or `null` → residual only) — sets its sampler + uv rect and
   *  the `uHasLayers` flag. */
  set layers(value: Texture | null) {
    const t = value ?? Texture.EMPTY;
    this.resources.uLayers = t.source;
    this.resources.uLayersSampler = t.source.style;
    const u = this.resources.materialUniforms.uniforms;
    u.uLayersRect = value ? uvRect(value) : new Float32Array([0, 0, 1, 1]);
    u.uHasLayers = value ? 1 : 0;
    this.resources.materialUniforms.update();
  }
  /** The SURFACE LOD — its B channel supplies the visual alpha applied after the reconstruction. */
  set surface(value: Texture) {
    this.resources.uSurface = value.source;
    this.resources.uSurfaceSampler = value.source.style;
    this.resources.materialUniforms.uniforms.uSurfaceRect = uvRect(value);
    this.resources.materialUniforms.update();
  }
  /** The shared tiling NOISE atlas (rows = fields; R/G = two decorrelated fields). */
  set noise(value: Texture) {
    this.resources.uNoise = value.source;
    this.resources.uNoiseSampler = value.source.style;
  }
  /** OUTPUT tint multiply (0xRRGGBB). White (0xffffff) for a real material = no-op (tint lives in the
   *  per-material channels); a SOLID material passes its `geoColor` here so white × geoColor = the flat box. */
  set tint(rgb: number) {
    this.resources.materialUniforms.uniforms.uTint = new Float32Array([((rgb >> 16) & 0xff) / 255, ((rgb >> 8) & 0xff) / 255, (rgb & 0xff) / 255]);
    this.resources.materialUniforms.update();
  }

  /** Push the material channels' params. `chA[i] = (tintR, tintG, tintB, hueSwing)`,
   *  `chB[i] = (chromaSwing, warmCoolBias, noiseRow, sampleSpace)`. */
  setChannels(chA: Float32Array, chB: Float32Array): void {
    const u = this.resources.materialUniforms.uniforms;
    u.uChA = chA;
    u.uChB = chB;
    this.resources.materialUniforms.update();
  }
  /** Noise globals: atlas `rows`, `uvTile` (noise tiles across a sprite), `worldTile`
   *  (world px per noise tile). */
  setNoiseGlobals(rows: number, uvTile: number, worldTile: number): void {
    this.resources.materialUniforms.uniforms.uNoiseParams = new Float32Array([rows, uvTile, worldTile, 0]);
    this.resources.materialUniforms.update();
  }
  /** The prim's world rect (px) — world-space noise samples `origin + vUV·size`. */
  setWorldRect(x: number, y: number, w: number, h: number): void {
    this.resources.materialUniforms.uniforms.uWorldRect = new Float32Array([x, y, w, h]);
    this.resources.materialUniforms.update();
  }
  /** The prim's STABLE per-instance seed (cell hash for static things, objectId for movers)
   *  — the UV-space noise offset is hashed from it, so instances differ without swimming. */
  setSeed(seed: number): void {
    this.resources.materialUniforms.uniforms.uSeed = seed;
    this.resources.materialUniforms.update();
  }
}

/** A fresh material bake material (its own uniform group, so pooled instances hold
 *  per-prim params independently). */
export function makeMaterialBakeShader(): MaterialBakeShader {
  const empty = Texture.EMPTY;
  return new MaterialBakeShader({
    glProgram: materialProgram(),
    resources: {
      uResidual: empty.source,
      uResidualSampler: empty.source.style,
      uLayers: empty.source,
      uLayersSampler: empty.source.style,
      uSurface: empty.source,
      uSurfaceSampler: empty.source.style,
      uNoise: empty.source,
      uNoiseSampler: empty.source.style,
      materialUniforms: new UniformGroup({
        uResidualRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uLayersRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uSurfaceRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uChA: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uChB: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uNoiseParams: { value: new Float32Array([1, 1, 128, 0]), type: "vec4<f32>" },
        uWorldRect: { value: new Float32Array([0, 0, 64, 64]), type: "vec4<f32>" },
        uTint: { value: new Float32Array([1, 1, 1]), type: "vec3<f32>" },
        uSeed: { value: 0, type: "f32" },
        uHasLayers: { value: 0, type: "f32" },
      }),
    },
  });
}
